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
        // Try both the new and old format keys
        let new_match_key = format!("normal_match:{}", game_id);
        let old_game_key = format!("game:{}", game_id);

        // Check new format first
        let game_data: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&new_match_key)
            .query_async(&mut *conn)
            .await
            .unwrap_or_default();

        // If new format has data, use it
        if !game_data.is_empty() {
            let players_key = format!("{}:players", new_match_key);

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
                    .arg(&new_match_key)
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
        }

        // If new format doesn't have data, check old format
        let old_game_data: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&old_game_key)
            .query_async(&mut *conn)
            .await
            .unwrap_or_default();

        if !old_game_data.is_empty() {
            // Fix
            let default_players = "[]".to_string();
            let players_str = old_game_data.get("players").unwrap_or(&default_players);
            let mut players: Vec<String> = serde_json::from_str(players_str).unwrap_or_default();

            // Remove the user from the players list
            players.retain(|p| p.as_str() != user_id);

            if players.is_empty() {
                // No players left, delete the game
                let _ = redis::cmd("DEL")
                    .arg(&old_game_key)
                    .query_async::<_, ()>(&mut *conn)
                    .await;

                // Also remove the PIN mapping
                if let Some(pin) = old_game_data.get("pin") {
                    let _ = redis::cmd("HDEL")
                        .arg("game_pins")
                        .arg(pin)
                        .query_async::<_, ()>(&mut *conn)
                        .await;
                }
            } else {
                // Update the players list
                let _ = redis::cmd("HSET")
                    .arg(&old_game_key)
                    .arg("players")
                    .arg(serde_json::to_string(&players).unwrap())
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
        }

        // Game not found in either format
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Game not found"
            })),
        )
            .into_response()
    } else {
        // User is not in any game
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "You are not in any game"
            })),
        )
            .into_response()
    }
}
