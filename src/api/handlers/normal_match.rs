use crate::redis::normal_match::id::NormalMatch;
use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};
use rand::Rng;
use serde_json::json;

#[axum::debug_handler]
pub async fn create_match_handler(
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
) -> Response {
    let mut conn = redis_pool
        .get()
        .await
        .expect("Failed to get Redis connection from pool");

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

    // Generate random 4-digit PIN code
    let pin_code = rand::thread_rng().gen_range(1000..=9999);

    // Create a new NormalMatch instance
    let normal_match = NormalMatch::new(
        game_id.clone(),
        pin_code,
        3, // Default number of crosses - adjust as needed
    );

    // Set game pin
    match redis::cmd("HSET")
        .arg("game_pins")
        .arg(pin_code.to_string())
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

    // Store the match data in Redis using the new model
    let redis_key = normal_match.redis_key();
    let hash_map = normal_match.to_redis_hash();

    // Create game entry with initial state
    match redis::cmd("HSET")
        .arg(&redis_key)
        .arg(hash_map)
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

    // Store the host player in a separate hash
    match redis::cmd("HSET")
        .arg(format!("{}:players", redis_key))
        .arg(&user_id)
        .arg("host")
        .query_async::<_, ()>(&mut *conn)
        .await
    {
        Ok(_) => (),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to set host player: {}", e)})),
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
    let stored_hash = match redis::cmd("HGETALL")
        .arg(&redis_key)
        .query_async::<_, std::collections::HashMap<String, String>>(&mut *conn)
        .await
    {
        Ok(hash) => hash,
        Err(_) => std::collections::HashMap::new(),
    };

    let stored_match = match NormalMatch::from_redis_hash(game_id.clone(), &stored_hash) {
        Ok(m) => m,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Game creation verification failed"
                })),
            )
                .into_response();
        }
    };

    let stored_player_game: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(&user_id)
        .query_async(&mut *conn)
        .await
        .unwrap_or(None);

    match stored_player_game {
        Some(player_game) if player_game == game_id => (
            StatusCode::CREATED,
            Json(json!({
                "message": "Game created and verified",
                "game_id": game_id,
                "game_pin": pin_code,
                "state": {
                    "id": stored_match.id,
                    "pin": stored_match.pin,
                    "status": stored_match.status.to_string(),
                    "number_of_crosses": stored_match.number_of_crosses,
                    "current_cross": stored_match.current_cross,
                    "created_timestamp": stored_match.created_timestamp,
                    "host": user_id
                }
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
