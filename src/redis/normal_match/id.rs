use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Status of a normal match
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalMatchStatus {
    Waiting,   // Players joining, waiting to start
    Dealing,   // Cards being dealt to players
    Bidding,   // Trump suit selection phase
    Playing,   // Main game (trick-taking)
    Completed, // Game finished
    Cancelled, // Game cancelled
}

impl ToString for NormalMatchStatus {
    fn to_string(&self) -> String {
        match self {
            NormalMatchStatus::Waiting => "waiting".to_string(),
            NormalMatchStatus::Dealing => "dealing".to_string(),
            NormalMatchStatus::Bidding => "bidding".to_string(),
            NormalMatchStatus::Playing => "playing".to_string(),
            NormalMatchStatus::Completed => "completed".to_string(),
            NormalMatchStatus::Cancelled => "cancelled".to_string(),
        }
    }
}

impl From<&str> for NormalMatchStatus {
    fn from(s: &str) -> Self {
        match s {
            "waiting" => NormalMatchStatus::Waiting,
            "dealing" => NormalMatchStatus::Dealing,
            "bidding" => NormalMatchStatus::Bidding,
            "playing" => NormalMatchStatus::Playing,
            "completed" => NormalMatchStatus::Completed,
            "cancelled" => NormalMatchStatus::Cancelled,
            _ => NormalMatchStatus::Waiting, // Default to waiting for unknown values
        }
    }
}

/// Represents a normal match stored in Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalMatch {
    pub id: String,
    pub pin: u32,
    pub status: NormalMatchStatus,
    pub number_of_crosses: u32,
    pub current_cross: u32,
    pub created_timestamp: u64,

    // Game state fields for trump selection and gameplay
    pub dealer_position: Option<usize>, // Position 0-3, None if not started
    pub current_bidder: Option<usize>,  // Current player bidding, None if not bidding
    pub current_leader: Option<usize>,  // Current trick leader, None if not playing
    pub trump_suit: Option<String>,     // "hearts", "diamonds", "clubs", "spades"
    pub trump_declarer: Option<usize>,  // Who declared trump

    // Bidding state
    pub highest_bid_length: Option<u8>, // Highest bid trump count
    pub highest_bidder: Option<usize>,  // Who has highest bid
    pub highest_bid_suit: Option<String>, // Trump suit of current highest bid
}

impl NormalMatch {
    /// Create a new normal match with default values
    pub fn new(id: String, pin: u32, number_of_crosses: u32) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

