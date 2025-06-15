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
    ) -> Result<CommonStateData, Box<dyn std::error::Error + Send + Sync>> {
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
    ) -> Result<Vec<PlayerInfo>, Box<dyn std::error::Error + Send + Sync>> {
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
    ) -> Result<u8, Box<dyn std::error::Error + Send + Sync>> {
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

    /// Check if user is the host of the game
    async fn is_host(
        game_id: &str,
        user_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let players_key = format!("normal_match:{}:players", game_id);
        let host_id: Option<String> = redis::cmd("HGET")
            .arg(&players_key)
            .arg("host")
            .query_async(redis_conn)
            .await
            .unwrap_or(None);

        Ok(host_id.as_deref() == Some(user_id))
    }

    /// Get player count for the game
    async fn get_player_count(
        game_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<u8, Box<dyn std::error::Error + Send + Sync>> {
        let players_key = format!("normal_match:{}:players", game_id);

        // Get all fields in the hash
        let players_data: Vec<(String, String)> = redis::cmd("HGETALL")
            .arg(&players_key)
            .query_async(redis_conn)
            .await?;

        // Count actual players (exclude "host" field)
        let player_count = players_data
            .iter()
            .step_by(2) // Only look at keys
            .filter(|(key, _)| key != "host")
            .count() as u8;

        Ok(player_count)
    }

    /// Get username for a user ID
    async fn get_username(
        user_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let username: String = redis::cmd("HGET")
            .arg("usernames")
            .arg(user_id)
            .query_async(redis_conn)
            .await
            .unwrap_or_else(|_| "Unknown Player".to_string());

        Ok(username)
    }

    /// Build waiting phase state with enhanced host and player management
    pub async fn build_waiting_state(
        game_id: &str,
        user_id: &str,
        timestamp: i64,
        redis_conn: &mut Connection,
    ) -> Result<WaitingStateData, Box<dyn std::error::Error + Send + Sync>> {
        // Build common state
        let common_state = Self::build_common_state(game_id, timestamp, redis_conn).await?;

        // Use helper functions for more accurate checks
        let is_host = Self::is_host(game_id, user_id, redis_conn)
            .await
            .unwrap_or(false);
        let player_count = Self::get_player_count(game_id, redis_conn)
            .await
            .unwrap_or(0);

        // Game can be started if host has 4 players and game is in waiting status
        let can_start_game =
            is_host && player_count >= 4 && common_state.match_info.status == "Waiting";

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

    /// Build dealing phase state with enhanced dealer and progress information
    pub async fn build_dealing_state(
        game_id: &str,
        timestamp: i64,
        redis_conn: &mut Connection,
    ) -> Result<DealingStateData, Box<dyn std::error::Error + Send + Sync>> {
        let common_state = Self::build_common_state(game_id, timestamp, redis_conn).await?;

        // Get dealer position from game state
        let game_match = NormalMatchRepository::get_by_id(redis_conn, game_id)
            .await?
            .ok_or("Game not found")?;

        let dealer_position = game_match.dealer_position.unwrap_or(0) as u8;

        // Determine dealing progress based on game state
        let dealing_progress = Self::get_dealing_progress(game_id, redis_conn)
            .await
            .unwrap_or_else(|_| "dealing".to_string());

        Ok(DealingStateData {
            common: common_state,
            dealer_position,
            dealing_progress,
        })
    }

    /// Get the current dealing progress
    async fn get_dealing_progress(
        game_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Check if hands have been dealt by looking for hand data
        let hand_key = format!("game_hands:{}:0", game_id);
        let hand_exists: bool = redis::cmd("EXISTS")
            .arg(&hand_key)
            .query_async(redis_conn)
            .await?;

        if hand_exists {
            // Check if all 4 players have hands
            let mut all_hands_dealt = true;
            for position in 0..4 {
                let pos_hand_key = format!("game_hands:{}:{}", game_id, position);
                let pos_exists: bool = redis::cmd("EXISTS")
                    .arg(&pos_hand_key)
                    .query_async(redis_conn)
                    .await?;

                if !pos_exists {
                    all_hands_dealt = false;
                    break;
                }
            }

            if all_hands_dealt {
                Ok("complete".to_string())
            } else {
                Ok("dealing".to_string())
            }
        } else {
            Ok("starting".to_string())
        }
    }

    /// Determine the appropriate phase-specific state to send
    pub async fn send_initial_state(
        game_id: &str,
        user_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<GameMessage, Box<dyn std::error::Error + Send + Sync>> {
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
                let state =
                    Self::build_bidding_state(game_id, user_id, timestamp, redis_conn).await?;
                Ok(GameMessage::new(
                    "initial_state_bidding".to_string(),
                    serde_json::to_value(&state)?,
                )
                .with_game_id(game_id.to_string())
                .with_phase("bidding".to_string())
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

    /// Build bidding phase state with player hand, available bids, and bidding context
    pub async fn build_bidding_state(
        game_id: &str,
        user_id: &str,
        timestamp: i64,
        redis_conn: &mut Connection,
    ) -> Result<BiddingStateData, Box<dyn std::error::Error + Send + Sync>> {
        // Build common state
        let common_state = Self::build_common_state(game_id, timestamp, redis_conn).await?;

        // Get game match for bidding info
        let game_match = NormalMatchRepository::get_by_id(redis_conn, game_id)
            .await?
            .ok_or("Game not found")?;

        let dealer_position = game_match
            .dealer_position
            .ok_or("Dealer position not set")? as u8;
        let current_bidder = game_match.current_bidder.ok_or("Current bidder not set")? as u8;

        // Get player position to determine if they're a player or spectator
        let player_position = Self::get_player_position(game_id, user_id, redis_conn)
            .await
            .ok();

        // Get player's hand and calculate available bids (only for actual players)
        let (player_hand, available_bids, can_bid, can_pass) =
            if let Some(position) = player_position {
                // Player - get their hand and calculate bids
                if let Some(hand) =
                    Self::get_player_hand(game_id, position as usize, redis_conn).await?
                {
                    let current_highest = game_match.highest_bid_length;
                    let available_bids = hand.get_available_bids(current_highest);
                    let is_turn = position == current_bidder;

                    let player_hand_data = PlayerHand {
                        cards: hand.to_codes(),
                        trump_counts: hand.calculate_trump_counts(),
                        position,
                    };

                    (
                        Some(player_hand_data),
                        available_bids.clone(),
                        is_turn && !available_bids.is_empty(),
                        is_turn,
                    )
                } else {
                    // Hand not found for player
                    (None, Vec::new(), false, false)
                }
            } else {
                // Spectator - no hand data
                (None, Vec::new(), false, false)
            };

        // Get highest bid info
        let highest_bid = Self::get_current_highest_bid(game_id, redis_conn).await?;

        // Build bidding history (simplified - just current highest bid for now)
        let bidding_history = Self::build_bidding_history(game_id, redis_conn).await?;

        Ok(BiddingStateData {
            common: common_state,
            dealer_position,
            current_bidder,
            player_hand,
            available_bids: available_bids
                .into_iter()
                .map(|bid| BidOption {
                    length: bid.length,
                    suit: bid.suit,
                    display_text: bid.display_text,
                    is_club_declaration: bid.is_club_declaration,
                })
                .collect(),
            highest_bid,
            bidding_history,
            can_bid,
            can_pass,
        })
    }

    /// Get player's hand from Redis
    async fn get_player_hand(
        game_id: &str,
        player_position: usize,
        redis_conn: &mut Connection,
    ) -> Result<Option<crate::game::hand::Hand>, Box<dyn std::error::Error + Send + Sync>> {
        use crate::redis::game_state::repository::GameStateRepository;

        match GameStateRepository::get_hand(redis_conn, game_id, player_position).await {
            Ok(hand) => Ok(hand),
            Err(e) => {
                eprintln!("Failed to get hand for player {}: {}", player_position, e);
                Ok(None)
            }
        }
    }

    /// Get current highest bid information
    async fn get_current_highest_bid(
        game_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<Option<BidInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let game_match = NormalMatchRepository::get_by_id(redis_conn, game_id)
            .await?
            .ok_or("Game not found")?;

        if let (Some(length), Some(bidder), Some(suit)) = (
            game_match.highest_bid_length,
            game_match.highest_bidder,
            game_match.highest_bid_suit,
        ) {
            // Get bidder username
            let username =
                Self::get_player_username_by_position(game_id, bidder, redis_conn).await?;

            Ok(Some(BidInfo {
                length,
                suit: suit.clone(),
                bidder: bidder as u8,
                bidder_username: username,
                is_club_declaration: suit == "clubs",
            }))
        } else {
            Ok(None)
        }
    }

    /// Get username for a player by their position
    async fn get_player_username_by_position(
        game_id: &str,
        player_position: usize,
        redis_conn: &mut Connection,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        use crate::redis::player::repository::PlayerRepository;

        let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;

        if player_position < players.len() {
            let user_id = &players[player_position].user_id;
            let username: String = redis::cmd("HGET")
                .arg("usernames")
                .arg(user_id)
                .query_async(redis_conn)
                .await
                .unwrap_or_else(|_| "Unknown Player".to_string());
            Ok(username)
        } else {
            Ok("Unknown Player".to_string())
        }
    }

    /// Build bidding history (simplified for now)
    async fn build_bidding_history(
        game_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<Vec<BidHistoryEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let mut history = Vec::new();

        // For now, just include the current highest bid if one exists
        // This could be enhanced to store full bidding history in Redis
        if let Some(current_bid) = Self::get_current_highest_bid(game_id, redis_conn).await? {
            history.push(BidHistoryEntry {
                player: current_bid.bidder,
                username: current_bid.bidder_username.clone(),
                action: "bid".to_string(),
                bid_info: Some(current_bid),
                timestamp: crate::websocket::timestamp::TimestampManager::now(),
            });
        }

        Ok(history)
    }
}
