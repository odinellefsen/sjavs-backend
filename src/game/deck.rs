use super::card::{Card, Rank, Suit};
use rand::seq::SliceRandom;
use rand::thread_rng;

/// A Sjavs deck (32 cards: 7, 8, 9, 10, J, Q, K, A in all suits)
#[derive(Debug, Clone)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    /// Create a new 32-card Sjavs deck
    pub fn new() -> Self {
        let mut cards = Vec::with_capacity(32);

        // Create all 32 cards (7, 8, 9, 10, J, Q, K, A in all suits)
        for suit in [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades] {
            for rank in [
                Rank::Seven,
                Rank::Eight,
                Rank::Nine,
                Rank::Ten,
                Rank::Jack,
                Rank::Queen,
                Rank::King,
                Rank::Ace,
            ] {
                cards.push(Card::new(suit, rank));
            }
        }

        Self { cards }
    }

    /// Shuffle the deck using cryptographically secure random number generator
    pub fn shuffle(&mut self) {
        let mut rng = thread_rng();
        self.cards.shuffle(&mut rng);
    }

    /// Deal cards to 4 players (8 cards each)
    pub fn deal(&mut self) -> Result<[Vec<Card>; 4], String> {
        if self.cards.len() != 32 {
            return Err("Deck must have exactly 32 cards to deal".to_string());
        }

        let mut hands = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];

        // Deal 8 cards to each player in round-robin fashion
        for i in 0..32 {
            let player = i % 4;
            hands[player].push(self.cards[i]);
        }

        // Sort each hand for easier display (by suit, then by rank)
        for hand in &mut hands {
            hand.sort_by(|a, b| {
                // Sort by suit first, then by rank
                a.suit
                    .to_string()
                    .cmp(&b.suit.to_string())
                    .then((a.rank as u8).cmp(&(b.rank as u8)))
            });
        }

        Ok(hands)
    }

    /// Calculate how many trumps a hand would have for each suit
    pub fn calculate_trump_counts(hand: &[Card]) -> [u8; 4] {
        let suits = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];
        let mut counts = [0u8; 4];

        for (i, &trump_suit) in suits.iter().enumerate() {
            counts[i] = hand.iter().filter(|card| card.is_trump(trump_suit)).count() as u8;
        }

        counts
    }

    /// Check if any hand has at least 5 trumps in any suit (minimum for valid bid)
    pub fn has_valid_hands(hands: &[Vec<Card>; 4]) -> bool {
        for hand in hands {
            let trump_counts = Self::calculate_trump_counts(hand);
            if trump_counts.iter().any(|&count| count >= 5) {
                return true;
            }
        }
        false
    }

    /// Generate hands until at least one player has a valid bid (5+ trumps)
    /// This implements the authentic Sjavs rule of redealing until someone can bid
    pub fn deal_until_valid() -> [Vec<Card>; 4] {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 1000; // Safety valve

        loop {
            attempts += 1;
            if attempts > MAX_ATTEMPTS {
                // This should never happen in practice, but prevents infinite loops
                panic!(
                    "Unable to generate valid hands after {} attempts",
                    MAX_ATTEMPTS
                );
            }

            let mut deck = Deck::new();
            deck.shuffle();

            if let Ok(hands) = deck.deal() {
                if Self::has_valid_hands(&hands) {
                    println!("Generated valid hands after {} attempts", attempts);
                    return hands;
                }
            }
        }
    }

    /// Get statistics about trump distribution in a set of hands
    pub fn analyze_hands(hands: &[Vec<Card>; 4]) -> HandAnalysis {
        let mut total_trumps_per_suit = [0u8; 4];
        let mut players_with_valid_bids = 0;
        let mut best_bid_length = 0u8;
        let mut best_bid_suits = Vec::new();

        for (player_idx, hand) in hands.iter().enumerate() {
            let trump_counts = Self::calculate_trump_counts(hand);

            // Add to total trump counts
            for i in 0..4 {
                total_trumps_per_suit[i] += trump_counts[i];
            }

            // Check if player has valid bid
            let max_trumps = trump_counts.iter().max().unwrap_or(&0);
            if *max_trumps >= 5 {
                players_with_valid_bids += 1;

                if *max_trumps > best_bid_length {
                    best_bid_length = *max_trumps;
                    best_bid_suits.clear();

                    // Find which suits have this many trumps
                    for (suit_idx, &count) in trump_counts.iter().enumerate() {
                        if count == best_bid_length {
                            let suit = match suit_idx {
                                0 => "hearts",
                                1 => "diamonds",
                                2 => "clubs",
                                3 => "spades",
                                _ => unreachable!(),
                            };
                            best_bid_suits.push((player_idx, suit.to_string()));
                        }
                    }
                }
            }
        }

        HandAnalysis {
            total_trumps_per_suit,
            players_with_valid_bids,
            best_bid_length,
            best_bid_suits,
        }
    }

    /// Get remaining cards in deck
    pub fn remaining_cards(&self) -> usize {
        self.cards.len()
    }

    /// Check if deck is complete (has all 32 cards)
    pub fn is_complete(&self) -> bool {
        self.cards.len() == 32
    }
}