        Self {
            id,
            pin,
            status: NormalMatchStatus::Waiting,
            number_of_crosses,
            current_cross: 0,
            created_timestamp: now,
            dealer_position: None,
            current_bidder: None,
            current_leader: None,
            trump_suit: None,
            trump_declarer: None,
            highest_bid_length: None,
            highest_bidder: None,
            highest_bid_suit: None,
        }
    }

    /// Get the Redis key for this match
    pub fn redis_key(&self) -> String {
        format!("normal_match:{}", self.id)
    }

    /// Convert from Redis hash map to NormalMatch
    pub fn from_redis_hash(id: String, hash: &HashMap<String, String>) -> Result<Self, String> {
        let pin = hash
            .get("pin")
            .ok_or("Missing pin field")?
            .parse::<u32>()
            .map_err(|_| "Invalid pin format")?;

        let status = hash
            .get("status")
            .map(|s| NormalMatchStatus::from(s.as_str()))
            .unwrap_or(NormalMatchStatus::Waiting);

        let number_of_crosses = hash
            .get("number_of_crosses")
            .ok_or("Missing number_of_crosses field")?
            .parse::<u32>()
            .map_err(|_| "Invalid number_of_crosses format")?;

        let current_cross = hash
            .get("current_cross")
            .ok_or("Missing current_cross field")?
            .parse::<u32>()
            .map_err(|_| "Invalid current_cross format")?;

        let created_timestamp = hash
            .get("created_timestamp")
            .ok_or("Missing created_timestamp field")?
            .parse::<u64>()
            .map_err(|_| "Invalid created_timestamp format")?;

        // Parse optional game state fields (backward compatibility)
        let dealer_position = hash
            .get("dealer_position")
            .and_then(|s| s.parse::<usize>().ok());

        let current_bidder = hash
            .get("current_bidder")
            .and_then(|s| s.parse::<usize>().ok());

        let current_leader = hash
            .get("current_leader")
            .and_then(|s| s.parse::<usize>().ok());

        let trump_suit = hash
            .get("trump_suit")
            .filter(|s| !s.is_empty())
            .map(|s| s.clone());

        let trump_declarer = hash
            .get("trump_declarer")
            .and_then(|s| s.parse::<usize>().ok());

        let highest_bid_length = hash
            .get("highest_bid_length")
            .and_then(|s| s.parse::<u8>().ok());

        let highest_bidder = hash
            .get("highest_bidder")
            .and_then(|s| s.parse::<usize>().ok());

        let highest_bid_suit = hash
            .get("highest_bid_suit")
            .filter(|s| !s.is_empty())
            .map(|s| s.clone());

        Ok(Self {
            id,
            pin,
            status,
            number_of_crosses,
            current_cross,
            created_timestamp,
            dealer_position,
            current_bidder,
            current_leader,
            trump_suit,
            trump_declarer,
            highest_bid_length,
            highest_bidder,
            highest_bid_suit,
        })
    }

    /// Convert NormalMatch to Redis hash map
    pub fn to_redis_hash(&self) -> HashMap<String, String> {
        let mut hash = HashMap::new();
        hash.insert("id".to_string(), self.id.clone());
        hash.insert("pin".to_string(), self.pin.to_string());
        hash.insert("status".to_string(), self.status.to_string());
        hash.insert(
            "number_of_crosses".to_string(),
            self.number_of_crosses.to_string(),
        );
        hash.insert("current_cross".to_string(), self.current_cross.to_string());
        hash.insert(
            "created_timestamp".to_string(),
            self.created_timestamp.to_string(),
        );

        // Add optional game state fields
        if let Some(dealer) = self.dealer_position {
            hash.insert("dealer_position".to_string(), dealer.to_string());
        }
        if let Some(bidder) = self.current_bidder {
            hash.insert("current_bidder".to_string(), bidder.to_string());
        }
        if let Some(leader) = self.current_leader {
            hash.insert("current_leader".to_string(), leader.to_string());
        }
        if let Some(ref suit) = self.trump_suit {
            hash.insert("trump_suit".to_string(), suit.clone());
        }
        if let Some(declarer) = self.trump_declarer {
            hash.insert("trump_declarer".to_string(), declarer.to_string());
        }
        if let Some(length) = self.highest_bid_length {
            hash.insert("highest_bid_length".to_string(), length.to_string());
        }
        if let Some(bidder) = self.highest_bidder {
            hash.insert("highest_bidder".to_string(), bidder.to_string());
        }
        if let Some(ref suit) = self.highest_bid_suit {
            hash.insert("highest_bid_suit".to_string(), suit.clone());
        }

        hash
    }

    /// Check if match can start (has correct status and setup)
    pub fn can_start(&self) -> bool {
        self.status == NormalMatchStatus::Waiting
        // Note: Player count check will be added when we integrate with player tracking
    }

    /// Start the dealing phase
    pub fn start_dealing(&mut self, dealer_position: usize) {
        if self.can_start() {
            self.status = NormalMatchStatus::Dealing;
            self.dealer_position = Some(dealer_position);
            self.current_bidder = Some((dealer_position + 1) % 4); // Left of dealer bids first

            // Reset any previous game state
            self.current_leader = None;
            self.trump_suit = None;
            self.trump_declarer = None;
            self.highest_bid_length = None;
            self.highest_bidder = None;
            self.highest_bid_suit = None;
        }
    }

    /// Start the bidding phase
    pub fn start_bidding(&mut self) {
        if self.status == NormalMatchStatus::Dealing {
            self.status = NormalMatchStatus::Bidding;
        }
    }

    /// Complete bidding and move to playing
    pub fn complete_bidding(&mut self, trump_suit: String, trump_declarer: usize) {
        if self.status == NormalMatchStatus::Bidding {
            self.status = NormalMatchStatus::Playing;
            self.trump_suit = Some(trump_suit);
            self.trump_declarer = Some(trump_declarer);
            self.current_bidder = None;

            // Set first leader (left of dealer)
            if let Some(dealer) = self.dealer_position {
                self.current_leader = Some((dealer + 1) % 4);
            }
        }
    }

    /// Update bidding state
    pub fn update_bid(&mut self, bidder: usize, bid_length: u8) {
        if self.status == NormalMatchStatus::Bidding {
            self.highest_bid_length = Some(bid_length);
            self.highest_bidder = Some(bidder);
            // Move to next bidder
            self.current_bidder = Some((bidder + 1) % 4);
        }
    }

    /// Reset to initial state for re-dealing
    pub fn reset_for_redeal(&mut self) {
        if self.status == NormalMatchStatus::Bidding {
            self.status = NormalMatchStatus::Dealing;
            self.current_bidder = self.dealer_position.map(|d| (d + 1) % 4);
            self.highest_bid_length = None;
            self.highest_bidder = None;
            self.highest_bid_suit = None;
            self.trump_suit = None;
            self.trump_declarer = None;
            self.current_leader = None;
        }
    }

    /// Check if it's a specific player's turn to bid
    pub fn is_player_turn_to_bid(&self, player_position: usize) -> bool {
        self.status == NormalMatchStatus::Bidding && self.current_bidder == Some(player_position)
    }

    /// Get next player position (for turn rotation)
    pub fn next_player(&self, current_player: usize) -> usize {
        (current_player + 1) % 4
    }

    /// Initialize first trick after bidding completes
    pub fn start_first_trick(&mut self) -> Result<(), String> {
        if self.status != NormalMatchStatus::Playing {
            return Err("Game must be in Playing state to start tricks".to_string());
        }

        if self.current_leader.is_none() {
            return Err("Current leader must be set to start tricks".to_string());
        }

        // First trick starts - leader is already set in complete_bidding()
        Ok(())
    }

    /// Check if it's a player's turn to play a card
    pub fn is_player_turn_to_play(&self, player_position: usize) -> bool {
        self.status == NormalMatchStatus::Playing && self.current_leader == Some(player_position)
    }

    /// Update current leader after trick completion
    pub fn update_current_leader(&mut self, new_leader: usize) {
        if self.status == NormalMatchStatus::Playing {
            self.current_leader = Some(new_leader);
        }
    }

    /// Check if game is in active play (not waiting or completed)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            NormalMatchStatus::Dealing | NormalMatchStatus::Bidding | NormalMatchStatus::Playing
        )
    }

    /// Validate if a bid is legal
    pub fn is_valid_bid(
        &self,
        player_position: usize,
        bid_length: u8,
        bid_suit: &str,
    ) -> Result<(), String> {
        // Must be player's turn
        if !self.is_player_turn_to_bid(player_position) {
            return Err("Not your turn to bid".to_string());
        }

        // Must be in bidding state
        if self.status != NormalMatchStatus::Bidding {
            return Err("Game is not in bidding phase".to_string());
        }

        // Bid length must be 5-8
        if bid_length < 5 || bid_length > 8 {
            return Err("Bid must be between 5 and 8 trumps".to_string());
        }

        // Suit must be valid
        if !["hearts", "diamonds", "clubs", "spades"].contains(&bid_suit) {
            return Err("Invalid trump suit".to_string());
        }

        // Check if bid beats current highest bid
        if let Some(current_highest) = self.highest_bid_length {
            if bid_length > current_highest {
                // Higher bid always wins
                return Ok(());
            } else if bid_length == current_highest {
                // Same length only wins if it's clubs and current isn't clubs
                let current_suit = self.highest_bid_suit.as_deref().unwrap_or("");
                if bid_suit == "clubs" && current_suit != "clubs" {
                    // Clubs can match any other suit of same length
                    return Ok(());
                } else if bid_suit == "clubs" && current_suit == "clubs" {
                    return Err("Cannot bid clubs to match clubs".to_string());
                } else {
                    return Err(
                        "Bid must be higher than current bid, or clubs to match".to_string()
                    );
                }
            } else {
                return Err(format!(
                    "Bid must be at least {} trumps",
                    current_highest + 1
                ));
            }
        }

        Ok(())
    }

    /// Record a bid and advance game state
    pub fn make_bid(
        &mut self,
        player_position: usize,
        bid_length: u8,
        bid_suit: String,
    ) -> Result<bool, String> {
        // Validate the bid first
        self.is_valid_bid(player_position, bid_length, &bid_suit)?;

        // Update bidding state
        self.highest_bid_length = Some(bid_length);
        self.highest_bidder = Some(player_position);
        self.highest_bid_suit = Some(bid_suit);
        self.current_bidder = Some((player_position + 1) % 4);

        // Check if this completes bidding (if next 3 players pass)
        // For now, return false - completion will be determined by subsequent passes
        Ok(false)
    }

    /// Record a pass and advance game state
    pub fn make_pass(&mut self, player_position: usize) -> Result<(bool, bool), String> {
        // Must be player's turn
        if !self.is_player_turn_to_bid(player_position) {
            return Err("Not your turn to bid".to_string());
        }

        // Must be in bidding state
        if self.status != NormalMatchStatus::Bidding {
            return Err("Game is not in bidding phase".to_string());
        }

        // Move to next player
        self.current_bidder = Some((player_position + 1) % 4);

        // Check conditions for completion or redeal
        if self.highest_bidder.is_none() {
            // No bids yet, check if we've completed a full circle
            if let Some(dealer) = self.dealer_position {
                let starting_bidder = (dealer + 1) % 4;
                if self.current_bidder == Some(starting_bidder) {
                    // All 4 players passed - need redeal
                    return Ok((true, false)); // (all_passed, bidding_complete)
                }
            }
        } else {
            // There's a bid, check if 3 consecutive passes after the bid
            if let Some(bidder) = self.highest_bidder {
                let next_after_bidder = (bidder + 1) % 4;
                if self.current_bidder == Some(next_after_bidder) {
                    // We've come full circle back to player after bidder
                    // This means 3 players passed after the bid
                    return Ok((false, true)); // (all_passed, bidding_complete)
                }
            }
        }

        Ok((false, false)) // Continue bidding
    }

    /// Complete bidding with the winning bid
    pub fn finish_bidding(&mut self) -> Result<(u8, String, usize), String> {
        if let (Some(bid_length), Some(bidder), Some(trump_suit)) = (
            self.highest_bid_length,
            self.highest_bidder,
            self.highest_bid_suit.clone(),
        ) {
            self.complete_bidding(trump_suit.clone(), bidder);
            Ok((bid_length, trump_suit, bidder))
        } else {
            Err("No valid bid to complete with".to_string())
        }
    }

    /// Determine partnerships based on trump declarer
    /// In Sjavs, trump declarer partners with holder of highest trump
    pub fn determine_partnerships(&self) -> Option<(usize, usize, Vec<usize>)> {
        if let Some(trump_declarer) = self.trump_declarer {
            // In a real game, we'd need to examine hands to find highest trump holder
            // For now, return a simple partnership structure
            // trump_declarer partners with player opposite them
            let partner = (trump_declarer + 2) % 4;
            let opponents = vec![(trump_declarer + 1) % 4, (trump_declarer + 3) % 4];
            Some((trump_declarer, partner, opponents))
        } else {
            None
        }
    }

    /// Get current bidding summary
    pub fn get_bidding_state(&self) -> BiddingState {
        BiddingState {
            current_bidder: self.current_bidder,
            highest_bid_length: self.highest_bid_length,
            highest_bidder: self.highest_bidder,
            trump_suit: self.trump_suit.clone(),
            trump_declarer: self.trump_declarer,
        }
    }
}

