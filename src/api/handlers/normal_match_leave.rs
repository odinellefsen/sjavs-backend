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

    // Check if player is currently in a game using the repository
    let player_game = match PlayerRepository::get_player_game(&mut conn, &user_id).await {
        Ok(game_id) => game_id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to check player game: {}", e)})),
            )
                .into_response();
        }
    };

    if let Some(game_id) = player_game {
        // Try to remove player using the repository (handles new format)
        match NormalMatchRepository::remove_player(&mut conn, &game_id, &user_id).await {
            Ok((game_deleted, affected_players)) => {
                // If host left (game deleted) and there were other players,
                // publish a message to Redis for WebSocket handlers to pick up
                if game_deleted && affected_players.len() > 1 {
                    if let Err(e) = NotificationRepository::publish_game_terminated(
                        &mut conn,
                        &game_id,
                        affected_players.clone(),
                        "Game terminated because host left",
                    )
                    .await
                    {
                        eprintln!("Failed to publish game termination event: {}", e);
                    }
                }

                // Player was successfully removed from the new format game
                return (
                    StatusCode::OK,
                    Json(json!({
                        "message": if game_deleted && affected_players.len() > 1 {
                            "You have left the game and it has been terminated"
                        } else {
                            "You have left the game"
                        },
                        "game_id": game_id,
                        "game_deleted": game_deleted
                    })),
                )
                    .into_response();
            }
            Err(e) if e.contains("Redis error") => {
                // Continue to check old format if Redis error - it might be old format
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Error leaving game: {}", e)})),
                )
                    .into_response();
            }
        }

        // If we reach here, try old format game
        let old_game_key = format!("game:{}", game_id);

        // Check old format
        let old_game_data: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&old_game_key)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();

        if !old_game_data.is_empty() {
            // Handle old format game
            let default_players = "[]".to_string();
            let players_str = old_game_data.get("players").unwrap_or(&default_players);
            let mut players: Vec<String> = serde_json::from_str(players_str).unwrap_or_default();

            // Remove the user from the players list
            players.retain(|p| p != &user_id);

            if players.is_empty() {
                // No players left, delete the game
                let _ = redis::cmd("DEL")
                    .arg(&old_game_key)
                    .query_async::<_, ()>(&mut conn)
                    .await;

                // Also remove the PIN mapping
                if let Some(pin) = old_game_data.get("pin") {
                    let _ = redis::cmd("HDEL")
                        .arg("game_pins")
                        .arg(pin)
                        .query_async::<_, ()>(&mut conn)
                        .await;
                }
            } else {
                // Update the players list
                let _ = redis::cmd("HSET")
                    .arg(&old_game_key)
                    .arg("players")
                    .arg(serde_json::to_string(&players).unwrap())
                    .query_async::<_, ()>(&mut conn)
                    .await;
            }

            // Remove the user reference to the game
            let _ = redis::cmd("HDEL")
                .arg("player_games")
                .arg(&user_id)
                .query_async::<_, ()>(&mut conn)
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
