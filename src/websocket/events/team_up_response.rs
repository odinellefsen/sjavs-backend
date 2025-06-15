use crate::websocket::handler::AppState;
use crate::websocket::types::GameMessage;
use deadpool_redis::Connection;
use serde_json::Value;
use std::sync::Arc;

pub async fn handle_team_up_response(
    state: &Arc<AppState>,
    user_id: &str,
    data: &Value,
    redis_conn: &mut Connection,
) -> Result<(), Box<dyn std::error::Error>> {
    // Extract necessary data
    let from_player_id = match data.get("from_player_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Err("Missing from_player_id in team up response".into()),
    };

    let game_id = match data.get("game_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Err("Missing game_id in team up response".into()),
    };

    let accepted = match data.get("accepted").and_then(|v| v.as_bool()) {
        Some(val) => val,
        None => return Err("Missing accepted field in team up response".into()),
    };

    // Get usernames for notifications
    let responder_username: String = redis::cmd("HGET")
        .arg("usernames")
        .arg(user_id)
        .query_async(&mut **redis_conn)
        .await?;

    let requester_username: String = redis::cmd("HGET")
        .arg("usernames")
        .arg(from_player_id)
        .query_async(&mut **redis_conn)
        .await?;

    if accepted {
        // Create a new team or add to requester's team
        let requester_team: Option<String> = redis::cmd("HGET")
            .arg(format!("game:{}:player_teams", game_id))
            .arg(from_player_id)
            .query_async(&mut **redis_conn)
            .await?;

        let team_id = if let Some(team_id) = requester_team {
            // Add responder to requester's team
            let _: () = redis::cmd("HSET")
                .arg(format!("game:{}:player_teams", game_id))
                .arg(user_id)
                .arg(&team_id)
                .query_async(&mut **redis_conn)
                .await?;

            let _: () = redis::cmd("SADD")
                .arg(format!("game:{}:team:{}", game_id, team_id))
                .arg(user_id)
                .query_async(&mut **redis_conn)
                .await?;

            team_id
        } else {
            // Create a new team with both players
            let new_team_id = format!("team_{}", uuid::Uuid::new_v4().to_string());

            // Add both players to the team
            let _: () = redis::cmd("HSET")
                .arg(format!("game:{}:player_teams", game_id))
                .arg(user_id)
                .arg(&new_team_id)
                .query_async(&mut **redis_conn)
                .await?;

            let _: () = redis::cmd("HSET")
                .arg(format!("game:{}:player_teams", game_id))
                .arg(from_player_id)
                .arg(&new_team_id)
                .query_async(&mut **redis_conn)
                .await?;

            // Store team members
            let _: () = redis::cmd("SADD")
                .arg(format!("game:{}:team:{}", game_id, new_team_id))
                .arg(user_id)
                .query_async(&mut **redis_conn)
                .await?;

            let _: () = redis::cmd("SADD")
                .arg(format!("game:{}:team:{}", game_id, new_team_id))
                .arg(from_player_id)
                .query_async(&mut **redis_conn)
                .await?;

            new_team_id
        };

        // Notify both players about the team creation
        let team_created_msg = GameMessage::new(
            "team_created".to_string(),
            serde_json::json!({
                "team_id": team_id,
                "members": [user_id, from_player_id],
                "member_usernames": [responder_username, requester_username],
                "game_id": game_id
            }),
        );

        // Send to requester
        if let Some(tx) = state.user_connections.get(from_player_id) {
            let msg = serde_json::to_string(&team_created_msg)?;
            tx.send(axum::extract::ws::Message::Text(msg)).await?;
        }

        // Send to responder
        if let Some(tx) = state.user_connections.get(user_id) {
            let msg = serde_json::to_string(&team_created_msg)?;
            tx.send(axum::extract::ws::Message::Text(msg)).await?;
        }

        // Broadcast team update to all players in the game
        let team_update_msg = GameMessage::new(
            "team_update".to_string(),
            serde_json::json!({
                "team_id": team_id,
                "members": [user_id, from_player_id],
                "member_usernames": [responder_username, requester_username],
                "game_id": game_id
            }),
        );

        if let Some(players) = state.game_players.get(game_id) {
            let msg = serde_json::to_string(&team_update_msg)?;
            for player_id in players.iter() {
                if player_id != user_id && player_id != from_player_id {
                    if let Some(tx) = state.user_connections.get(player_id) {
                        let _ = tx.send(axum::extract::ws::Message::Text(msg.clone())).await;
                    }
                }
            }
        }
    } else {
        // Team up request was declined
        let declined_msg = GameMessage::new(
            "team_up_declined".to_string(),
            serde_json::json!({
                "by_player_id": user_id,
                "by_player_username": responder_username,
                "message": format!("{} declined your team up request", responder_username)
            }),
        );

        // Send declined notification to requester
        if let Some(tx) = state.user_connections.get(from_player_id) {
            let msg = serde_json::to_string(&declined_msg)?;
            tx.send(axum::extract::ws::Message::Text(msg)).await?;
        }
    }

    Ok(())
}
