use crate::auth::verify_clerk_token;
use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::StatusCode,
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

#[derive(Deserialize)]
pub struct WsQuery {
    token: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    match verify_clerk_token(&query.token).await {
        Ok(claims) => {
            println!("Authentication successful for user: {}", claims.sub);
            let user_id = claims.sub.clone();
            ws.on_upgrade(move |socket| handle_socket(socket, state, user_id))
        }
        Err(e) => {
            eprintln!("Authentication error: {:?}", e);
            (
                StatusCode::UNAUTHORIZED,
                format!("Authentication failed: {}", e),
            )
                .into_response()
        }
    }
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
                        println!("Test event received with data: {:?}", game_msg.data); // Log test event
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
