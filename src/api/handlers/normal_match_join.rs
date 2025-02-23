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
    let mut conn = redis_pool.lock().await;

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

    // Fetch the game data.
    let game_data: HashMap<String, String> = redis::cmd("HGETALL")
        .arg(format!("game:{}", game_id))
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

    // Optionally: Check if the game is joinable (e.g. status "waiting")
    if let Some(status) = game_data.get("status") {
        if status != "waiting" {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "Game not joinable",
                    "message": "The game is no longer accepting players."
                })),
            )
                .into_response();
        }
    }

    // Retrieve and update the players list.
    let mut players: Vec<String> = match game_data.get("players") {
        Some(players_str) => serde_json::from_str(players_str).unwrap_or_default(),
        None => vec![],
    };

    // Add the player if not already present.
    if !players.contains(&user_id) {
        players.push(user_id.clone());
    }

    let updated_players = serde_json::to_string(&players).unwrap();
    let update_result: redis::RedisResult<()> = redis::cmd("HSET")
        .arg(format!("game:{}", game_id))
        .arg("players")
        .arg(updated_players)
        .query_async(&mut *conn)
        .await;

    if let Err(e) = update_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to update game players: {}", e)
            })),
        )
            .into_response();
    }

    // Associate the player with the game.
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

    (
        StatusCode::OK,
        Json(json!({
            "message": "Successfully joined game",
            "game_id": game_id,
            "players": players,
        })),
    )
        .into_response()
}
