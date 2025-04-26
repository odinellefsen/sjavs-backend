use crate::api::handlers::{debug, normal_match, normal_match_join, normal_match_leave};
use crate::RedisPool;
use axum::{
    routing::{delete, post},
    Router,
};

pub fn create_router(redis_pool: RedisPool) -> Router {
    Router::new()
        .route("/normal-match", post(normal_match::create_match_handler))
        .route(
            "/normal-match/leave",
            delete(normal_match_leave::leave_match_handler),
        )
        .route(
            "/normal-match/join",
            post(normal_match_join::join_match_handler),
        )
        .route("/debug/flush", post(debug::flush_redis_handler))
        .with_state(redis_pool)
}
