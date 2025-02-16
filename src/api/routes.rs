use crate::api::handlers::normal_match;
use crate::RedisPool;
use axum::{
    routing::{delete, post},
    Router,
};

pub fn create_router(redis_pool: RedisPool) -> Router {
    Router::new()
        .route("/normal-match", post(normal_match::create_match_handler))
        .route("/normal-match", delete(normal_match::leave_match_handler))
        .with_state(redis_pool)
}
