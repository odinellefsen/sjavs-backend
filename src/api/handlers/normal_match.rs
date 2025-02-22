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
pub async fn create_match_handler(
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
) -> Response {
    let mut conn = redis_pool.lock().await;

    // Check if player is already in a game
    let player_game: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(&user_id)
        .query_async::<_, Option<String>>(&mut *conn)
        .await
        .unwrap_or(None);

    if let Some(game_id) = player_game {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "Already in game",
                "message": "You are already in an active game. Please leave or finish your current game before creating a new one.",
                "game_id": game_id
            })),
        )
            .into_response();
    }

    // Generate new game ID (timestamp + random suffix)
    let game_id = format!(
        "game_{}_{:x}",
        chrono::Utc::now().timestamp(),
        rand::random::<u16>()
    );

    // Create game entry with initial state using HSET with multiple fields
    match redis::cmd("HMSET")
        .arg(format!("game:{}", game_id))
        .arg("game_id")
        .arg(&game_id)
        .arg("host")
        .arg(&user_id)
        .arg("players")
        .arg(format!("[{}]", user_id)) // Store players as JSON array string
        .arg("status")
        .arg("waiting")
        .arg("created_at")
        .arg(chrono::Utc::now().to_rfc3339())
        .query_async::<_, ()>(&mut *conn)
        .await
    {
        Ok(_) => (),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create game: {}", e)})),
            )
                .into_response();
        }
    }

    // Associate player with game
    match redis::cmd("HSET")
        .arg("player_games")
        .arg(&user_id)
        .arg(&game_id)
        .query_async::<_, ()>(&mut *conn)
        .await
    {
        Ok(_) => (),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to associate player: {}", e)})),
            )
                .into_response();
        }
    }

    // After creating the game, verify it exists
    let stored_game: HashMap<String, String> = redis::cmd("HGETALL")
        .arg(format!("game:{}", game_id))
        .query_async(&mut *conn)
        .await
        .unwrap_or_default();

    let stored_player_game: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(&user_id)
        .query_async(&mut *conn)
        .await
        .unwrap_or(None);

    match (!stored_game.is_empty(), stored_player_game) {
        (true, Some(player_game)) if player_game == game_id => (
            StatusCode::CREATED,
            Json(json!({
                "message": "Game created and verified",
                "game_id": game_id,
                "state": stored_game
            })),
        )
            .into_response(),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Game creation verification failed"
            })),
        )
            .into_response(),
    }
}

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
