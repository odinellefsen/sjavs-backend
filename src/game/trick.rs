use crate::game::card::{Card, Suit};
use serde::{Deserialize, Serialize};

/// Current state of a trick in progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrickState {
    /// Current trick number (1-8)
    pub trick_number: u8,

    /// The suit led by the first card (determines follow suit requirements)
    pub lead_suit: Option<Suit>,

    /// Cards played in this trick: (player_position, card)
    pub cards_played: Vec<(usize, Card)>,

    /// Position of player whose turn it is to play
    pub current_player: usize,

    /// Winner of this trick (None until trick is complete)
    pub trick_winner: Option<usize>,

    /// Whether all 4 cards have been played
    pub is_complete: bool,

    /// Game ID this trick belongs to
    pub game_id: String,

    /// Trump suit for this game
    pub trump_suit: String,
}

impl TrickState {
    /// Create a new trick with the initial leader
    pub fn new(
        game_id: String,
        trick_number: u8,
        initial_leader: usize,
        trump_suit: String,
    ) -> Self {
        Self {
            trick_number,
            lead_suit: None,
            cards_played: Vec::new(),
            current_player: initial_leader,
            trick_winner: None,
            is_complete: false,
            game_id,
            trump_suit,
        }
    }

    /// Play a card to this trick
    pub fn play_card(&mut self, player_position: usize, card: Card) -> Result<(), String> {
        // Validate it's the player's turn
        if player_position != self.current_player {
            return Err(format!(
                "Not your turn. Current player is {}",
                self.current_player
            ));
        }

        // Validate trick isn't complete
        if self.is_complete {
            return Err("Trick is already complete".to_string());
        }

        // Validate max 4 cards
        if self.cards_played.len() >= 4 {
            return Err("Trick already has 4 cards".to_string());
        }

        // Set lead suit if this is the first card
        if self.cards_played.is_empty() {
            self.lead_suit = Some(card.suit);
        }

        // Add the card
        self.cards_played.push((player_position, card));

        // Update current player (or mark complete)
        if self.cards_played.len() == 4 {
            self.is_complete = true;
            self.trick_winner = Some(self.determine_winner());
        } else {
            self.current_player = (self.current_player + 1) % 4;
        }

        Ok(())
    }

    /// Determine the winner of this trick
    fn determine_winner(&self) -> usize {
        if self.cards_played.len() != 4 {
            panic!("Cannot determine winner of incomplete trick");
        }

        let trump_suit = Suit::from(self.trump_suit.as_str());
        let lead_suit = self.lead_suit.expect("Lead suit should be set");

        let mut best_player = self.cards_played[0].0;
        let mut best_card = &self.cards_played[0].1;

        for (player, card) in &self.cards_played[1..] {
            if card.beats(best_card, trump_suit, lead_suit) {
                best_player = *player;
                best_card = card;
            }
        }

        best_player
    }

    /// Get cards that can legally be played by a player
    pub fn get_legal_cards(&self, player_hand: &[Card]) -> Vec<Card> {
        if let Some(lead_suit) = self.lead_suit {
            // Must follow suit if possible
            let same_suit_cards: Vec<Card> = player_hand
                .iter()
                .filter(|card| card.suit == lead_suit)
                .cloned()
                .collect();

            if !same_suit_cards.is_empty() {
                // Must follow suit
                same_suit_cards
            } else {
                // Can play any card if can't follow suit
                player_hand.to_vec()
            }
        } else {
            // First card of trick - can play any card
            player_hand.to_vec()
        }
    }

    /// Get summary of current trick for display
    pub fn get_summary(&self) -> TrickSummary {
        TrickSummary {
            trick_number: self.trick_number,
            cards_played: self.cards_played.len() as u8,
            current_player: if self.is_complete {
                None
            } else {
                Some(self.current_player)
            },
            lead_suit: self.lead_suit.map(|s| s.to_string()),
            is_complete: self.is_complete,
            winner: self.trick_winner,
        }
    }

    /// Calculate points won in this trick
    pub fn calculate_points(&self) -> u8 {
        self.cards_played
            .iter()
            .map(|(_, card)| card.point_value())
            .sum()
    }

    /// Reset for a new trick
    pub fn start_next_trick(&mut self, new_leader: usize) {
        self.trick_number += 1;
        self.lead_suit = None;
        self.cards_played.clear();
        self.current_player = new_leader;
        self.trick_winner = None;
        self.is_complete = false;
    }
}

/// Summary of trick state for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrickSummary {
    pub trick_number: u8,
    pub cards_played: u8,
    pub current_player: Option<usize>,
    pub lead_suit: Option<String>,
    pub is_complete: bool,
    pub winner: Option<usize>,
}

/// Game-wide trick tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTrickState {
    /// Current trick being played
    pub current_trick: TrickState,

    /// Tricks won by each team: (trump_team_tricks, opponent_team_tricks)
    pub tricks_won: (u8, u8),

    /// Points accumulated by each team: (trump_team_points, opponent_team_points)
    pub points_accumulated: (u8, u8),

    /// All completed tricks (for history/debugging)
    pub completed_tricks: Vec<TrickState>,

    /// Trump declaring team (trump_declarer, partner)
    pub trump_team: (usize, usize),

    /// Whether all 8 tricks are complete
    pub game_complete: bool,
}

impl GameTrickState {
    /// Create new game trick state
    pub fn new(
        game_id: String,
        initial_leader: usize,
        trump_suit: String,
        trump_team: (usize, usize),
    ) -> Self {
        Self {
            current_trick: TrickState::new(game_id, 1, initial_leader, trump_suit),
            tricks_won: (0, 0),
            points_accumulated: (0, 0),
            completed_tricks: Vec::new(),
            trump_team,
            game_complete: false,
        }
    }

