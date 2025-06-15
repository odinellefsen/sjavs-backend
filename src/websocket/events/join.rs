use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::redis::player::repository::PlayerRepository;
use crate::websocket::handler::{subscribe_user_to_game, AppState};
use crate::websocket::state_builder::StateBuilder;
use crate::websocket::types::GameMessage;
use deadpool_redis::Connection;
use serde_json::Value;
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

    // Check if the player is in this game using PlayerRepository
    match PlayerRepository::get_player_game(redis_conn, user_id).await {
        Ok(Some(id)) if id == game_id => (),
        Ok(Some(_)) => return Err("Player is in a different game".into()),
        Ok(None) => return Err(format!("Player {} is not a member of any game", user_id).into()),
        Err(e) => return Err(format!("Redis error: {}", e).into()),
    };

    // Get the game using NormalMatchRepository
    let game = match NormalMatchRepository::get_by_id(redis_conn, &game_id).await {
        Ok(Some(game)) => game,
        Ok(None) => return Err(format!("Game {} not found", game_id).into()),
        Err(e) => return Err(format!("Failed to get game data: {}", e).into()),
    };

    // Register for WebSocket events and PubSub (replaces the old in-memory tracking)
    subscribe_user_to_game(state, game_id, user_id).await;

    // Send confirmation to the client that they're now subscribed
    let join_msg = GameMessage::new(
        "subscribed".to_string(),
        serde_json::json!({
            "message": "Successfully subscribed to game updates",
            "game_id": game_id,
            "status": game.status.to_string()
        }),
    )
    .with_game_id(game_id.to_string());

    if let Some(tx) = state.user_connections.get(user_id) {
        let msg = serde_json::to_string(&join_msg)?;
        tx.send(axum::extract::ws::Message::Text(msg)).await?;
    }

    // Get the host ID from the players hash
    let redis_key = format!("normal_match:{}", game_id);
    let players_key = format!("{}:players", redis_key);

    let host_id: Option<String> = redis::cmd("HGET")
        .arg(&players_key)
        .arg("host")
        .query_async(redis_conn)
        .await
        .unwrap_or(None);

    // Send full game state
    let game_state_msg = GameMessage::new(
        "game_state".to_string(),
        serde_json::json!({
            "game_id": game_id,
            "state": {
                "id": game.id,
                "pin": game.pin,
                "status": game.status.to_string(),
                "number_of_crosses": game.number_of_crosses,
                "current_cross": game.current_cross,
                "created_timestamp": game.created_timestamp,
                "host": host_id.unwrap_or_default()
            }
        }),
    )
    .with_game_id(game_id.to_string());

    if let Some(tx) = state.user_connections.get(user_id) {
        let msg = serde_json::to_string(&game_state_msg)?;
        tx.send(axum::extract::ws::Message::Text(msg)).await?;
    }

    // Get players in the game
    let players: Vec<(String, String)> = redis::cmd("HGETALL")
        .arg(&players_key)
        .query_async(redis_conn)
        .await?;

    // Convert to Vec<String> for player IDs only
    let player_ids: Vec<String> = players
        .iter()
        .step_by(2)
        .map(|(id, _)| id.clone())
        .collect();

    // Build player info list with usernames
    let mut player_info = Vec::new();
    for player_id in &player_ids {
        let player_username: String = redis::cmd("HGET")
            .arg("usernames")
            .arg(player_id)
            .query_async(redis_conn)
            .await
            .unwrap_or_else(|_| "Unknown Player".to_string());

        player_info.push(serde_json::json!({
            "id": player_id,
            "username": player_username
        }));
    }

    // Send player list to joining player
    let player_list_msg = GameMessage::new(
        "player_list".to_string(),
        serde_json::json!({
            "game_id": game_id,
            "players": player_info
        }),
    )
    .with_game_id(game_id.to_string());

    if let Some(tx) = state.user_connections.get(user_id) {
        let msg = serde_json::to_string(&player_list_msg)?;
        tx.send(axum::extract::ws::Message::Text(msg)).await?;
    }

    // Send phase-specific initial state
    match StateBuilder::send_initial_state(game_id, user_id, redis_conn).await {
        Ok(initial_state_msg) => {
            if let Some(tx) = state.user_connections.get(user_id) {
                let msg = serde_json::to_string(&initial_state_msg)?;
                let _ = tx.send(axum::extract::ws::Message::Text(msg)).await;
            }
        }
        Err(e) => {
            // Log error but don't fail the join - fall back to basic state
            eprintln!(
                "Failed to send initial state for user {} in game {}: {}",
                user_id, game_id, e
            );
        }
    }

    // Get joining player's username
    let player_username: String = redis::cmd("HGET")
        .arg("usernames")
        .arg(user_id)
        .query_async(redis_conn)
        .await
        .unwrap_or_else(|_| "Unknown Player".to_string());

    // Broadcast to other players that this player is now connected via WebSocket
    let player_connected_msg = GameMessage::new(
        "player_connected".to_string(),
        serde_json::json!({
            "game_id": game_id,
            "player_id": user_id,
            "username": player_username
        }),
    )
    .with_game_id(game_id.to_string());

    for player_id in player_ids {
        if player_id != user_id {
            if let Some(tx) = state.user_connections.get(&player_id) {
                let msg = serde_json::to_string(&player_connected_msg)?;
                let _ = tx.send(axum::extract::ws::Message::Text(msg)).await;
            }
        }
    }

    Ok(())
}
