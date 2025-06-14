use super::card::{Card, Suit};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A player's hand with bidding functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hand {
    pub cards: Vec<Card>,
    pub player_position: usize,
}

impl Hand {
    pub fn new(cards: Vec<Card>, player_position: usize) -> Self {
        Self {
            cards,
            player_position,
        }
    }

    /// Get available trump bids for this hand based on current game state
    pub fn get_available_bids(&self, current_highest: Option<u8>) -> Vec<BidOption> {
        let trump_counts = self.calculate_trump_counts();
        let mut bids = Vec::new();

        let min_bid = current_highest.map(|h| h + 1).unwrap_or(5);

        for (suit_name, &count) in &trump_counts {
            if count >= min_bid {
                // Can bid this length or higher
                for bid_length in min_bid..=count {
                    bids.push(BidOption {
                        length: bid_length,
                        suit: suit_name.to_string(),
                        display_text: format!("{} trumps ({})", bid_length, suit_name),
                        is_club_declaration: *suit_name == "clubs",
                    });
                }
            } else if count == current_highest.unwrap_or(0) && *suit_name == "clubs" {
                // Can match current bid if we have clubs (club preference rule)
                bids.push(BidOption {
                    length: count,
                    suit: suit_name.to_string(),
                    display_text: format!("{} trumps ({}) - Club Declaration!", count, suit_name),
                    is_club_declaration: true,
                });
            }
        }

        // Sort by length ascending, then by club preference (clubs first for same length)
        bids.sort_by(|a, b| {
            a.length
                .cmp(&b.length)
                .then(if a.is_club_declaration && !b.is_club_declaration {
                    std::cmp::Ordering::Less // Clubs come first
                } else if !a.is_club_declaration && b.is_club_declaration {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                })
        });

        bids
    }

    /// Calculate trump counts for all suits for this hand
    pub fn calculate_trump_counts(&self) -> HashMap<String, u8> {
        let mut counts = HashMap::new();

        for (suit_name, suit) in [
            ("hearts", Suit::Hearts),
            ("diamonds", Suit::Diamonds),
            ("clubs", Suit::Clubs),
            ("spades", Suit::Spades),
        ] {
            let count = self.cards.iter().filter(|card| card.is_trump(suit)).count() as u8;
            counts.insert(suit_name.to_string(), count);
        }

        counts
    }

    /// Get the best possible bid for this hand
    pub fn get_best_bid(&self) -> Option<BidOption> {
        let trump_counts = self.calculate_trump_counts();
        let max_count = trump_counts.values().max().copied()?;

        if max_count < 5 {
            return None; // Can't bid
        }

        // Find suits with max count, prefer clubs if tied
        let best_suits: Vec<_> = trump_counts
            .iter()
            .filter(|(_, &count)| count == max_count)
            .collect();

        let (suit_name, &count) = best_suits
            .iter()
            .find(|(suit, _)| suit == &&"clubs".to_string())
            .or_else(|| best_suits.first())
            .unwrap();

        Some(BidOption {
            length: count,
            suit: suit_name.to_string(),
            display_text: format!("{} trumps ({})", count, suit_name),
            is_club_declaration: *suit_name == "clubs",
        })
    }

    /// Check if hand has a specific card
    pub fn has_card(&self, card: &Card) -> bool {
        self.cards.contains(card)
    }

    /// Remove a card from the hand (for playing cards)
    pub fn remove_card(&mut self, card: &Card) -> bool {
        if let Some(pos) = self.cards.iter().position(|c| c == card) {
            self.cards.remove(pos);
            true
        } else {
            false
        }
    }

    /// Add a card to the hand
    pub fn add_card(&mut self, card: Card) {
        self.cards.push(card);
        // Keep hand sorted for easier display
        self.sort_cards();
    }

    /// Sort cards in hand by suit and rank
    pub fn sort_cards(&mut self) {
        self.cards.sort_by(|a, b| {
            a.suit
                .to_string()
                .cmp(&b.suit.to_string())
                .then((a.rank as u8).cmp(&(b.rank as u8)))
        });
    }

    /// Get hand as card codes for storage/transmission
    pub fn to_codes(&self) -> Vec<String> {
        self.cards.iter().map(|c| c.code()).collect()
    }

    /// Create hand from card codes
    pub fn from_codes(codes: Vec<String>, player_position: usize) -> Result<Self, String> {
        let mut cards = Vec::new();
        for code in codes {
            cards.push(Card::from_code(&code)?);
        }
        let mut hand = Self::new(cards, player_position);
        hand.sort_cards();
        Ok(hand)
    }

    /// Get total point value of cards in hand
    pub fn point_value(&self) -> u8 {
        self.cards.iter().map(|c| c.point_value()).sum()
    }

    /// Check if hand is valid for Sjavs (exactly 8 cards)
    pub fn is_valid(&self) -> bool {
        self.cards.len() == 8
    }