/// Current state of bidding
#[derive(Debug, Clone)]
pub struct BiddingState {
    pub current_bidder: Option<usize>,
    pub highest_bid_length: Option<u8>,
    pub highest_bidder: Option<usize>,
    pub trump_suit: Option<String>,
    pub trump_declarer: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_state_transitions() {
        let mut match_obj = NormalMatch::new("test_game".to_string(), 1234, 3);

        // Initial state
        assert_eq!(match_obj.status, NormalMatchStatus::Waiting);
        assert!(match_obj.can_start());
        assert!(!match_obj.is_active());

        // Start dealing
        match_obj.start_dealing(0);
        assert_eq!(match_obj.status, NormalMatchStatus::Dealing);
        assert_eq!(match_obj.dealer_position, Some(0));
        assert_eq!(match_obj.current_bidder, Some(1)); // Left of dealer
        assert!(match_obj.is_active());

        // Start bidding
        match_obj.start_bidding();
        assert_eq!(match_obj.status, NormalMatchStatus::Bidding);
        assert!(match_obj.is_player_turn_to_bid(1));
        assert!(!match_obj.is_player_turn_to_bid(0));

        // Update bid
        match_obj.update_bid(1, 6);
        assert_eq!(match_obj.highest_bid_length, Some(6));
        assert_eq!(match_obj.highest_bidder, Some(1));
        assert_eq!(match_obj.current_bidder, Some(2)); // Next player

        // Complete bidding
        match_obj.complete_bidding("hearts".to_string(), 1);
        assert_eq!(match_obj.status, NormalMatchStatus::Playing);
        assert_eq!(match_obj.trump_suit, Some("hearts".to_string()));
        assert_eq!(match_obj.trump_declarer, Some(1));
        assert_eq!(match_obj.current_leader, Some(1)); // Left of dealer
        assert_eq!(match_obj.current_bidder, None); // No more bidding
    }

