use super::handler::ws_handler;
use crate::RedisPool;
use axum::{routing::get, Router};

pub fn create_router(redis_pool: RedisPool) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(redis_pool)
}
