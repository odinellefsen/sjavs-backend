use crate::redis::normal_match::id::{NormalMatch, NormalMatchStatus};
use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

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

    // Check if player is already in a game using repository
    match NormalMatchRepository::get_player_game(&mut conn, &user_id).await {
        Ok(Some(game_id)) => {
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
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to check player game status: {}", e)
                })),
            )
                .into_response();
        }
        _ => {} // Continue if player is not in a game
    }

    // Look up game ID by PIN code using repository
    let game_id = match NormalMatchRepository::get_id_by_pin(&mut conn, &payload.pin_code).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid pin",
                    "message": "No game found for the provided pin."
                })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to look up game by PIN: {}", e)
                })),
            )
                .into_response();
        }
    };

    // Get game data using repository
    let game_match = match NormalMatchRepository::get_by_id(&mut conn, &game_id).await {
        Ok(Some(match_data)) => match_data,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Game not found",
                    "message": "The game for the provided pin does not exist."
                })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to get game data: {}", e)
                })),
            )
                .into_response();
        }
    };

    // Check if the game is joinable
    if game_match.status != NormalMatchStatus::Waiting {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "Game not joinable",
                "message": "The game is no longer accepting players."
            })),
        )
            .into_response();
    }

    // Add player to the game using repository
    if let Err(e) = NormalMatchRepository::add_player(&mut conn, &game_id, &user_id, "player").await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to add player to game: {}", e)
            })),
        )
            .into_response();
    }

    // Get the redis key and players key
    let redis_key = format!("normal_match:{}", game_id);
    let players_key = format!("{}:players", redis_key);

    // Get the host ID from the players hash
    let host_id: Option<String> = redis::cmd("HGET")
        .arg(&players_key)
        .arg("host")
        .query_async(&mut conn)
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
