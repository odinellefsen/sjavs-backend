use deadpool_redis::Connection;
use serde_json::Value;

pub struct NotificationRepository;

impl NotificationRepository {
    /// Publish a game event to the game_events_list for WebSocket distribution
    pub async fn publish_event(
        conn: &mut Connection,
        event_type: &str,
        game_id: &str,
        affected_players: Vec<String>,
        message: &str,
        additional_data: Option<Value>,
    ) -> Result<(), String> {
        // Create the event payload
        let mut payload = serde_json::json!({
            "event": event_type,
            "game_id": game_id,
            "affected_players": affected_players,
            "message": message
        });

        // Add any additional data if provided
        if let Some(extra_data) = additional_data {
            if let Some(obj) = payload.as_object_mut() {
                for (key, value) in extra_data.as_object().unwrap_or(&serde_json::Map::new()) {
                    obj.insert(key.clone(), value.clone());
                }
            }
        }

        // Publish to Redis
        redis::cmd("LPUSH")
            .arg("game_events_list")
            .arg(serde_json::to_string(&payload).unwrap_or_default())
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to publish game event: {}", e))?;

        Ok(())
    }

    /// Simplified version for player join notifications
    pub async fn publish_player_joined(
        conn: &mut Connection,
        game_id: &str,
        player_id: &str,
        username: &str,
        affected_players: Vec<String>,
    ) -> Result<(), String> {
        let additional_data = serde_json::json!({
            "player_id": player_id,
            "username": username
        });

        Self::publish_event(
            conn,
            "player_joined",
            game_id,
            affected_players,
            &format!("Player {} joined the game", username),
            Some(additional_data),
        )
        .await
    }

    /// Simplified version for game termination notifications
    pub async fn publish_game_terminated(
        conn: &mut Connection,
        game_id: &str,
        affected_players: Vec<String>,
        reason: &str,
    ) -> Result<(), String> {
        Self::publish_event(
            conn,
            "game_terminated",
            game_id,
            affected_players,
            reason,
            None,
        )
        .await
    }
}
