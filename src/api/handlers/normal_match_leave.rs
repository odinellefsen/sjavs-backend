use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::collections::HashMap;

#[axum::debug_handler]
pub async fn leave_match_handler(
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
) -> Response {
    let mut conn = redis_pool.lock().await;

    // Check if player is currently in a game
    let player_game: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(&user_id)
        .query_async::<_, Option<String>>(&mut *conn)
        .await
        .unwrap_or(None);

    if let Some(game_id) = player_game {
        // Fetch the game data
        let game_data: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(format!("game:{}", game_id))
            .query_async(&mut *conn)
            .await
            .unwrap_or_default();

        if !game_data.is_empty() {
            // Update players list
            let players: Vec<String> =
                serde_json::from_str(&game_data["players"]).unwrap_or_default();
            let updated_players: Vec<String> =
                players.into_iter().filter(|p| p != &user_id).collect();

            if updated_players.is_empty() {
                // Remove the game from Redis if no players remain
                let _ = redis::cmd("DEL")
                    .arg(format!("game:{}", game_id))
                    .query_async::<_, ()>(&mut *conn)
                    .await;
            } else {
                // Update the players field
                let _ = redis::cmd("HSET")
                    .arg(format!("game:{}", game_id))
                    .arg("players")
                    .arg(serde_json::to_string(&updated_players).unwrap())
                    .query_async::<_, ()>(&mut *conn)
                    .await;
            }

            // Remove the user reference to the game
            let _ = redis::cmd("HDEL")
                .arg("player_games")
                .arg(&user_id)
                .query_async::<_, ()>(&mut *conn)
                .await;

            return (
                StatusCode::OK,
                Json(json!({
                    "message": "You have left the game",
                    "game_id": game_id
                })),
            )
                .into_response();
        } else {
            // Game not found
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Game not found"
                })),
            )
                .into_response();
        }
    } else {
        // User is not in any game
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "You are not in any game"
            })),
        )
            .into_response();
    }
}
