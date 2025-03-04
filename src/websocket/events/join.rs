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

    // Check if the game exists and if the player is actually in this game
    let is_player_in_game: bool = redis::cmd("SISMEMBER")
        .arg(format!("game:{}:players", game_id))
        .arg(user_id)
        .query_async(&mut **redis_conn)
        .await?;

    if !is_player_in_game {
        return Err(format!("Player {} is not a member of game {}", user_id, game_id).into());
    }

    // Add player to the in-memory game players map for broadcasting
    state
        .game_players
        .entry(game_id.to_string())
        .or_insert_with(HashSet::new)
        .insert(user_id.to_string());

    // Get game status to tell the client
    let status: String = redis::cmd("HGET")
        .arg(format!("game:{}", game_id))
        .arg("status")
        .query_async(&mut **redis_conn)
        .await?;

    // Send confirmation to the client that they're now subscribed
    let join_msg = GameMessage {
        event: "subscribed".to_string(),
        data: serde_json::json!({
            "message": "Successfully subscribed to game updates",
            "game_id": game_id,
            "status": status
        }),
    };

    if let Some(tx) = state.user_connections.get(user_id) {
        let msg = serde_json::to_string(&join_msg)?;
        tx.send(axum::extract::ws::Message::Text(msg)).await?;
    }

    // Notify this player about all other players currently in the game
    let players: Vec<String> = redis::cmd("SMEMBERS")
        .arg(format!("game:{}:players", game_id))
        .query_async(&mut **redis_conn)
        .await?;

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

    // Broadcast to other players that this player is now connected via WebSocket
    let player_username: String = redis::cmd("HGET")
        .arg("usernames")
        .arg(user_id)
        .query_async(&mut **redis_conn)
        .await
        .unwrap_or_else(|_| "Unknown Player".to_string());

    let player_connected_msg = GameMessage {
        event: "player_connected".to_string(),
        data: serde_json::json!({
            "game_id": game_id,
            "player_id": user_id,
            "username": player_username
        }),
    };

    for player_id in players {
        if player_id != user_id {
            if let Some(tx) = state.user_connections.get(&player_id) {
                let msg = serde_json::to_string(&player_connected_msg)?;
                let _ = tx.send(axum::extract::ws::Message::Text(msg)).await;
            }
        }
    }

    Ok(())
}
