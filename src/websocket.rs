use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{Extension, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Serialize, Deserialize, Debug)]
struct GameMessage {
    event: String,
    data: serde_json::Value,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RwLock<AppState>>>,
    Extension(user_id): Extension<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, user_id))
}

async fn handle_socket(mut socket: WebSocket, _state: Arc<RwLock<AppState>>, user_id: String) {
    while let Some(Ok(msg)) = socket.next().await {
        if let Message::Text(text) = msg {
            if let Ok(game_msg) = serde_json::from_str::<GameMessage>(&text) {
                println!("Message from user {}: {:?}", user_id, game_msg);
                match game_msg.event.as_str() {
                    "join" => {
                        let response = GameMessage {
                            event: "joined".to_string(),
                            data: serde_json::json!({"message": "Welcome to Sjavs!"}),
                        };
                        let _ = socket
                            .send(Message::Text(serde_json::to_string(&response).unwrap()))
                            .await;
                    }
                    "test" => {
                        let response = GameMessage {
                            event: "test_response".to_string(),
                            data: serde_json::json!({"message": "Test received on server!"}),
                        };
                        let _ = socket
                            .send(Message::Text(serde_json::to_string(&response).unwrap()))
                            .await;
                    }
                    _ => {}
                }
            }
        }
    }
}
