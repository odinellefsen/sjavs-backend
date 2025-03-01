use super::types::GameMessage;
use crate::RedisPool;
use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Extension, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::StreamExt;

#[axum::debug_handler]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, redis_pool))
}

pub async fn handle_socket(mut socket: WebSocket, user_id: String, redis_pool: RedisPool) {
    let mut conn = redis_pool
        .get()
        .await
        .expect("Failed to get Redis connection from pool");
    while let Some(Ok(msg)) = socket.next().await {
        if let Message::Text(text) = msg {
            if let Ok(game_msg) = serde_json::from_str::<GameMessage>(&text) {
                println!("Message from user {}: {:?}", user_id, game_msg);
                match game_msg.event.as_str() {
                    "join" => {
                        if let Some(game_id) = game_msg.data.get("game_id").and_then(|v| v.as_str())
                        {
                            let status: Option<String> = redis::cmd("HGET")
                                .arg(format!("game:{}", game_id))
                                .arg("status")
                                .query_async(&mut *conn)
                                .await
                                .unwrap_or(None);

                            let response = GameMessage {
                                event: "joined".to_string(),
                                data: serde_json::json!({
                                    "message": "Welcome to Sjavs!",
                                    "status": status.unwrap_or_else(|| "not_found".to_string())
                                }),
                            };
                            let _ = socket
                                .send(Message::Text(serde_json::to_string(&response).unwrap()))
                                .await;
                        }
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
