use crate::redis::normal_match::id::{NormalMatch, NormalMatchStatus};
use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::redis::player::repository::PlayerRepository;
use crate::websocket::timestamp::TimestampManager;
use crate::websocket::types::*;
use deadpool_redis::Connection;

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
                let state =
                    Self::build_playing_state(game_id, user_id, timestamp, redis_conn).await?;
                Ok(GameMessage::new(
                    "initial_state_playing".to_string(),
                    serde_json::to_value(&state)?,
                )
                .with_game_id(game_id.to_string())
                .with_phase("playing".to_string())
                .with_timestamp(timestamp))
            }
            NormalMatchStatus::Completed => {
                let state =
                    Self::build_completed_state(game_id, user_id, timestamp, redis_conn).await?;
                Ok(GameMessage::new(
                    "initial_state_completed".to_string(),
                    serde_json::to_value(&state)?,
                )
                .with_game_id(game_id.to_string())
                .with_phase("completed".to_string())
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

    /// Build playing phase state with player hand, legal cards, and trick context
    pub async fn build_playing_state(
        game_id: &str,
        user_id: &str,
        timestamp: i64,
        redis_conn: &mut Connection,
    ) -> Result<PlayingStateData, Box<dyn std::error::Error + Send + Sync>> {
        // Build common state
        let common_state = Self::build_common_state(game_id, timestamp, redis_conn).await?;

        // Get game match for trump information
        let game_match = NormalMatchRepository::get_by_id(redis_conn, game_id)
            .await?
            .ok_or("Game not found")?;

        // Get trump info
        let trump_info = Self::build_trump_info(&game_match, redis_conn).await?;

        // Get current trick state from Redis
        let trick_state = Self::get_current_trick_state(game_id, redis_conn).await?;

        // Get player position to determine permissions and data access
        let player_position = Self::get_player_position(game_id, user_id, redis_conn)
            .await
            .ok();

        // Get player's hand and legal cards (only for actual players)
        let (player_hand, legal_cards) = if let Some(position) = player_position {
            if let Some(hand) =
                Self::get_player_hand(game_id, position as usize, redis_conn).await?
            {
                let player_hand_data = PlayerHand {
                    cards: hand.to_codes(),
                    trump_counts: hand.calculate_trump_counts(),
                    position,
                };

                // Calculate legal cards if it's this player's turn
                let legal_cards = if position == trick_state.current_player.unwrap_or(99) {
                    // Need to use the game trick state to calculate legal cards
                    Self::calculate_legal_cards(game_id, &hand.cards, redis_conn)
                        .await
                        .unwrap_or_else(|_| Vec::new())
                        .into_iter()
                        .map(|card| card.to_string())
                        .collect()
                } else {
                    Vec::new() // Not their turn, no legal cards needed
                };

                (Some(player_hand_data), legal_cards)
            } else {
                // Hand not found for player
                (None, Vec::new())
            }
        } else {
            // Spectator - no hand data or legal cards
            (None, Vec::new())
        };

        // Build score state from current game progress
        let score_state = Self::build_score_state(game_id, redis_conn).await?;

        // Build turn info
        let turn_info = Self::build_turn_info(&trick_state, player_position, redis_conn).await?;

        Ok(PlayingStateData {
            common: common_state,
            trump_info,
            player_hand,
            legal_cards,
            current_trick: trick_state,
            score_state,
            turn_info,
        })
    }

    /// Build trump information from game match data
    async fn build_trump_info(
        game_match: &NormalMatch,
        redis_conn: &mut Connection,
    ) -> Result<TrumpInfo, Box<dyn std::error::Error + Send + Sync>> {
        let trump_suit = game_match.trump_suit.clone().ok_or("Trump suit not set")?;
        let trump_declarer = game_match.trump_declarer.ok_or("Trump declarer not set")? as u8;

        // Get trump declarer username
        let trump_declarer_username = Self::get_player_username_by_position(
            &game_match.id,
            trump_declarer as usize,
            redis_conn,
        )
        .await?;

        // Build partnerships (trump declarer + opposite player vs other two)
        let partnership =
            Self::build_partnership(game_match, trump_declarer as usize, redis_conn).await?;

        Ok(TrumpInfo {
            trump_suit,
            trump_declarer,
            trump_declarer_username,
            partnership,
        })
    }

    /// Build partnership information
    async fn build_partnership(
        game_match: &NormalMatch,
        trump_declarer: usize,
        redis_conn: &mut Connection,
    ) -> Result<Partnership, Box<dyn std::error::Error + Send + Sync>> {
        use crate::redis::player::repository::PlayerRepository;

        let players = PlayerRepository::get_players_in_game(redis_conn, &game_match.id).await?;

        // Traditional Sjavs partnerships: trump declarer + opposite player
        let partner_position = (trump_declarer + 2) % 4;
        let trump_team_positions = vec![trump_declarer, partner_position];
        let opponent_team_positions = vec![(trump_declarer + 1) % 4, (trump_declarer + 3) % 4];

        let mut trump_team = Vec::new();
        let mut opponent_team = Vec::new();

        // Build trump team player info
        for pos in trump_team_positions {
            if pos < players.len() {
                let username = Self::get_username(&players[pos].user_id, redis_conn).await?;
                trump_team.push(PlayerInfo {
                    user_id: players[pos].user_id.clone(),
                    username,
                    position: Some(pos as u8),
                    role: players[pos].role.clone(),
                });
            }
        }

        // Build opponent team player info
        for pos in opponent_team_positions {
            if pos < players.len() {
                let username = Self::get_username(&players[pos].user_id, redis_conn).await?;
                opponent_team.push(PlayerInfo {
                    user_id: players[pos].user_id.clone(),
                    username,
                    position: Some(pos as u8),
                    role: players[pos].role.clone(),
                });
            }
        }

        Ok(Partnership {
            trump_team,
            opponent_team,
        })
    }

    /// Get current trick state from Redis
    async fn get_current_trick_state(
        game_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<TrickState, Box<dyn std::error::Error + Send + Sync>> {
        use crate::redis::trick_state::TrickStateRepository;

        let game_trick_state = TrickStateRepository::get_trick_state(redis_conn, game_id)
            .await?
            .ok_or("No active trick state found")?;

        // Convert cards_played from (usize, Card) to CardPlay format
        let cards_played = game_trick_state
            .current_trick
            .cards_played
            .into_iter()
            .map(|(player_pos, card)| CardPlay {
                player: player_pos as u8,
                username: "Player".to_string(), // Could enhance with actual usernames
                card: card.to_string(),
                timestamp: crate::websocket::timestamp::TimestampManager::now(),
            })
            .collect();

        Ok(TrickState {
            trick_number: game_trick_state.current_trick.trick_number,
            cards_played,
            current_player: if game_trick_state.current_trick.is_complete {
                None
            } else {
                Some(game_trick_state.current_trick.current_player as u8)
            },
            leader: game_trick_state.current_trick.current_player as u8, // Could be enhanced to track actual leader
            is_complete: game_trick_state.current_trick.is_complete,
            winner: game_trick_state.current_trick.trick_winner.map(|w| w as u8),
        })
    }

    /// Build current score state
    async fn build_score_state(
        game_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<ScoreState, Box<dyn std::error::Error + Send + Sync>> {
        use crate::redis::trick_state::TrickStateRepository;

        let game_trick_state = TrickStateRepository::get_trick_state(redis_conn, game_id)
            .await?
            .ok_or("No trick state found")?;

        Ok(ScoreState {
            trump_team_tricks: game_trick_state.tricks_won.0,
            opponent_team_tricks: game_trick_state.tricks_won.1,
            trump_team_points: game_trick_state.points_accumulated.0,
            opponent_team_points: game_trick_state.points_accumulated.1,
            tricks_remaining: 8 - (game_trick_state.tricks_won.0 + game_trick_state.tricks_won.1),
        })
    }

    /// Build turn information
    async fn build_turn_info(
        trick_state: &TrickState,
        player_position: Option<u8>,
        redis_conn: &mut Connection,
    ) -> Result<TurnInfo, Box<dyn std::error::Error + Send + Sync>> {
        let current_player = trick_state.current_player.unwrap_or(0);
        let current_player_username = format!("Player {}", current_player + 1); // Could enhance with actual username lookup

        let is_your_turn = player_position.map_or(false, |pos| pos == current_player);

        Ok(TurnInfo {
            current_player,
            current_player_username,
            is_your_turn,
        })
    }

    /// Calculate legal cards for a player using the actual game trick state
    async fn calculate_legal_cards(
        game_id: &str,
        player_cards: &[crate::game::card::Card],
        redis_conn: &mut Connection,
    ) -> Result<Vec<crate::game::card::Card>, Box<dyn std::error::Error + Send + Sync>> {
        use crate::redis::trick_state::TrickStateRepository;

        // Get the actual game trick state which has the get_legal_cards method
        let game_trick_state = TrickStateRepository::get_trick_state(redis_conn, game_id)
            .await?
            .ok_or("No trick state found")?;

        // Use the game trick state to calculate legal cards
        Ok(game_trick_state.current_trick.get_legal_cards(player_cards))
    }

    /// Build completed phase state with final results, cross scores, and winner information
    pub async fn build_completed_state(
        game_id: &str,
        user_id: &str,
        timestamp: i64,
        redis_conn: &mut Connection,
    ) -> Result<CompletedStateData, Box<dyn std::error::Error + Send + Sync>> {
        // Build common state
        let common_state = Self::build_common_state(game_id, timestamp, redis_conn).await?;

        // Get game match to ensure it's completed
        let game_match = NormalMatchRepository::get_by_id(redis_conn, game_id)
            .await?
            .ok_or("Game not found")?;

        // Verify game is actually completed
        if !matches!(game_match.status, NormalMatchStatus::Completed) {
            return Err("Game is not in completed state".into());
        }

        // Build final scoring results
        let final_scores = Self::get_final_game_results(game_id, &game_match, redis_conn).await?;

        // Get cross/rubber scores
        let cross_scores = Self::get_cross_scores(game_id, redis_conn).await?;

        // Build winner information
        let winner_info = Self::build_winner_info(&final_scores, &game_match, redis_conn).await?;

        // Check if new game can be started (host decision and rubber not complete)
        let can_start_new_game =
            Self::can_start_new_game(game_id, user_id, &cross_scores, redis_conn).await?;

        Ok(CompletedStateData {
            common: common_state,
            final_scores,
            cross_scores,
            winner_info,
            can_start_new_game,
        })
    }

    /// Get final game results from stored game data
    async fn get_final_game_results(
        game_id: &str,
        game_match: &NormalMatch,
        redis_conn: &mut Connection,
    ) -> Result<GameResult, Box<dyn std::error::Error + Send + Sync>> {
        // Try to get stored game results first
        if let Ok(Some(stored_result)) = Self::get_stored_game_result(game_id, redis_conn).await {
            return Ok(stored_result);
        }

        // If no stored result, try to reconstruct from game state
        // This handles edge cases where the game completed but results weren't stored
        let trump_suit = game_match
            .trump_suit
            .as_ref()
            .ok_or("No trump suit found in completed game")?;
        let trump_declarer = game_match
            .trump_declarer
            .ok_or("No trump declarer found in completed game")? as u8;

        // Get trump declarer username
        let trump_declarer_username =
            Self::get_player_username_by_position(game_id, trump_declarer as usize, redis_conn)
                .await?;

        // Create a fallback result (this shouldn't normally happen)
        Ok(GameResult {
            result_type: "completed".to_string(),
            description: "Game completed - results unavailable".to_string(),
            trump_team_score: 0,
            opponent_team_score: 0,
            individual_vol: false,
        })
    }

    /// Get stored game result from Redis
    async fn get_stored_game_result(
        game_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<Option<GameResult>, Box<dyn std::error::Error + Send + Sync>> {
        // Try to get the stored game result
        let result_key = format!("game_result:{}", game_id);
        let result_data: Option<String> = redis::cmd("GET")
            .arg(&result_key)
            .query_async(redis_conn)
            .await
            .unwrap_or(None);

        if let Some(data) = result_data {
            match serde_json::from_str::<GameResult>(&data) {
                Ok(result) => Ok(Some(result)),
                Err(_) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// Get cross/rubber scores
    async fn get_cross_scores(
        game_id: &str,
        redis_conn: &mut Connection,
    ) -> Result<CrossScores, Box<dyn std::error::Error + Send + Sync>> {
        use crate::redis::cross_state::repository::CrossStateRepository;

        // Try to get cross state from Redis
        match CrossStateRepository::get_cross_state(redis_conn, game_id).await {
            Ok(Some(cross_state)) => Ok(CrossScores {
                trump_team_remaining: cross_state.trump_team_score,
                opponent_team_remaining: cross_state.opponent_team_score,
                trump_team_crosses: cross_state.trump_team_crosses,
                opponent_team_crosses: cross_state.opponent_team_crosses,
            }),
            Ok(None) => {
                // No cross state found - create default scores
                Ok(CrossScores {
                    trump_team_remaining: 24,
                    opponent_team_remaining: 24,
                    trump_team_crosses: 0,
                    opponent_team_crosses: 0,
                })
            }
            Err(e) => {
                eprintln!("Failed to get cross state: {}", e);
                // Return default scores on error
                Ok(CrossScores {
                    trump_team_remaining: 24,
                    opponent_team_remaining: 24,
                    trump_team_crosses: 0,
                    opponent_team_crosses: 0,
                })
            }
        }
    }

    /// Build winner information from game results
    async fn build_winner_info(
        final_scores: &GameResult,
        game_match: &NormalMatch,
        redis_conn: &mut Connection,
    ) -> Result<Option<WinnerInfo>, Box<dyn std::error::Error + Send + Sync>> {
        // Determine winning team based on final scores
        let trump_team_won = final_scores.trump_team_score > final_scores.opponent_team_score;

        if trump_team_won {
            // Trump team won
            let trump_declarer =
                game_match.trump_declarer.ok_or("No trump declarer found")? as usize;
            let partner_position = (trump_declarer + 2) % 4;

            let mut winning_players = Vec::new();
            let players = PlayerRepository::get_players_in_game(redis_conn, &game_match.id).await?;

            // Add trump declarer
            if trump_declarer < players.len() {
                let username =
                    Self::get_username(&players[trump_declarer].user_id, redis_conn).await?;
                winning_players.push(PlayerInfo {
                    user_id: players[trump_declarer].user_id.clone(),
                    username,
                    position: Some(trump_declarer as u8),
                    role: players[trump_declarer].role.clone(),
                });
            }

            // Add partner
            if partner_position < players.len() {
                let username =
                    Self::get_username(&players[partner_position].user_id, redis_conn).await?;
                winning_players.push(PlayerInfo {
                    user_id: players[partner_position].user_id.clone(),
                    username,
                    position: Some(partner_position as u8),
                    role: players[partner_position].role.clone(),
                });
            }

            Ok(Some(WinnerInfo {
                winning_team: "trump_team".to_string(),
                winning_players,
                double_victory: false, // Could be enhanced to detect double victories
            }))
        } else {
            // Opponent team won
            let trump_declarer =
                game_match.trump_declarer.ok_or("No trump declarer found")? as usize;
            let opponent_positions = vec![(trump_declarer + 1) % 4, (trump_declarer + 3) % 4];

            let mut winning_players = Vec::new();
            let players = PlayerRepository::get_players_in_game(redis_conn, &game_match.id).await?;

            for pos in opponent_positions {
                if pos < players.len() {
                    let username = Self::get_username(&players[pos].user_id, redis_conn).await?;
                    winning_players.push(PlayerInfo {
                        user_id: players[pos].user_id.clone(),
                        username,
                        position: Some(pos as u8),
                        role: players[pos].role.clone(),
                    });
                }
            }

            Ok(Some(WinnerInfo {
                winning_team: "opponent_team".to_string(),
                winning_players,
                double_victory: false, // Could be enhanced to detect double victories
            }))
        }
    }

    /// Check if a new game can be started
    async fn can_start_new_game(
        game_id: &str,
        user_id: &str,
        cross_scores: &CrossScores,
        redis_conn: &mut Connection,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Check if user is the host
        let is_host = Self::is_host(game_id, user_id, redis_conn).await?;

        // Check if rubber is complete (either team has won the rubber)
        let rubber_complete =
            cross_scores.trump_team_remaining <= 0 || cross_scores.opponent_team_remaining <= 0;

        // Can start new game if:
        // 1. User is the host AND
        // 2. Rubber is not complete (still games to play)
        Ok(is_host && !rubber_complete)
    }
}