    /// Complete current trick and start next one
    pub fn complete_trick(&mut self) -> Result<TrickCompletionResult, String> {
        if !self.current_trick.is_complete {
            return Err("Current trick is not complete".to_string());
        }

        let winner = self.current_trick.trick_winner.unwrap();
        let points = self.current_trick.calculate_points();

        // Determine which team won the trick
        let trump_team_won = self.trump_team.0 == winner || self.trump_team.1 == winner;

        // Update counters
        if trump_team_won {
            self.tricks_won.0 += 1;
            self.points_accumulated.0 += points;
        } else {
            self.tricks_won.1 += 1;
            self.points_accumulated.1 += points;
        }

        // Store completed trick
        self.completed_tricks.push(self.current_trick.clone());

        // Check if game is complete
        if self.current_trick.trick_number == 8 {
            self.game_complete = true;
            return Ok(TrickCompletionResult {
                winner,
                points,
                trump_team_won,
                game_complete: true,
                next_leader: None,
            });
        }

        // Start next trick with winner as leader
        self.current_trick.start_next_trick(winner);

        Ok(TrickCompletionResult {
            winner,
            points,
            trump_team_won,
            game_complete: false,
            next_leader: Some(winner),
        })
    }

    /// Check if individual vol was achieved (single player won all tricks)
    pub fn check_individual_vol(&self) -> bool {
        if self.tricks_won.0 != 8 && self.tricks_won.1 != 8 {
            return false; // No vol at all
        }

        // Check if all tricks were won by a single player
        let trump_team_players = [self.trump_team.0, self.trump_team.1];
        let mut trick_winners: Vec<usize> = Vec::new();

        for trick in &self.completed_tricks {
            if let Some(winner) = trick.trick_winner {
                trick_winners.push(winner);
            }
        }

        // If trump team won all tricks, check if single player did it
        if self.tricks_won.0 == 8 {
            let trump_player_0_tricks = trick_winners
                .iter()
                .filter(|&&w| w == trump_team_players[0])
                .count();
            let trump_player_1_tricks = trick_winners
                .iter()
                .filter(|&&w| w == trump_team_players[1])
                .count();

            trump_player_0_tricks == 8 || trump_player_1_tricks == 8
        } else {
            false // Opponent team vol is never individual vol
        }
    }

    /// Get complete game scoring data
    pub fn get_final_scoring(&self) -> Result<crate::game::scoring::SjavsScoring, String> {
        if !self.game_complete {
            return Err("Game not yet complete".to_string());
        }

        Ok(crate::game::scoring::SjavsScoring {
            trump_team_points: self.points_accumulated.0,
            opponent_team_points: self.points_accumulated.1,
            trump_team_tricks: self.tricks_won.0,
            opponent_team_tricks: self.tricks_won.1,
            trump_suit: self.current_trick.trump_suit.clone(),
            individual_vol: self.check_individual_vol(),
        })
    }
}

/// Result of completing a trick
#[derive(Debug, Clone)]
pub struct TrickCompletionResult {
    pub winner: usize,
    pub points: u8,
    pub trump_team_won: bool,
    pub game_complete: bool,
    pub next_leader: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::{Rank, Suit};

    #[test]
    fn test_trick_creation() {
        let trick = TrickState::new("game123".to_string(), 1, 0, "hearts".to_string());

        assert_eq!(trick.trick_number, 1);
        assert_eq!(trick.current_player, 0);
        assert!(!trick.is_complete);
        assert!(trick.lead_suit.is_none());
        assert_eq!(trick.cards_played.len(), 0);
    }

    #[test]
    fn test_play_card() {
        let mut trick = TrickState::new("game123".to_string(), 1, 0, "hearts".to_string());

        let card = Card::new(Suit::Spades, Rank::Ace);

        // First card should set lead suit
        assert!(trick.play_card(0, card).is_ok());
        assert_eq!(trick.lead_suit, Some(Suit::Spades));
        assert_eq!(trick.current_player, 1);
        assert!(!trick.is_complete);
    }

    #[test]
    fn test_complete_trick() {
        let mut trick = TrickState::new("game123".to_string(), 1, 0, "hearts".to_string());

        // Play 4 cards
        trick
            .play_card(0, Card::new(Suit::Spades, Rank::Seven))
            .unwrap();
        trick
            .play_card(1, Card::new(Suit::Spades, Rank::Eight))
            .unwrap();
        trick
            .play_card(2, Card::new(Suit::Spades, Rank::Nine))
            .unwrap();
        trick
            .play_card(3, Card::new(Suit::Spades, Rank::Ace))
            .unwrap();

        assert!(trick.is_complete);
        assert_eq!(trick.trick_winner, Some(3)); // Ace wins
    }

    #[test]
    fn test_follow_suit_validation() {
        let trick = TrickState {
            trick_number: 1,
            lead_suit: Some(Suit::Hearts),
            cards_played: vec![(0, Card::new(Suit::Hearts, Rank::Seven))],
            current_player: 1,
            trick_winner: None,
            is_complete: false,
            game_id: "test".to_string(),
            trump_suit: "spades".to_string(),
        };

        let hand = vec![
            Card::new(Suit::Hearts, Rank::Ace),  // Must play this
            Card::new(Suit::Spades, Rank::King), // Can't play this
        ];

        let legal_cards = trick.get_legal_cards(&hand);
        assert_eq!(legal_cards.len(), 1);
        assert_eq!(legal_cards[0].suit, Suit::Hearts);
    }
}
