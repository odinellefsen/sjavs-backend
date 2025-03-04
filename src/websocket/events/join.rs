use crate::websocket::handler::AppState;
use crate::websocket::types::GameMessage;
use deadpool_redis::Connection;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

pub async fn handle_join_event(
    state: &Arc<AppState>,
    user_id: &str,
    data: &Value,
    redis_conn: &mut Connection,
) -> Result<(), Box<dyn std::error::Error>> {
    // Extract the game ID from the message data
    let game_id = match data.get("game_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Err("Missing game_id in join request".into()),
    };

    // Check if the game exists
    let game_exists: bool = redis::cmd("EXISTS")
        .arg(format!("game:{}", game_id))
        .query_async(&mut **redis_conn)
        .await?;

    if !game_exists {
        // Send error message back to user
        let error_msg = GameMessage {
            event: "join_error".to_string(),
            data: serde_json::json!({
                "message": "Game not found",
                "game_id": game_id
            }),
        };

        if let Some(tx) = state.user_connections.get(user_id) {
            let msg = serde_json::to_string(&error_msg)?;
            tx.send(axum::extract::ws::Message::Text(msg)).await?;
        }

        return Err("Game not found".into());
    }

    // Check if player is already in a game
    let player_game: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(user_id)
        .query_async(&mut **redis_conn)
        .await?;

    if let Some(current_game_id) = player_game {
        if current_game_id == game_id {
            // Player is already in this game, just send a confirmation
            let already_joined_msg = GameMessage {
                event: "already_joined".to_string(),
                data: serde_json::json!({
                    "message": "You are already in this game",
                    "game_id": game_id
                }),
            };

            if let Some(tx) = state.user_connections.get(user_id) {
                let msg = serde_json::to_string(&already_joined_msg)?;
                tx.send(axum::extract::ws::Message::Text(msg)).await?;
            }

            return Ok(());
        } else {
            // Player is in a different game, send error
            let error_msg = GameMessage {
                event: "join_error".to_string(),
                data: serde_json::json!({
                    "message": "You are already in another game",
                    "current_game_id": current_game_id
                }),
            };

            if let Some(tx) = state.user_connections.get(user_id) {
                let msg = serde_json::to_string(&error_msg)?;
                tx.send(axum::extract::ws::Message::Text(msg)).await?;
            }

            return Err("Player already in another game".into());
        }
    }

    // Get game status
    let status: String = redis::cmd("HGET")
        .arg(format!("game:{}", game_id))
        .arg("status")
        .query_async(&mut **redis_conn)
        .await?;

    if status != "waiting" {
        // Game is not in waiting state
        let error_msg = GameMessage {
            event: "join_error".to_string(),
            data: serde_json::json!({
                "message": "Cannot join game - game is not in waiting state",
                "status": status
            }),
        };

        if let Some(tx) = state.user_connections.get(user_id) {
            let msg = serde_json::to_string(&error_msg)?;
            tx.send(axum::extract::ws::Message::Text(msg)).await?;
        }

        return Err("Game not in waiting state".into());
    }

    // Add player to the game in Redis
    let _: () = redis::cmd("HSET")
        .arg("player_games")
        .arg(user_id)
        .arg(game_id)
        .query_async(&mut **redis_conn)
        .await?;

    let _: () = redis::cmd("SADD")
        .arg(format!("game:{}:players", game_id))
        .arg(user_id)
        .query_async(&mut **redis_conn)
        .await?;

    // Add player to in-memory game players map
    state
        .game_players
        .entry(game_id.to_string())
        .or_insert_with(HashSet::new)
        .insert(user_id.to_string());

    // Get player username
    let username: String = redis::cmd("HGET")
        .arg("usernames")
        .arg(user_id)
        .query_async(&mut **redis_conn)
        .await
        .unwrap_or_else(|_| "Unknown Player".to_string());

    // Send join confirmation to the player
    let join_msg = GameMessage {
        event: "joined".to_string(),
        data: serde_json::json!({
            "message": "Successfully joined the game",
            "game_id": game_id,
            "status": status
        }),
    };

    if let Some(tx) = state.user_connections.get(user_id) {
        let msg = serde_json::to_string(&join_msg)?;
        tx.send(axum::extract::ws::Message::Text(msg)).await?;
    }

    // Get all players in the game
    let players: Vec<String> = redis::cmd("SMEMBERS")
        .arg(format!("game:{}:players", game_id))
        .query_async(&mut **redis_conn)
        .await?;

    // Get usernames for all players
    let mut player_info = Vec::new();
    for player_id in &players {
        let player_username: String = redis::cmd("HGET")
            .arg("usernames")
            .arg(player_id)
            .query_async(&mut **redis_conn)
            .await
            .unwrap_or_else(|_| "Unknown Player".to_string());

        player_info.push(serde_json::json!({
            "id": player_id,
            "username": player_username
        }));
    }

    // Send player list to the joining player
    let player_list_msg = GameMessage {
        event: "player_list".to_string(),
        data: serde_json::json!({
            "game_id": game_id,
            "players": player_info
        }),
    };

    if let Some(tx) = state.user_connections.get(user_id) {
        let msg = serde_json::to_string(&player_list_msg)?;
        tx.send(axum::extract::ws::Message::Text(msg)).await?;
    }

    // Notify other players about the new player
    let player_joined_msg = GameMessage {
        event: "player_joined".to_string(),
        data: serde_json::json!({
            "game_id": game_id,
            "player_id": user_id,
            "username": username,
            "message": format!("{} has joined the game", username)
        }),
    };

    // Broadcast to all other players in the game
    for player_id in players {
        if player_id != user_id {
            if let Some(tx) = state.user_connections.get(&player_id) {
                let msg = serde_json::to_string(&player_joined_msg)?;
                let _ = tx.send(axum::extract::ws::Message::Text(msg)).await;
            }
        }
    }

    Ok(())
}
