use axum::{
    extract::{WebSocketUpgrade, State},
    response::IntoResponse,
    routing::get,
};
use axum::extract::ws::{Message, WebSocket};
use std::sync::Arc;
use tokio::sync::RwLock;
use futures_util::{StreamExt, SinkExt};
use serde::{Serialize, Deserialize};
use crate::state::AppState;

#[derive(Serialize, Deserialize)]
struct GameMessage {
    event: String,
    data: serde_json::Value,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, _state: Arc<RwLock<AppState>>) {
    while let Some(Ok(msg)) = socket.next().await {
        if let Message::Text(text) = msg {
            if let Ok(game_msg) = serde_json::from_str::<GameMessage>(&text) {
                match game_msg.event.as_str() {
                    "join" => {
                        let response = GameMessage {
                            event: "joined".to_string(),
                            data: serde_json::json!({"message": "Welcome to Sjavs!"}),
                        };
                        let _ = socket.send(Message::Text(serde_json::to_string(&response).unwrap())).await;
                    }
                    "test" => {
                            println!("Test event received with data: {:?}", game_msg.data); // Log test event
                            let response = GameMessage {
                                event: "test_response".to_string(),
                                data: serde_json::json!({"message": "Test received on server!"}),
                            };
                            let _ = socket.send(Message::Text(serde_json::to_string(&response).unwrap())).await;
                        },
                    _ => {}
                }
            }
        }
    }
}
