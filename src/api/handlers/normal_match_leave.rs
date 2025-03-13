use crate::redis::normal_match::id::NormalMatch;
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
    let mut conn = redis_pool
        .get()
        .await
        .expect("Failed to get Redis connection from pool");

    // Check if player is currently in a game
    let game_id: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(&user_id)
        .query_async::<_, Option<String>>(&mut *conn)
        .await
        .unwrap_or(None);

    if let Some(game_id) = game_id {
        // Construct the normal match key
        let match_key = format!("normal_match:{}", game_id);

        // Fetch the game data
        let game_data: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&match_key)
            .query_async(&mut *conn)
            .await
            .unwrap_or_default();

        if !game_data.is_empty() {
            // Get players for this game
            let players_key = format!("{}:players", match_key);
            let is_player_in_game: bool = redis::cmd("HEXISTS")
                .arg(&players_key)
                .arg(&user_id)
                .query_async(&mut *conn)
                .await
                .unwrap_or(false);

            if is_player_in_game {
                // Remove player from the game's player list
                let _ = redis::cmd("HDEL")
                    .arg(&players_key)
                    .arg(&user_id)
                    .query_async::<_, ()>(&mut *conn)
                    .await;

                // Check if there are any players left
                let remaining_players: u32 = redis::cmd("HLEN")
                    .arg(&players_key)
                    .query_async(&mut *conn)
                    .await
                    .unwrap_or(0);

                if remaining_players == 0 {
                    // No players left, delete the game
                    let _ = redis::cmd("DEL")
                        .arg(&match_key)
                        .arg(&players_key)
                        .query_async::<_, ()>(&mut *conn)
                        .await;

                    // Also remove the PIN mapping if it exists
                    if let Some(pin) = game_data.get("pin") {
                        let _ = redis::cmd("HDEL")
                            .arg("game_pins")
                            .arg(pin)
                            .query_async::<_, ()>(&mut *conn)
                            .await;
                    }
                }
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
