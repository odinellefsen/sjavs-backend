use super::types::GameMessage;
use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Extension, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::StreamExt;

#[axum::debug_handler]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(user_id): Extension<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, user_id))
}

pub async fn handle_socket(mut socket: WebSocket, user_id: String) {
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
