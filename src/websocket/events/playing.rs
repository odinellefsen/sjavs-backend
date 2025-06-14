use serde::{Deserialize, Serialize};

/// WebSocket events for trick-taking phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrickEvent {
    /// A card was played in the current trick
    CardPlayed {
        game_id: String,
        player_position: u8,
        card: String,
        trick_number: u8,
        cards_in_trick: u8,
        next_player: Option<u8>,
        trick_complete: bool,
        trick_winner: Option<u8>,
        points_won: u8,
    },

    /// A trick was completed
    TrickCompleted {
        game_id: String,
        trick_number: u8,
        winner: u8,
        points: u8,
        trump_team_score: u8,
        opponent_team_score: u8,
        game_complete: bool,
    },
}

impl TrickEvent {
    /// Get the game ID for this event
    pub fn game_id(&self) -> &str {
        match self {
            TrickEvent::CardPlayed { game_id, .. } => game_id,
            TrickEvent::TrickCompleted { game_id, .. } => game_id,
        }
    }
}
