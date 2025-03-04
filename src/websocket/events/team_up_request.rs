use crate::websocket::handler::AppState;
use crate::websocket::types::GameMessage;
use deadpool_redis::Connection;
use serde_json::Value;
use std::sync::Arc;

pub async fn handle_team_up_request(
    state: &Arc<AppState>,
    user_id: &str,
    data: &Value,
    redis_conn: &mut Connection,
) -> Result<(), Box<dyn std::error::Error>> {
    // Extract the target player ID and game ID from the message data
    let target_player_id = match data.get("target_player_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Err("Missing target_player_id in team up request".into()),
    };

    let game_id = match data.get("game_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Err("Missing game_id in team up request".into()),
    };

    // Check if the target player is in the same game
    let target_in_game = if let Some(players) = state.game_players.get(game_id) {
        players.contains(target_player_id)
    } else {
        false
    };

    if !target_in_game {
        // Send error message back to requestor
        let error_msg = GameMessage {
            event: "team_up_request_error".to_string(),
            data: serde_json::json!({
                "message": "Target player is not in this game",
                "target_player_id": target_player_id
            }),
        };

        if let Some(tx) = state.user_connections.get(user_id) {
            let msg = serde_json::to_string(&error_msg)?;
            tx.send(axum::extract::ws::Message::Text(msg)).await?;
        }

        return Err("Target player not in game".into());
    }

    // Check if the target player is already in a team
    let target_team: Option<String> = redis::cmd("HGET")
        .arg(format!("game:{}:player_teams", game_id))
        .arg(target_player_id)
        .query_async(&mut **redis_conn)
        .await?;

    if target_team.is_some() {
        // Send error message back to requestor
        let error_msg = GameMessage {
            event: "team_up_request_error".to_string(),
            data: serde_json::json!({
                "message": "Target player is already in a team",
                "target_player_id": target_player_id
            }),
        };

        if let Some(tx) = state.user_connections.get(user_id) {
            let msg = serde_json::to_string(&error_msg)?;
            tx.send(axum::extract::ws::Message::Text(msg)).await?;
        }

        return Err("Target player already in a team".into());
    }

    // Get sender's username for the notification
    let sender_username: String = redis::cmd("HGET")
        .arg("usernames")
        .arg(user_id)
        .query_async(&mut **redis_conn)
        .await?;

    // Create and send the team up request notification to the target player
    let request_msg = GameMessage {
        event: "team_up_request".to_string(),
        data: serde_json::json!({
            "from_player_id": user_id,
            "from_player_username": sender_username,
            "game_id": game_id,
            "message": format!("{} wants to team up with you!", sender_username)
        }),
    };

    // Send the team up request to the target player
    if let Some(tx) = state.user_connections.get(target_player_id) {
        let msg = serde_json::to_string(&request_msg)?;
        tx.send(axum::extract::ws::Message::Text(msg)).await?;

        // Send confirmation to the requesting player
        let confirm_msg = GameMessage {
            event: "team_up_request_sent".to_string(),
            data: serde_json::json!({
                "target_player_id": target_player_id,
                "message": format!("Team up request sent to player!")
            }),
        };

        if let Some(tx) = state.user_connections.get(user_id) {
            let msg = serde_json::to_string(&confirm_msg)?;
            tx.send(axum::extract::ws::Message::Text(msg)).await?;
        }
    } else {
        // Target player is not connected
        let error_msg = GameMessage {
            event: "team_up_request_error".to_string(),
            data: serde_json::json!({
                "message": "Target player is not currently connected",
                "target_player_id": target_player_id
            }),
        };

        if let Some(tx) = state.user_connections.get(user_id) {
            let msg = serde_json::to_string(&error_msg)?;
            tx.send(axum::extract::ws::Message::Text(msg)).await?;
        }

        return Err("Target player not connected".into());
    }

    Ok(())
}
