use crate::redis::normal_match::id::NormalMatchStatus;
use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::redis::player::repository::PlayerRepository;
use crate::websocket::timestamp::TimestampManager;
use crate::websocket::types::*;
use deadpool_redis::Connection;
use serde_json::Value;
use std::collections::HashMap;

pub struct StateBuilder;

impl StateBuilder {
    /// Build common state shared across all phases
    pub async fn build_common_state(
        game_id: &str,
        timestamp: i64,
        redis_conn: &mut Connection,
    ) -> Result<CommonStateData, Box<dyn std::error::Error>> {
        // Get game match info
        let game_match = NormalMatchRepository::get_by_id(redis_conn, game_id)
            .await?
            .ok_or("Game not found")?;

        // Get host ID
        let players_key = format!("normal_match:{}:players", game_id);
        let host_id: Option<String> = redis::cmd("HGET")
            .arg(&players_key)
            .arg("host")
            .query_async(redis_conn)
            .await
            .unwrap_or(None);

        let match_info = MatchInfo {
            id: game_match.id.clone(),
            pin: game_match.pin,
            status: game_match.status.to_string(),
            number_of_crosses: game_match.number_of_crosses,
            current_cross: game_match.current_cross,
            created_timestamp: game_match.created_timestamp,
            host: host_id.unwrap_or_default(),
        };

        // Get all players in the game
        let players = Self::get_players_info(game_id, redis_conn).await?;

        Ok(CommonStateData {
            game_id: game_id.to_string(),
            match_info,
            players,
            timestamp,
        })
    }

    /// Get player information for all players in the game
    async fn get_players_info(
        game_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<Vec<PlayerInfo>, Box<dyn std::error::Error>> {
        let players_key = format!("normal_match:{}:players", game_id);

        // Get all player IDs and roles from the hash
        let players_data: Vec<(String, String)> = redis::cmd("HGETALL")
            .arg(&players_key)
            .query_async(redis_conn)
            .await?;

        let mut player_info = Vec::new();

        // Process the hash pairs (field, value)
        let mut i = 0;
        while i < players_data.len() {
            let player_id = &players_data[i].0;
            let role = &players_data[i].1;

            // Skip non-player entries like "host"
            if player_id == "host" {
                i += 2;
                continue;
            }

            // Get username from usernames hash
            let username: String = redis::cmd("HGET")
                .arg("usernames")
                .arg(player_id)
                .query_async(redis_conn)
                .await
                .unwrap_or_else(|_| "Unknown Player".to_string());

            // Try to get player position (may not exist in all phases)
            let position = Self::get_player_position(game_id, player_id, redis_conn)
                .await
                .ok();

            player_info.push(PlayerInfo {
                user_id: player_id.clone(),
                username,
                position,
                role: role.clone(),
            });

            i += 2; // Move to next key-value pair
        }

        Ok(player_info)
    }

    /// Get player position in the game (0-3)
    async fn get_player_position(
        game_id: &str,
        user_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<u8, Box<dyn std::error::Error>> {
        // This would be implemented based on your position storage system
        // For now, return a placeholder - you'll need to implement based on your Redis schema
        let position_key = format!("game_positions:{}", game_id);
        let position: Option<u8> = redis::cmd("HGET")
            .arg(&position_key)
            .arg(user_id)
            .query_async(redis_conn)
            .await
            .unwrap_or(None);

        position.ok_or_else(|| "Player position not found".into())
    }

    /// Build waiting phase state
    pub async fn build_waiting_state(
        game_id: &str,
        user_id: &str,
        timestamp: i64,
        redis_conn: &mut Connection,
    ) -> Result<WaitingStateData, Box<dyn std::error::Error>> {
        let common_state = Self::build_common_state(game_id, timestamp, redis_conn).await?;

        // Check if user is host
        let is_host = common_state.match_info.host == user_id;

        // Check if game can be started (4 players)
        let player_count = common_state.players.len() as u8;
        let can_start_game = is_host && player_count >= 4;
        let players_needed = if player_count >= 4 {
            0
        } else {
            4 - player_count
        };

        Ok(WaitingStateData {
            common: common_state,
            can_start_game,
            players_needed,
            is_host,
        })
    }

    /// Build dealing phase state
    pub async fn build_dealing_state(
        game_id: &str,
        timestamp: i64,
        redis_conn: &mut Connection,
    ) -> Result<DealingStateData, Box<dyn std::error::Error>> {
        let common_state = Self::build_common_state(game_id, timestamp, redis_conn).await?;

        // Get dealer position from game state
        let game_match = NormalMatchRepository::get_by_id(redis_conn, game_id)
            .await?
            .ok_or("Game not found")?;

        let dealer_position = game_match.dealer_position.unwrap_or(0) as u8;

        // For simplicity, assume dealing is in progress
        let dealing_progress = "dealing".to_string();

        Ok(DealingStateData {
            common: common_state,
            dealer_position,
            dealing_progress,
        })
    }

    /// Determine the appropriate phase-specific state to send
    pub async fn send_initial_state(
        game_id: &str,
        user_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<GameMessage, Box<dyn std::error::Error>> {
        // Generate timestamp for this snapshot
        let timestamp = TimestampManager::snapshot_timestamp();

        // Get current game status
        let game_match = NormalMatchRepository::get_by_id(redis_conn, game_id)
            .await?
            .ok_or("Game not found")?;

        // Build appropriate state based on game phase
        match game_match.status {
            NormalMatchStatus::Waiting => {
                let state =
                    Self::build_waiting_state(game_id, user_id, timestamp, redis_conn).await?;
                Ok(GameMessage::new(
                    "initial_state_waiting".to_string(),
                    serde_json::to_value(&state)?,
                )
                .with_game_id(game_id.to_string())
                .with_phase("waiting".to_string())
                .with_timestamp(timestamp))
            }
            NormalMatchStatus::Dealing => {
                let state = Self::build_dealing_state(game_id, timestamp, redis_conn).await?;
                Ok(GameMessage::new(
                    "initial_state_dealing".to_string(),
                    serde_json::to_value(&state)?,
                )
                .with_game_id(game_id.to_string())
                .with_phase("dealing".to_string())
                .with_timestamp(timestamp))
            }
            NormalMatchStatus::Bidding => {
                // TODO: Implement in Step 3
                Ok(GameMessage::new(
                    "initial_state_placeholder".to_string(),
                    serde_json::json!({"message": "Bidding state not yet implemented"}),
                )
                .with_timestamp(timestamp))
            }
            NormalMatchStatus::Playing => {
                // TODO: Implement in Step 4
                Ok(GameMessage::new(
                    "initial_state_placeholder".to_string(),
                    serde_json::json!({"message": "Playing state not yet implemented"}),
                )
                .with_timestamp(timestamp))
            }
            NormalMatchStatus::Completed => {
                // TODO: Implement in Step 5
                Ok(GameMessage::new(
                    "initial_state_placeholder".to_string(),
                    serde_json::json!({"message": "Completed state not yet implemented"}),
                )
                .with_timestamp(timestamp))
            }
            _ => Err("Unknown game status".into()),
        }
    }
}
