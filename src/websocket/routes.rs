use super::handler::ws_handler;
use crate::state::AppState;
use crate::RedisPool;
use axum::{routing::get, Router};
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn create_router(redis_pool: RedisPool, state: Arc<RwLock<AppState>>) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state((redis_pool, state))
}
