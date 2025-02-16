use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

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

    // Create game entry with initial state
    let game_state = json!({
        "game_id": game_id,
        "host": user_id,
        "players": [user_id],
        "status": "waiting",
        "created_at": chrono::Utc::now().to_rfc3339(),
    });

    // Store game state
    match redis::cmd("HSET")
        .arg("games")
        .arg(&game_id)
        .arg(game_state.to_string())
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
    let stored_game: Option<String> = redis::cmd("HGET")
        .arg("games")
        .arg(&game_id)
        .query_async(&mut *conn)
        .await
        .unwrap_or(None);

    let stored_player_game: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(&user_id)
        .query_async(&mut *conn)
        .await
        .unwrap_or(None);

    match (stored_game, stored_player_game) {
        (Some(game), Some(player_game)) if player_game == game_id => (
            StatusCode::CREATED,
            Json(json!({
                "message": "Game created and verified",
                "game_id": game_id,
                "state": serde_json::from_str::<serde_json::Value>(&game).unwrap()
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
        let game_data: Option<String> = redis::cmd("HGET")
            .arg("games")
            .arg(&game_id)
            .query_async(&mut *conn)
            .await
            .unwrap_or(None);

        if let Some(game_data) = game_data {
            let mut game_state: serde_json::Value =
                serde_json::from_str(&game_data).unwrap_or(json!({}));

            // Remove player from the players list
            if let Some(players) = game_state["players"].as_array_mut() {
                if let Some(index) = players.iter().position(|p| p == &json!(user_id)) {
                    players.remove(index);
                }
            }

            // If no players remain, remove the game entirely
            if let Some(players) = game_state["players"].as_array() {
                if players.is_empty() {
                    // Remove the game from Redis
                    let _ = redis::cmd("HDEL")
                        .arg("games")
                        .arg(&game_id)
                        .query_async::<_, ()>(&mut *conn)
                        .await;
                } else {
                    // Otherwise update the game with the new state
                    let _ = redis::cmd("HSET")
                        .arg("games")
                        .arg(&game_id)
                        .arg(game_state.to_string())
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
