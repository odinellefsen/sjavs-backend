use crate::redis::normal_match::id::{NormalMatch, NormalMatchStatus};
use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct JoinRequest {
    pub pin_code: String,
}

#[axum::debug_handler]
pub async fn join_match_handler(
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
    Json(payload): Json<JoinRequest>,
) -> Response {
    let mut conn = redis_pool
        .get()
        .await
        .expect("Failed to get Redis connection from pool");

    // Check if the player is already in a game.
    let player_game: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(&user_id)
        .query_async(&mut *conn)
        .await
        .unwrap_or(None);

    if let Some(game_id) = player_game {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "Already in game",
                "message": "You are already in an active game. Please leave your current game before joining a new one.",
                "game_id": game_id
            })),
        )
            .into_response();
    }

    // Look up the game using the provided pin code.
    let game_id: Option<String> = redis::cmd("HGET")
        .arg("game_pins")
        .arg(&payload.pin_code)
        .query_async(&mut *conn)
        .await
        .unwrap_or(None);

    if game_id.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid pin",
                "message": "No game found for the provided pin."
            })),
        )
            .into_response();
    }
    let game_id = game_id.unwrap();

    // Construct the normal match key - THIS WAS THE ISSUE
    let match_key = format!("normal_match:{}", game_id);

    // Fetch the game data using the new key format
    let game_data: HashMap<String, String> = redis::cmd("HGETALL")
        .arg(&match_key)
        .query_async(&mut *conn)
        .await
        .unwrap_or_default();

    if game_data.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Game not found",
                "message": "The game for the provided pin does not exist."
            })),
        )
            .into_response();
    }

    // Get the game status from the hash map
    let status_str = game_data
        .get("status")
        .map(String::as_str)
        .unwrap_or("waiting");
    let status = NormalMatchStatus::from(status_str);

    // Check if the game is joinable
    if status != NormalMatchStatus::Waiting {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "Game not joinable",
                "message": "The game is no longer accepting players."
            })),
        )
            .into_response();
    }

    // Add the player to the players list for this game
    let players_key = format!("{}:players", match_key);

    // Check if the player is already in the game
    let is_player_in_game: bool = redis::cmd("HEXISTS")
        .arg(&players_key)
        .arg(&user_id)
        .query_async(&mut *conn)
        .await
        .unwrap_or(false);

    if !is_player_in_game {
        // Add the player to the game
        let add_result: redis::RedisResult<()> = redis::cmd("HSET")
            .arg(&players_key)
            .arg(&user_id)
            .arg("player")
            .query_async(&mut *conn)
            .await;

        if let Err(e) = add_result {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to add player to game: {}", e)
                })),
            )
                .into_response();
        }
    }

    // Associate the player with the game
    let assoc_result: redis::RedisResult<()> = redis::cmd("HSET")
        .arg("player_games")
        .arg(&user_id)
        .arg(&game_id)
        .query_async(&mut *conn)
        .await;

    if let Err(e) = assoc_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to associate player with game: {}", e)
            })),
        )
            .into_response();
    }

    // Try to convert the game data to a NormalMatch struct for better type safety
    let game_match = match NormalMatch::from_redis_hash(game_id.clone(), &game_data) {
        Ok(m) => m,
        Err(_) => {
            // Even if conversion fails, we've already added the player, so continue
            return (
                StatusCode::OK,
                Json(json!({
                    "message": "Joined game successfully",
                    "game_id": game_id,
                    "state": game_data
                })),
            )
                .into_response();
        }
    };

    // Get the host ID from the players hash
    let host_id: Option<String> = redis::cmd("HGET")
        .arg(&players_key)
        .arg("host")
        .query_async(&mut *conn)
        .await
        .unwrap_or(None);

    // Return success response with game details
    (
        StatusCode::OK,
        Json(json!({
            "message": "Joined game successfully",
            "game_id": game_id,
            "state": {
                "id": game_match.id,
                "pin": game_match.pin,
                "status": game_match.status.to_string(),
                "number_of_crosses": game_match.number_of_crosses,
                "current_cross": game_match.current_cross,
                "created_timestamp": game_match.created_timestamp,
                "host": host_id.unwrap_or_default()
            }
        })),
    )
        .into_response()
}
