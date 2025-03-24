use super::types::GameMessage;
use crate::websocket::events::join::handle_join_event;
use crate::websocket::events::team_up_request::handle_team_up_request;
use crate::websocket::events::team_up_response::handle_team_up_response;
use crate::RedisPool;
use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Extension, State, WebSocketUpgrade},
    response::IntoResponse,
};
use dashmap::DashMap;
use futures_util::SinkExt;
use futures_util::StreamExt;
use redis;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;

// Define types for our connection registry
type UserId = String;
type GameId = String;
type MessageSender = mpsc::Sender<Message>;

// Application state to track connections
pub struct AppState {
    pub user_connections: DashMap<UserId, MessageSender>,
    pub game_players: DashMap<GameId, HashSet<UserId>>,
    pub redis_pool: RedisPool,
}

pub fn create_app_state(redis_pool: RedisPool) -> Arc<AppState> {
    let app_state = Arc::new(AppState {
        user_connections: DashMap::new(),
        game_players: DashMap::new(),
        redis_pool,
    });

    // Start a single Redis polling task for all connections
    start_event_listener(app_state.clone());

    app_state
}

// Start a single Redis polling task that distributes messages to all connections
fn start_event_listener(app_state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            // Get a connection for event polling
            let mut conn = match app_state.redis_pool.get().await {
                Ok(conn) => conn,
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            // Use BRPOP with a timeout to check for game events
            let key = "game_events_list";
            let result: Option<(String, String)> = redis::cmd("BRPOP")
                .arg(key)
                .arg(5) // 5 second timeout
                .query_async(&mut *conn)
                .await
                .unwrap_or(None);

            if let Some((_, payload)) = result {
                // Parse the JSON payload
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&payload) {
                    // Get the list of affected players
                    if let Some(affected) = event["affected_players"].as_array() {
                        let affected_players: Vec<String> = affected
                            .iter()
                            .filter_map(|p| p.as_str().map(|s| s.to_string()))
                            .collect();

                        // The event type and game ID
                        let event_type =
                            event["event"].as_str().unwrap_or("game_update").to_string();
                        let game_id = event["game_id"].as_str().unwrap_or("").to_string();
                        let message = event["message"]
                            .as_str()
                            .unwrap_or("Game update")
                            .to_string();

                        // Distribute to all affected players that are connected
                        for player_id in affected_players {
                            if let Some(tx) = app_state.user_connections.get(&player_id) {
                                let game_msg = GameMessage {
                                    event: event_type.clone(),
                                    data: json!({
                                        "message": message,
                                        "game_id": game_id
                                    }),
                                };

                                let _ = tx
                                    .send(Message::Text(
                                        serde_json::to_string(&game_msg).unwrap_or_default(),
                                    ))
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    });
}

#[axum::debug_handler]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(user_id): Extension<String>,
    State(app_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, app_state))
}

pub async fn handle_socket(socket: WebSocket, user_id: String, state: Arc<AppState>) {
    // Create independent clones for each use
    let forward_user_id = user_id.clone();
    let cleanup_user_id = user_id.clone();

    let (mut sender, mut receiver) = socket.split();

    // Create a channel for sending messages to this client
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    // Store the sender in our connection registry
    state.user_connections.insert(forward_user_id.clone(), tx);

    // Task to forward messages from the channel to the WebSocket
    let forward_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let Err(e) = sender.send(message).await {
                eprintln!("Failed to send message to user {}: {}", forward_user_id, e);
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
        // Clone directly from original user_id
        let handler_user_id = user_id.clone();

        if let Message::Text(text) = msg {
            if let Ok(game_msg) = serde_json::from_str::<GameMessage>(&text) {
                match game_msg.event.as_str() {
                    "join" => {
                        if let Err(e) = handle_join_event(
                            &state,
                            &handler_user_id,
                            &game_msg.data,
                            &mut redis_conn,
                        )
                        .await
                        {
                            eprintln!("Join event error: {}", e);
                        }
                    }
                    "team_up_request" => {
                        if let Err(e) = handle_team_up_request(
                            &state,
                            &handler_user_id,
                            &game_msg.data,
                            &mut redis_conn,
                        )
                        .await
                        {
                            eprintln!("Team up request error: {}", e);
                        }
                    }
                    "team_up_response" => {
                        if let Err(e) = handle_team_up_response(
                            &state,
                            &handler_user_id,
                            &game_msg.data,
                            &mut redis_conn,
                        )
                        .await
                        {
                            eprintln!("Team up response error: {}", e);
                        }
                    }
                    _ => {
                        // Handle unknown event types
                        println!("Received unknown event type: {}", game_msg.event);
                    }
                }
            }
        }
    }

    // Remove user from connection registry
    state.user_connections.remove(&cleanup_user_id);

    // Remove user from any games they were in
    for mut game_entry in state.game_players.iter_mut() {
        game_entry.retain(|id| id != &cleanup_user_id);
    }

    // Abort the forward task
    forward_task.abort();
}