    #[test]
    fn test_redeal_functionality() {
        let mut match_obj = NormalMatch::new("test_redeal".to_string(), 5678, 3);

        // Set up bidding state
        match_obj.start_dealing(2);
        match_obj.start_bidding();
        match_obj.update_bid(3, 5);

        // Reset for redeal
        match_obj.reset_for_redeal();
        assert_eq!(match_obj.status, NormalMatchStatus::Dealing);
        assert_eq!(match_obj.current_bidder, Some(3)); // Left of dealer again
        assert_eq!(match_obj.highest_bid_length, None);
        assert_eq!(match_obj.highest_bidder, None);
        assert_eq!(match_obj.trump_suit, None);
    }

    #[test]
    fn test_redis_serialization_backward_compatibility() {
        // Test that old matches without new fields can still be loaded
        let mut old_hash = HashMap::new();
        old_hash.insert("pin".to_string(), "1234".to_string());
        old_hash.insert("status".to_string(), "waiting".to_string());
        old_hash.insert("number_of_crosses".to_string(), "3".to_string());
        old_hash.insert("current_cross".to_string(), "0".to_string());
        old_hash.insert("created_timestamp".to_string(), "1234567890".to_string());

        let match_obj = NormalMatch::from_redis_hash("test_id".to_string(), &old_hash).unwrap();

        // Should have default values for new fields
        assert_eq!(match_obj.dealer_position, None);
        assert_eq!(match_obj.current_bidder, None);
        assert_eq!(match_obj.trump_suit, None);
    }

