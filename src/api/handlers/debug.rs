use crate::RedisPool;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Endpoint to wipe all Redis data (use for debugging only).
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
