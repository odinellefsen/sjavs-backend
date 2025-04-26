use crate::redis::pubsub::repository::PubSubRepository;
use deadpool_redis::Connection;
use serde_json::Value;

pub struct NotificationRepository;

impl NotificationRepository {
    // Publish a game event using Redis Pub/Sub
    pub async fn publish_event(
        conn: &mut Connection,
        event_type: &str,
        game_id: &str,
        affected_players: Vec<String>,
        message: &str,
        additional_data: Option<Value>,
    ) -> Result<(), String> {
        // Use the PubSub repository to publish the event
        PubSubRepository::publish_game_event(
            conn,
            event_type,
            game_id,
            &affected_players,
            message,
            additional_data,
        )
        .await
    }

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