    /// Get cards that can legally be played given the current trick state
    pub fn get_playable_cards(&self, trump_suit: Suit, lead_suit: Option<Suit>) -> Vec<Card> {
        match lead_suit {
            None => {
                // Can lead any card
                self.cards.clone()
            }
            Some(lead) => {
                // Must follow suit if possible
                let following_cards: Vec<_> = self
                    .cards
                    .iter()
                    .filter(|&card| card.suit == lead && !card.is_trump(trump_suit))
                    .copied()
                    .collect();

                if !following_cards.is_empty() {
                    following_cards
                } else {
                    // Can't follow suit, can play any card
                    self.cards.clone()
                }
            }
        }
    }

    /// Get a summary of the hand for display
    pub fn get_summary(&self) -> HandSummary {
        let trump_counts = self.calculate_trump_counts();
        let point_value = self.point_value();
        let best_bid = self.get_best_bid();

        HandSummary {
            card_count: self.cards.len() as u8,
            trump_counts: trump_counts.clone(),
            point_value,
            best_possible_bid: best_bid,
            has_permanent_trumps: self.cards.iter().any(|c| c.is_permanent_trump()),
            can_bid: self.cards.len() == 8 && trump_counts.values().any(|&c| c >= 5),
        }
    }
}

/// A bid option available to a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidOption {
    pub length: u8,
    pub suit: String,
    pub display_text: String,
    pub is_club_declaration: bool,
}

