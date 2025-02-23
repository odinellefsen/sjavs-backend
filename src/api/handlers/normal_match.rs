use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};
use rand::Rng;
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

    // random 4 digit pin code
    let pin_code = rand::thread_rng().gen_range(1000..=9999).to_string();

    // set game pin
    match redis::cmd("HSET")
        // key
        .arg("game_pins")
        // field
        .arg(&pin_code)
        // value
        .arg(&game_id)
        .query_async::<_, ()>(&mut *conn)
        .await
    {
        Ok(_) => (),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to set game pin: {}", e)})),
            )
                .into_response();
        }
    }

    let now = chrono::Utc::now().to_rfc3339();
    let players = format!("[{}]", user_id);
    let game_fields = HashMap::from([
        ("host", user_id.as_str()),
        ("players", &players),
        ("status", "waiting"),
        ("created_at", &now),
    ]);

    // Create game entry with initial state using HSET with multiple fields
    match redis::cmd("HSET")
        .arg(format!("game:{}", game_id))
        .arg(game_fields)
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
                "game_pin": pin_code,
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
