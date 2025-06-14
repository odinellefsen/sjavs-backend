use crate::api::schemas::{DebugResponse, ErrorResponse};
use crate::RedisPool;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Flush all Redis data (development only)
/// 
/// ⚠️ **WARNING**: This endpoint wipes ALL data from Redis. Use only for development
/// and testing purposes. This action is irreversible and will delete all games,
/// players, and cached data.
#[utoipa::path(
    post,
    path = "/debug/flush",
    tag = "Debug",
    responses(
        (
            status = 200, 
            description = "Redis data flushed successfully",
            body = DebugResponse
        ),
        (
            status = 500, 
            description = "Failed to flush Redis data",
            body = ErrorResponse
        )
    ),
    security(
        ("jwt_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn flush_redis_handler(State(redis_pool): State<RedisPool>) -> Response {
    // Acquire a connection from the pool
    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get Redis connection: {}", e)})),
            )
                .into_response();
        }
    };

    // Issue FLUSHALL to clear all keys
    if let Err(e) = redis::cmd("FLUSHALL")
        .query_async::<_, ()>(&mut *conn)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to flush Redis: {}", e)})),
        )
            .into_response();
    }

    // Success response
    (
        StatusCode::OK,
        Json(json!({"message": "Redis flushed successfully"})),
    )
        .into_response()
}
