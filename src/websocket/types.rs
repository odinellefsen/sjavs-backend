use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameMessage {
    pub event: String,
    pub data: serde_json::Value,
    /// Timestamp for ordering (milliseconds since epoch)
    pub timestamp: i64,
    /// Game ID for routing
    pub game_id: Option<String>,
    /// Phase-specific event subtype
    pub phase: Option<String>,
}

impl GameMessage {
    pub fn new(event: String, data: serde_json::Value) -> Self {
        Self {
            event,
            data,
            timestamp: Utc::now().timestamp_millis(),
            game_id: None,
            phase: None,
        }
    }

    pub fn with_game_id(mut self, game_id: String) -> Self {
        self.game_id = Some(game_id);
        self
    }

    pub fn with_phase(mut self, phase: String) -> Self {
        self.phase = Some(phase);
        self
    }

    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }
}

/// Phase-specific initial state events
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum InitialStateEvent {
    Waiting(WaitingStateData),
    Dealing(DealingStateData),
    Bidding(BiddingStateData),
    Playing(PlayingStateData),
    Completed(CompletedStateData),
}

/// Common state data shared across all phases
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommonStateData {
    pub game_id: String,
    pub match_info: MatchInfo,
    pub players: Vec<PlayerInfo>,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MatchInfo {
    pub id: String,
    pub pin: u32,
    pub status: String,
    pub number_of_crosses: u32,
    pub current_cross: u32,
    pub created_timestamp: u64,
    pub host: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerInfo {
    pub user_id: String,
    pub username: String,
    pub position: Option<u8>,
    pub role: String,
}

/// Waiting phase state (players joining)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WaitingStateData {
    pub common: CommonStateData,
    pub can_start_game: bool,
    pub players_needed: u8,
    pub is_host: bool,
}

/// Dealing phase state (cards being dealt)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DealingStateData {
    pub common: CommonStateData,
    pub dealer_position: u8,
    pub dealing_progress: String, // "dealing", "validating", "complete"
}

/// Bidding phase state (trump selection)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BiddingStateData {
    pub common: CommonStateData,
    pub dealer_position: u8,
    pub current_bidder: u8,
    pub player_hand: Option<PlayerHand>, // Only for requesting player
    pub available_bids: Vec<BidOption>,
    pub highest_bid: Option<BidInfo>,
    pub bidding_history: Vec<BidHistoryEntry>,
    pub can_bid: bool,
    pub can_pass: bool,
}

/// Playing phase state (trick-taking)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayingStateData {
    pub common: CommonStateData,
    pub trump_info: TrumpInfo,
    pub player_hand: Option<PlayerHand>, // Only for requesting player
    pub legal_cards: Vec<String>,
    pub current_trick: TrickState,
    pub score_state: ScoreState,
    pub turn_info: TurnInfo,
}

/// Completed phase state (game finished)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompletedStateData {
    pub common: CommonStateData,
    pub final_scores: GameResult,
    pub cross_scores: CrossScores,
    pub winner_info: Option<WinnerInfo>,
    pub can_start_new_game: bool,
}

// Supporting structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerHand {
    pub cards: Vec<String>,
    pub trump_counts: HashMap<String, u8>,
    pub position: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BidOption {
    pub length: u8,
    pub suit: String,
    pub display_text: String,
    pub is_club_declaration: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BidInfo {
    pub length: u8,
    pub suit: String,
    pub bidder: u8,
    pub bidder_username: String,
    pub is_club_declaration: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BidHistoryEntry {
    pub player: u8,
    pub username: String,
    pub action: String, // "bid" or "pass"
    pub bid_info: Option<BidInfo>,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TrumpInfo {
    pub trump_suit: String,
    pub trump_declarer: u8,
    pub trump_declarer_username: String,
    pub partnership: Partnership,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Partnership {
    pub trump_team: Vec<PlayerInfo>,
    pub opponent_team: Vec<PlayerInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TrickState {
    pub trick_number: u8,
    pub cards_played: Vec<CardPlay>,
    pub current_player: Option<u8>,
    pub leader: u8,
    pub is_complete: bool,
    pub winner: Option<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CardPlay {
    pub player: u8,
    pub username: String,
    pub card: String,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScoreState {
    pub trump_team_tricks: u8,
    pub opponent_team_tricks: u8,
    pub trump_team_points: u8,
    pub opponent_team_points: u8,
    pub tricks_remaining: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TurnInfo {
    pub current_player: u8,
    pub current_player_username: String,
    pub is_your_turn: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameResult {
    pub result_type: String,
    pub description: String,
    pub trump_team_score: u8,
    pub opponent_team_score: u8,
    pub individual_vol: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CrossScores {
    pub trump_team_remaining: i8,
    pub opponent_team_remaining: i8,
    pub trump_team_crosses: u8,
    pub opponent_team_crosses: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WinnerInfo {
    pub winning_team: String,
    pub winning_players: Vec<PlayerInfo>,
    pub double_victory: bool,
}
