use crate::api::handlers::{
    debug, game_bidding, game_playing, game_start, normal_match, normal_match_join,
    normal_match_leave, openapi,
};
use crate::RedisPool;
use axum::{
    routing::{get, post},
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
            "/normal-match/join",
            post(normal_match_join::join_match_handler),
        )
        .route(
            "/normal-match/leave",
            post(normal_match_leave::leave_match_handler),
        )
        // Game management endpoints
        .route("/game/start", post(game_start::start_game_handler))
        .route("/game/hand", get(game_start::get_player_hand_handler))
        .route("/game/bid", post(game_bidding::make_bid_handler))
        .route("/game/pass", post(game_bidding::pass_bid_handler))
        .route("/game/play-card", post(game_playing::play_card_handler))
        .route("/game/trick", get(game_playing::get_trick_info_handler))
        // Debug endpoints
        .route("/debug/flush", post(debug::flush_redis_handler))
        .with_state(redis_pool)
}
