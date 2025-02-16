use crate::state::AppState;
use crate::websocket::handler::handle_socket;
use crate::RedisPool;
use axum::{
    extract::{Extension, State, WebSocketUpgrade},
    response::IntoResponse,
};
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State((_, state)): State<(RedisPool, Arc<RwLock<AppState>>)>,
    Extension(user_id): Extension<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, user_id))
}
