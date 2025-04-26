use crate::redis::normal_match::id::NormalMatchStatus;
use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::redis::notification::repository::NotificationRepository;
use crate::redis::player::repository::PlayerRepository;
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
    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get Redis connection from pool: {}", e)})),
            )
                .into_response();
        }
    };

    // Check if player is already in a game using repository
    match PlayerRepository::get_player_game(&mut conn, &user_id).await {
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

    // Get all players in the game for notification
    let all_players: Vec<String> = redis::cmd("HKEYS")
        .arg(&players_key)
        .query_async(&mut conn)
        .await
        .unwrap_or_default();

    // Get the username for notification
    let username: String = redis::cmd("HGET")
        .arg("usernames")
        .arg(&user_id)
        .query_async(&mut conn)
        .await
        .unwrap_or_else(|_| "Unknown Player".to_string());

    // Notify other players about the new player
    if all_players.len() > 1 {
        // Only notify if there are other players
        if let Err(e) = NotificationRepository::publish_player_joined(
            &mut conn,
            &game_id,
            &user_id,
            &username,
            all_players,
        )
        .await
        {
            eprintln!("Failed to publish player joined event: {}", e);
        }
    }

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
