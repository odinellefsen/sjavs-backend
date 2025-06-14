use crate::api::handlers::{debug, normal_match, normal_match_join, normal_match_leave, openapi};
use crate::RedisPool;
use axum::{
    routing::{delete, get, post},
    Router,
};

/// Create router for public endpoints (no authentication required)
pub fn create_public_router(redis_pool: RedisPool) -> Router {
    Router::new()
        // OpenAPI documentation endpoints
        .route("/openapi.json", get(openapi::get_openapi_json))
        .with_state(redis_pool)
}

/// Create router for protected endpoints (authentication required)
pub fn create_protected_router(redis_pool: RedisPool) -> Router {
    Router::new()
        // Match management endpoints
        .route("/normal-match", post(normal_match::create_match_handler))
        .route(
            "/normal-match/leave",
            delete(normal_match_leave::leave_match_handler),
        )
        .route(
            "/normal-match/join",
            post(normal_match_join::join_match_handler),
        )
        // Debug endpoints
        .route("/debug/flush", post(debug::flush_redis_handler))
        .with_state(redis_pool)
}

/// Legacy function for backward compatibility (deprecated)
#[deprecated(note = "Use create_public_router and create_protected_router instead")]
pub fn create_router(redis_pool: RedisPool) -> Router {
    Router::new()
        .merge(create_public_router(redis_pool.clone()))
        .merge(create_protected_router(redis_pool))
}