    #[test]
    fn test_redis_serialization_with_new_fields() {
        let mut match_obj = NormalMatch::new("test_full".to_string(), 9999, 5);
        match_obj.start_dealing(1);
        match_obj.start_bidding();
        match_obj.update_bid(2, 7);

        // Serialize to hash
        let hash = match_obj.to_redis_hash();

        // Deserialize back
        let restored_match = NormalMatch::from_redis_hash(match_obj.id.clone(), &hash).unwrap();

        // Verify all fields are preserved
        assert_eq!(restored_match.dealer_position, Some(1));
        assert_eq!(restored_match.current_bidder, Some(3)); // After bid update
        assert_eq!(restored_match.highest_bid_length, Some(7));
        assert_eq!(restored_match.highest_bidder, Some(2));
        assert_eq!(restored_match.status, NormalMatchStatus::Bidding);
    }

    #[test]
    fn test_enum_string_conversions() {
        // Test new enum values
        assert_eq!(NormalMatchStatus::Dealing.to_string(), "dealing");
        assert_eq!(NormalMatchStatus::Bidding.to_string(), "bidding");
        assert_eq!(NormalMatchStatus::Playing.to_string(), "playing");

        // Test parsing
        assert_eq!(
            NormalMatchStatus::from("dealing"),
            NormalMatchStatus::Dealing
        );
        assert_eq!(
            NormalMatchStatus::from("bidding"),
            NormalMatchStatus::Bidding
        );
        assert_eq!(
            NormalMatchStatus::from("playing"),
            NormalMatchStatus::Playing
        );

        // Test unknown value defaults to waiting
        assert_eq!(
            NormalMatchStatus::from("unknown"),
            NormalMatchStatus::Waiting
        );
    }

    #[test]
    fn test_helper_methods() {
        let match_obj = NormalMatch::new("helper_test".to_string(), 1111, 3);

        // Test next player calculation
        assert_eq!(match_obj.next_player(0), 1);
        assert_eq!(match_obj.next_player(3), 0); // Wraps around

        // Test activity status
        assert!(!match_obj.is_active()); // Waiting status

        let mut active_match = match_obj.clone();
        active_match.status = NormalMatchStatus::Bidding;
        assert!(active_match.is_active());
    }
}
