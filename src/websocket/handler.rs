use super::types::GameMessage;
use crate::RedisPool;
use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Extension, State, WebSocketUpgrade},
    response::IntoResponse,
};
use dashmap::DashMap;
use futures_util::SinkExt;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;

// Define types for our connection registry
type UserId = String;
type GameId = String;
type MessageSender = mpsc::Sender<Message>;

// Application state to track connections
pub struct AppState {
    // Map user_id -> sender channel
    user_connections: DashMap<UserId, MessageSender>,
    // Map game_id -> set of user_ids in that game
    game_players: DashMap<GameId, Vec<UserId>>,
    redis_pool: RedisPool,
}

// Replace the let statement with a function that creates the state
pub fn create_app_state(redis_pool: RedisPool) -> Arc<AppState> {
    Arc::new(AppState {
        user_connections: DashMap::new(),
        game_players: DashMap::new(),
        redis_pool,
    })
}

// Then modify your ws_handler to use this function
#[axum::debug_handler]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
) -> impl IntoResponse {
    // Create the app state here or pass it from main
    let app_state = create_app_state(redis_pool);
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, app_state))
}

pub async fn handle_socket(socket: WebSocket, user_id: String, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Create a channel for sending messages to this client
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    // Store the sender in our connection registry
    state.user_connections.insert(user_id.clone(), tx);

    // Task to forward messages from the channel to the WebSocket
    let forward_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    // Get Redis connection
    let mut redis_conn = match state.redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Failed to get Redis connection: {}", e);
            return;
        }
    };

    // Process incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            if let Ok(game_msg) = serde_json::from_str::<GameMessage>(&text) {
                println!("Message from user {}: {:?}", user_id, game_msg);

                match game_msg.event.as_str() {
                    "join" => {
                        if let Some(game_id) = game_msg.data.get("game_id").and_then(|v| v.as_str())
                        {
                            // Add user to the game's player list
                            state
                                .game_players
                                .entry(game_id.to_string())
                                .or_insert_with(Vec::new)
                                .push(user_id.clone());

                            // Get game status from Redis
                            let status: Option<String> = redis::cmd("HGET")
                                .arg(format!("game:{}", game_id))
                                .arg("status")
                                .query_async(&mut *redis_conn)
                                .await
                                .unwrap_or(None);

                            // Send welcome message to the user
                            let response = GameMessage {
                                event: "joined".to_string(),
                                data: serde_json::json!({
                                    "message": "Welcome to Sjavs!",
                                    "status": status.unwrap_or_else(|| "not_found".to_string())
                                }),
                            };

                            // Send to this user
                            if let Some(tx) = state.user_connections.get(&user_id) {
                                let _ = tx
                                    .send(Message::Text(serde_json::to_string(&response).unwrap()))
                                    .await;
                            }

                            // Notify other players in the game
                            broadcast_to_game(
                                &state,
                                game_id,
                                &GameMessage {
                                    event: "player_joined".to_string(),
                                    data: serde_json::json!({
                                        "user_id": user_id,
                                        "message": "A new player has joined the game!"
                                    }),
                                },
                                Some(&user_id), // Exclude the joining player
                            )
                            .await;
                        }
                    }
                    "game_action" => {
                        // Handle game actions (moves, etc.)
                        if let Some(game_id) = game_msg.data.get("game_id").and_then(|v| v.as_str())
                        {
                            // Process the game action
                            // ...

                            // Broadcast the action to all players in the game
                            broadcast_to_game(&state, game_id, &game_msg, None).await;
                        }
                    }
                    "leave" => {
                        if let Some(game_id) = game_msg.data.get("game_id").and_then(|v| v.as_str())
                        {
                            // Remove player from game
                            if let Some(mut players) = state.game_players.get_mut(game_id) {
                                players.retain(|id| id != &user_id);
                            }

                            // Notify other players
                            broadcast_to_game(
                                &state,
                                game_id,
                                &GameMessage {
                                    event: "player_left".to_string(),
                                    data: serde_json::json!({
                                        "user_id": user_id,
                                        "message": "A player has left the game"
                                    }),
                                },
                                None,
                            )
                            .await;
                        }
                    }
                    "test" => {
                        // Handle test event
                        let response = GameMessage {
                            event: "test_response".to_string(),
                            data: serde_json::json!({"message": "Test received on server!"}),
                        };

                        if let Some(tx) = state.user_connections.get(&user_id) {
                            let _ = tx
                                .send(Message::Text(serde_json::to_string(&response).unwrap()))
                                .await;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // WebSocket closed, clean up
    println!("WebSocket connection closed for user {}", user_id);

    // Remove user from connection registry
    state.user_connections.remove(&user_id);

    // Remove user from any games they were in
    for mut game_entry in state.game_players.iter_mut() {
        game_entry.retain(|id| id != &user_id);
    }

    // Abort the forward task
    forward_task.abort();
}

// Helper function to broadcast a message to all players in a game
async fn broadcast_to_game(
    state: &AppState,
    game_id: &str,
    message: &GameMessage,
    exclude_user: Option<&str>,
) {
    if let Some(players) = state.game_players.get(game_id) {
        let message_text = serde_json::to_string(message).unwrap();

        for user_id in players.iter() {
            // Skip excluded user if specified
            if let Some(excluded) = exclude_user {
                if user_id == excluded {
                    continue;
                }
            }

            // Send message to this player
            if let Some(tx) = state.user_connections.get(user_id) {
                let _ = tx.send(Message::Text(message_text.clone())).await;
            }
        }
    }
}