impl Default for Deck {
    fn default() -> Self {
        Self::new()
    }
}

/// Analysis of dealt hands
#[derive(Debug)]
pub struct HandAnalysis {
    pub total_trumps_per_suit: [u8; 4], // [hearts, diamonds, clubs, spades]
    pub players_with_valid_bids: u8,
    pub best_bid_length: u8,
    pub best_bid_suits: Vec<(usize, String)>, // (player_index, suit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck_creation() {
        let deck = Deck::new();
        assert_eq!(deck.cards.len(), 32);
        assert!(deck.is_complete());

        // Check we have right number of each suit
        for suit in [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades] {
            let count = deck.cards.iter().filter(|c| c.suit == suit).count();
            assert_eq!(count, 8, "Should have 8 cards of each suit");
        }

        // Check we have right number of each rank
        for rank in [
            Rank::Seven,
            Rank::Eight,
            Rank::Nine,
            Rank::Ten,
            Rank::Jack,
            Rank::Queen,
            Rank::King,
            Rank::Ace,
        ] {
            let count = deck.cards.iter().filter(|c| c.rank == rank).count();
            assert_eq!(count, 4, "Should have 4 cards of each rank");
        }
    }

    #[test]
    fn test_dealing() {
        let mut deck = Deck::new();
        let hands = deck.deal().unwrap();

        // Each player should have 8 cards
        for (i, hand) in hands.iter().enumerate() {
            assert_eq!(hand.len(), 8, "Player {} should have 8 cards", i);
        }

        // All cards should be dealt (no duplicates)
        let mut all_cards: Vec<&Card> = Vec::new();
        for hand in &hands {
            all_cards.extend(hand.iter());
        }
        assert_eq!(all_cards.len(), 32);

        // No duplicates
        for i in 0..all_cards.len() {
            for j in (i + 1)..all_cards.len() {
                assert_ne!(all_cards[i], all_cards[j], "Found duplicate card");
            }
        }
    }

    #[test]
    fn test_trump_counting() {
        // Create a hand with known trumps
        let hand = vec![
            Card::new(Suit::Clubs, Rank::Queen),  // Permanent trump (always)
            Card::new(Suit::Hearts, Rank::Jack),  // Permanent trump (always)
            Card::new(Suit::Hearts, Rank::Ace),   // Trump if hearts is trump
            Card::new(Suit::Hearts, Rank::King),  // Trump if hearts is trump
            Card::new(Suit::Spades, Rank::Seven), // Not trump if hearts is trump
            Card::new(Suit::Diamonds, Rank::Eight), // Not trump if hearts is trump
        ];

        let counts = Deck::calculate_trump_counts(&hand);

        // Hearts: 2 permanent + 2 suit = 4 trumps
        assert_eq!(counts[0], 4, "Hearts trump count");

        // Diamonds: 2 permanent + 1 suit = 3 trumps
        assert_eq!(counts[1], 3, "Diamonds trump count");

        // Clubs: 2 permanent + 0 suit = 2 trumps
        assert_eq!(counts[2], 2, "Clubs trump count");

        // Spades: 2 permanent + 1 suit = 3 trumps
        assert_eq!(counts[3], 3, "Spades trump count");
    }

    #[test]
    fn test_valid_hands_detection() {
        // Create hands where only one player has 5+ trumps
        let hands = [
            vec![
                Card::new(Suit::Clubs, Rank::Queen), // Permanent trump
                Card::new(Suit::Hearts, Rank::Jack), // Permanent trump
                Card::new(Suit::Hearts, Rank::Ace),  // Hearts trump
                Card::new(Suit::Hearts, Rank::King), // Hearts trump
                Card::new(Suit::Hearts, Rank::Ten),  // Hearts trump (5 total)
                Card::new(Suit::Spades, Rank::Seven),
                Card::new(Suit::Spades, Rank::Eight),
                Card::new(Suit::Diamonds, Rank::Nine),
            ],
            vec![
                Card::new(Suit::Spades, Rank::Queen), // Only 2 trumps for any suit
                Card::new(Suit::Diamonds, Rank::Jack),
                Card::new(Suit::Diamonds, Rank::Ace),
                Card::new(Suit::Diamonds, Rank::King),
                Card::new(Suit::Clubs, Rank::Ace),
                Card::new(Suit::Clubs, Rank::King),
                Card::new(Suit::Spades, Rank::Ace),
                Card::new(Suit::Spades, Rank::King),
            ],
            vec![
                Card::new(Suit::Clubs, Rank::Jack),
                Card::new(Suit::Spades, Rank::Jack),
                Card::new(Suit::Hearts, Rank::Seven),
                Card::new(Suit::Hearts, Rank::Eight),
                Card::new(Suit::Hearts, Rank::Nine),
                Card::new(Suit::Diamonds, Rank::Seven),
                Card::new(Suit::Diamonds, Rank::Eight),
                Card::new(Suit::Clubs, Rank::Seven),
            ],
            vec![
                Card::new(Suit::Clubs, Rank::Eight),
                Card::new(Suit::Clubs, Rank::Nine),
                Card::new(Suit::Clubs, Rank::Ten),
                Card::new(Suit::Clubs, Rank::King),
                Card::new(Suit::Spades, Rank::Nine),
                Card::new(Suit::Spades, Rank::Ten),
                Card::new(Suit::Diamonds, Rank::Ten),
                Card::new(Suit::Diamonds, Rank::Queen),
            ],
        ];

        assert!(Deck::has_valid_hands(&hands), "Should detect valid hands");
    }

    #[test]
    fn test_deal_until_valid() {
        // This test might take a moment but should always succeed
        let hands = Deck::deal_until_valid();

        // Verify we got valid hands
        assert!(Deck::has_valid_hands(&hands));

        // Verify structure is correct
        assert_eq!(hands.len(), 4);
        for hand in &hands {
            assert_eq!(hand.len(), 8);
        }
    }

    #[test]
    fn test_hand_analysis() {
        let hands = [
            vec![
                Card::new(Suit::Clubs, Rank::Queen), // Permanent trump
                Card::new(Suit::Hearts, Rank::Jack), // Permanent trump
                Card::new(Suit::Hearts, Rank::Ace),  // Hearts trump
                Card::new(Suit::Hearts, Rank::King), // Hearts trump
                Card::new(Suit::Hearts, Rank::Ten),  // Hearts trump (5 total)
                Card::new(Suit::Spades, Rank::Seven),
                Card::new(Suit::Spades, Rank::Eight),
                Card::new(Suit::Diamonds, Rank::Nine),
            ],
            vec![
                Card::new(Suit::Spades, Rank::Queen),
                Card::new(Suit::Diamonds, Rank::Jack),
                Card::new(Suit::Diamonds, Rank::Ace),
                Card::new(Suit::Diamonds, Rank::King),
                Card::new(Suit::Clubs, Rank::Ace),
                Card::new(Suit::Clubs, Rank::King),
                Card::new(Suit::Spades, Rank::Ace),
                Card::new(Suit::Spades, Rank::King),
            ],
            vec![
                Card::new(Suit::Clubs, Rank::Jack),
                Card::new(Suit::Spades, Rank::Jack),
                Card::new(Suit::Hearts, Rank::Seven),
                Card::new(Suit::Hearts, Rank::Eight),
                Card::new(Suit::Hearts, Rank::Nine),
                Card::new(Suit::Diamonds, Rank::Seven),
                Card::new(Suit::Diamonds, Rank::Eight),
                Card::new(Suit::Clubs, Rank::Seven),
            ],
            vec![
                Card::new(Suit::Clubs, Rank::Eight),
                Card::new(Suit::Clubs, Rank::Nine),
                Card::new(Suit::Clubs, Rank::Ten),
                Card::new(Suit::Clubs, Rank::King),
                Card::new(Suit::Spades, Rank::Nine),
                Card::new(Suit::Spades, Rank::Ten),
                Card::new(Suit::Diamonds, Rank::Ten),
                Card::new(Suit::Diamonds, Rank::Queen),
            ],
        ];

        let analysis = Deck::analyze_hands(&hands);

        // Should have at least one player with valid bid
        assert!(analysis.players_with_valid_bids >= 1);
        assert!(analysis.best_bid_length >= 5);
        assert!(!analysis.best_bid_suits.is_empty());
    }
}
