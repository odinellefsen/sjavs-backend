use deadpool_redis::Connection;
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

pub struct PubSubRepository;

impl PubSubRepository {
    // Channel for game events - each game gets its own channel
    pub fn game_channel(game_id: &str) -> String {
        format!("game_events:{}", game_id)
    }

    // Channel for player-specific events
    pub fn player_channel(player_id: &str) -> String {
        format!("player_events:{}", player_id)
    }

    // Generate a unique instance ID for this server
    pub fn generate_instance_id() -> String {
        format!("instance:{}", Uuid::new_v4())
    }

    /// Publish a game event to all instances via Pub/Sub
    pub async fn publish_game_event(
        conn: &mut Connection,
        event_type: &str,
        game_id: &str,
        affected_players: &[String],
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

        let json_payload = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());

        // Publish to game channel
        redis::cmd("PUBLISH")
            .arg(Self::game_channel(game_id))
            .arg(&json_payload)
            .query_async::<_, i32>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to publish to game channel: {}", e))?;

        // Also publish to each player's channel
        for player_id in affected_players {
            redis::cmd("PUBLISH")
                .arg(Self::player_channel(player_id))
                .arg(&json_payload)
                .query_async::<_, i32>(&mut *conn)
                .await
                .map_err(|e| format!("Failed to publish to player channel: {}", e))?;
        }

        Ok(())
    }

    /// Simplified version for player join notifications
    pub async fn publish_player_joined(
        conn: &mut Connection,
        game_id: &str,
        player_id: &str,
        username: &str,
        affected_players: &[String],
    ) -> Result<(), String> {
        let additional_data = serde_json::json!({
            "player_id": player_id,
            "username": username
        });

        Self::publish_game_event(
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
        affected_players: &[String],
        reason: &str,
    ) -> Result<(), String> {
        Self::publish_game_event(
            conn,
            "game_terminated",
            game_id,
            affected_players,
            reason,
            None,
        )
        .await
    }

    /// Subscribe to channels for specific games and players
    /// Returns the PubSub object that can be used for listening to messages
    pub async fn subscribe_to_channels(
        conn: redis::aio::Connection,
        game_ids: &HashSet<String>,
        player_ids: &HashSet<String>,
    ) -> Result<redis::aio::PubSub, String> {
        // Create a vector of channels to subscribe to
        let mut channels = Vec::new();

        // Add game channels
        for game_id in game_ids {
            channels.push(Self::game_channel(game_id));
        }

        // Add player channels
        for player_id in player_ids {
            channels.push(Self::player_channel(player_id));
        }

        // Convert to PubSub
        let mut pubsub = conn.into_pubsub();

        // Subscribe to all channels if there are any
        if !channels.is_empty() {
            for channel in channels {
                pubsub
                    .subscribe(channel)
                    .await
                    .map_err(|e| format!("Failed to subscribe to channel: {}", e))?;
            }
        }

        Ok(pubsub)
    }
}
