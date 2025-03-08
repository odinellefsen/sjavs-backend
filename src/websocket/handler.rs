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
    Arc::new(AppState {
        user_connections: DashMap::new(),
        game_players: DashMap::new(),
        redis_pool,
    })
}

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

    // WebSocket closed, clean up
    println!("WebSocket connection closed for user {}", cleanup_user_id);

    // Remove user from connection registry
    state.user_connections.remove(&cleanup_user_id);

    // Remove user from any games they were in
    for mut game_entry in state.game_players.iter_mut() {
        game_entry.retain(|id| id != &cleanup_user_id);
    }

    // Abort the forward task
    forward_task.abort();
}
