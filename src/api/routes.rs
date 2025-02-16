use crate::api::handlers::match_handler;
use crate::state::AppState;
use crate::RedisPool;
use axum::{routing::post, Router};
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn create_router(redis_pool: RedisPool, state: Arc<RwLock<AppState>>) -> Router {
    Router::new()
        .route("/create-match", post(match_handler::create_match_handler))
        .route("/leave-match", post(match_handler::leave_match_handler))
        .with_state((redis_pool, state))
}