/// Summary information about a hand
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandSummary {
    pub card_count: u8,
    pub trump_counts: HashMap<String, u8>,
    pub point_value: u8,
    pub best_possible_bid: Option<BidOption>,
    pub has_permanent_trumps: bool,
    pub can_bid: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::card::Rank;

    #[test]
    fn test_trump_counting() {
        let cards = vec![
            Card::new(Suit::Clubs, Rank::Queen), // Permanent trump
            Card::new(Suit::Hearts, Rank::Jack), // Permanent trump
            Card::new(Suit::Hearts, Rank::Ace),  // Trump if hearts
            Card::new(Suit::Hearts, Rank::King), // Trump if hearts
            Card::new(Suit::Spades, Rank::Seven),
            Card::new(Suit::Spades, Rank::Eight),
            Card::new(Suit::Diamonds, Rank::Nine),
            Card::new(Suit::Diamonds, Rank::Ten),
        ];

        let hand = Hand::new(cards, 0);
        let counts = hand.calculate_trump_counts();

        assert_eq!(counts["hearts"], 4); // 2 permanent + 2 suit
        assert_eq!(counts["clubs"], 2); // 2 permanent + 0 suit
        assert_eq!(counts["spades"], 4); // 2 permanent + 2 suit
        assert_eq!(counts["diamonds"], 4); // 2 permanent + 2 suit
    }

    #[test]
    fn test_available_bids() {
        let cards = vec![
            Card::new(Suit::Clubs, Rank::Queen),
            Card::new(Suit::Hearts, Rank::Jack),
            Card::new(Suit::Hearts, Rank::Ace),
            Card::new(Suit::Hearts, Rank::King),
            Card::new(Suit::Hearts, Rank::Ten),
            Card::new(Suit::Hearts, Rank::Nine),
            Card::new(Suit::Spades, Rank::Seven),
            Card::new(Suit::Spades, Rank::Eight),
        ];

        let hand = Hand::new(cards, 0);
        let bids = hand.get_available_bids(None);

        // Should have bids for hearts (6 trumps)
        let heart_bids: Vec<_> = bids.iter().filter(|b| b.suit == "hearts").collect();
        assert!(!heart_bids.is_empty());
        assert!(heart_bids.iter().any(|b| b.length == 6));

        // Should be able to bid 5 and 6 trumps in hearts
        assert!(heart_bids.iter().any(|b| b.length == 5));
    }

    #[test]
    fn test_club_preference_bidding() {
        let cards = vec![
            Card::new(Suit::Clubs, Rank::Queen),
            Card::new(Suit::Hearts, Rank::Jack),
            Card::new(Suit::Clubs, Rank::Ace),
            Card::new(Suit::Clubs, Rank::King),
            Card::new(Suit::Clubs, Rank::Ten),
            Card::new(Suit::Hearts, Rank::Nine),
            Card::new(Suit::Spades, Rank::Seven),
            Card::new(Suit::Spades, Rank::Eight),
        ];

        let hand = Hand::new(cards, 0);

        // Can match bid of 5 with clubs even though we have exactly 5
        let bids = hand.get_available_bids(Some(5));
        let club_bids: Vec<_> = bids.iter().filter(|b| b.suit == "clubs").collect();

        assert!(!club_bids.is_empty());
        assert!(club_bids
            .iter()
            .any(|b| b.length == 5 && b.is_club_declaration));
    }

    #[test]
    fn test_best_bid() {
        let cards = vec![
            Card::new(Suit::Clubs, Rank::Queen),
            Card::new(Suit::Hearts, Rank::Jack),
            Card::new(Suit::Hearts, Rank::Ace),
            Card::new(Suit::Hearts, Rank::King),
            Card::new(Suit::Hearts, Rank::Ten),
            Card::new(Suit::Hearts, Rank::Nine),
            Card::new(Suit::Spades, Rank::Seven),
            Card::new(Suit::Spades, Rank::Eight),
        ];

        let hand = Hand::new(cards, 0);
        let best_bid = hand.get_best_bid().unwrap();

        assert_eq!(best_bid.length, 6); // Hearts trump count
        assert_eq!(best_bid.suit, "hearts");
    }

    #[test]
    fn test_best_bid_prefers_clubs() {
        let cards = vec![
            Card::new(Suit::Clubs, Rank::Queen),
            Card::new(Suit::Hearts, Rank::Jack),
            Card::new(Suit::Clubs, Rank::Ace),
            Card::new(Suit::Clubs, Rank::King),
            Card::new(Suit::Clubs, Rank::Ten), // Added extra club to get 5 total
            Card::new(Suit::Hearts, Rank::Nine),
            Card::new(Suit::Spades, Rank::Seven),
            Card::new(Suit::Spades, Rank::Eight),
        ];

        let hand = Hand::new(cards, 0);
        let best_bid = hand.get_best_bid().unwrap();

        // Clubs has 5 trumps (2 permanent + 3 suit), hearts has 3 trumps (2 permanent + 1 suit)
        assert_eq!(best_bid.length, 5);
        assert_eq!(best_bid.suit, "clubs");
        assert!(best_bid.is_club_declaration);
    }

    #[test]
    fn test_card_codes_conversion() {
        let cards = vec![
            Card::new(Suit::Hearts, Rank::Ace),
            Card::new(Suit::Spades, Rank::King),
        ];

        let hand = Hand::new(cards, 1);
        let codes = hand.to_codes();
        assert_eq!(codes, vec!["AH", "KS"]);

        let restored_hand = Hand::from_codes(codes, 1).unwrap();
        assert_eq!(restored_hand.cards.len(), 2);
        assert_eq!(restored_hand.player_position, 1);
    }

    #[test]
    fn test_hand_validation() {
        let valid_cards = vec![
            Card::new(Suit::Hearts, Rank::Ace),
            Card::new(Suit::Hearts, Rank::King),
            Card::new(Suit::Hearts, Rank::Queen),
            Card::new(Suit::Hearts, Rank::Jack),
            Card::new(Suit::Spades, Rank::Ace),
            Card::new(Suit::Spades, Rank::King),
            Card::new(Suit::Spades, Rank::Queen),
            Card::new(Suit::Spades, Rank::Jack),
        ];

        let valid_hand = Hand::new(valid_cards, 0);
        assert!(valid_hand.is_valid());

        let invalid_hand = Hand::new(vec![Card::new(Suit::Hearts, Rank::Ace)], 0);
        assert!(!invalid_hand.is_valid());
    }

    #[test]
    fn test_playable_cards() {
        let cards = vec![
            Card::new(Suit::Hearts, Rank::Ace),
            Card::new(Suit::Hearts, Rank::King),
            Card::new(Suit::Spades, Rank::Queen),
            Card::new(Suit::Clubs, Rank::Jack),
            Card::new(Suit::Spades, Rank::Seven),
            Card::new(Suit::Diamonds, Rank::Eight),
            Card::new(Suit::Diamonds, Rank::Nine),
            Card::new(Suit::Diamonds, Rank::Ten),
        ];

        let hand = Hand::new(cards, 0);

        // Can lead any card
        let playable = hand.get_playable_cards(Suit::Hearts, None);
        assert_eq!(playable.len(), 8);

        // Must follow diamonds if possible
        let playable = hand.get_playable_cards(Suit::Hearts, Some(Suit::Diamonds));
        assert_eq!(playable.len(), 3); // Three diamonds that aren't trump
    }

    #[test]
    fn test_hand_summary() {
        let cards = vec![
            Card::new(Suit::Clubs, Rank::Queen), // Permanent trump
            Card::new(Suit::Hearts, Rank::Jack), // Permanent trump
            Card::new(Suit::Hearts, Rank::Ace),  // 11 points
            Card::new(Suit::Hearts, Rank::King), // 4 points
            Card::new(Suit::Hearts, Rank::Ten),  // 10 points
            Card::new(Suit::Hearts, Rank::Nine),
            Card::new(Suit::Spades, Rank::Seven),
            Card::new(Suit::Spades, Rank::Eight),
        ];

        let hand = Hand::new(cards, 0);
        let summary = hand.get_summary();

        assert_eq!(summary.card_count, 8);
        assert!(summary.has_permanent_trumps);
        assert!(summary.can_bid);
        assert_eq!(summary.point_value, 30); // 11 + 4 + 10 + 3 + 2 = 30
        assert!(summary.best_possible_bid.is_some());
    }
}
