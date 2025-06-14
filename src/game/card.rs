use serde::{Deserialize, Serialize};
use std::fmt;

/// Card suits in Sjavs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Suit::Hearts => write!(f, "H"),
            Suit::Diamonds => write!(f, "D"),
            Suit::Clubs => write!(f, "C"),
            Suit::Spades => write!(f, "S"),
        }
    }
}

impl From<&str> for Suit {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "hearts" | "h" => Suit::Hearts,
            "diamonds" | "d" => Suit::Diamonds,
            "clubs" | "c" => Suit::Clubs,
            "spades" | "s" => Suit::Spades,
            _ => panic!("Invalid suit: {}", s),
        }
    }
}

/// Card ranks in Sjavs (7, 8, 9, 10, J, Q, K, A)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rank {
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Jack = 11,
    Queen = 12,
    King = 13,
    Ace = 14,
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Rank::Seven => write!(f, "7"),
            Rank::Eight => write!(f, "8"),
            Rank::Nine => write!(f, "9"),
            Rank::Ten => write!(f, "10"),
            Rank::Jack => write!(f, "J"),
            Rank::Queen => write!(f, "Q"),
            Rank::King => write!(f, "K"),
            Rank::Ace => write!(f, "A"),
        }
    }
}

impl From<&str> for Rank {
    fn from(s: &str) -> Self {
        match s {
            "7" => Rank::Seven,
            "8" => Rank::Eight,
            "9" => Rank::Nine,
            "10" => Rank::Ten,
            "J" => Rank::Jack,
            "Q" => Rank::Queen,
            "K" => Rank::King,
            "A" => Rank::Ace,
            _ => panic!("Invalid rank: {}", s),
        }
    }
}

/// A playing card
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

impl Card {
    pub fn new(suit: Suit, rank: Rank) -> Self {
        Self { suit, rank }
    }

    /// Get card code for storage/transmission (e.g., "AS", "QC", "10H")
    pub fn code(&self) -> String {
        format!("{}{}", self.rank, self.suit)
    }

    /// Create card from code (e.g., "AS" -> Ace of Spades)
    pub fn from_code(code: &str) -> Result<Self, String> {
        if code.len() < 2 {
            return Err("Card code too short".to_string());
        }

        let (rank_str, suit_str) = if code.starts_with("10") {
            ("10", &code[2..])
        } else {
            (&code[..1], &code[1..])
        };

        let rank = Rank::from(rank_str);
        let suit = Suit::from(suit_str);

        Ok(Card::new(suit, rank))
    }

    /// Get point value of card (Sjavs scoring system)
    pub fn point_value(&self) -> u8 {
        match self.rank {
            Rank::Ace => 11,
            Rank::King => 4,
            Rank::Queen => 3,
            Rank::Jack => 2,
            Rank::Ten => 10,
            _ => 0, // 7, 8, 9 have no points
        }
    }

    /// Check if this card is a permanent trump (6 cards total)
    /// Club Queen, Spade Queen, and all 4 Jacks
    pub fn is_permanent_trump(&self) -> bool {
        matches!(
            (self.suit, self.rank),
            (Suit::Clubs, Rank::Queen) |    // Highest permanent trump
            (Suit::Spades, Rank::Queen) |   // Second highest permanent trump
            (Suit::Clubs, Rank::Jack) |     // Third highest permanent trump
            (Suit::Spades, Rank::Jack) |    // Fourth highest permanent trump
            (Suit::Hearts, Rank::Jack) |    // Fifth highest permanent trump
            (Suit::Diamonds, Rank::Jack) // Lowest permanent trump
        )
    }

    /// Check if this card is trump given a trump suit
    pub fn is_trump(&self, trump_suit: Suit) -> bool {
        self.is_permanent_trump() || self.suit == trump_suit
    }

    /// Get trump order for comparison (higher number = stronger trump)
    /// This implements the authentic Sjavs trump hierarchy
    pub fn trump_order(&self, trump_suit: Suit) -> Option<u8> {
        if !self.is_trump(trump_suit) {
            return None;
        }

        // Permanent trumps (highest priority)
        match (self.suit, self.rank) {
            (Suit::Clubs, Rank::Queen) => Some(20), // Highest trump always
            (Suit::Spades, Rank::Queen) => Some(19), // Second highest trump always
            (Suit::Clubs, Rank::Jack) => Some(18),  // Third highest trump always
            (Suit::Spades, Rank::Jack) => Some(17), // Fourth highest trump always
            (Suit::Hearts, Rank::Jack) => Some(16), // Fifth highest trump always
            (Suit::Diamonds, Rank::Jack) => Some(15), // Lowest permanent trump

            // Suit trumps (excluding permanent trumps)
            _ if self.suit == trump_suit => {
                match self.rank {
                    Rank::Ace => Some(14),
                    Rank::King => Some(13),
                    Rank::Queen => Some(12), // Only for hearts/diamonds when trump
                    Rank::Ten => Some(11),
                    Rank::Nine => Some(10),
                    Rank::Eight => Some(9),
                    Rank::Seven => Some(8),
                    Rank::Jack => unreachable!("Jacks are permanent trumps"),
                }
            }
            _ => None,
        }
    }

    /// Get non-trump order for comparison when cards are not trump
    pub fn non_trump_order(&self) -> u8 {
        match self.rank {
            Rank::Ace => 14,
            Rank::King => 13,
            Rank::Queen => 12,
            Rank::Jack => 11,
            Rank::Ten => 10,
            Rank::Nine => 9,
            Rank::Eight => 8,
            Rank::Seven => 7,
        }
    }

    /// Compare two cards for trick-taking (returns true if self beats other)
    pub fn beats(&self, other: &Card, trump_suit: Suit, lead_suit: Suit) -> bool {
        let self_trump_order = self.trump_order(trump_suit);
        let other_trump_order = other.trump_order(trump_suit);

        match (self_trump_order, other_trump_order) {
            // Both are trump cards
            (Some(self_order), Some(other_order)) => self_order > other_order,

            // Self is trump, other is not
            (Some(_), None) => true,

            // Other is trump, self is not
            (None, Some(_)) => false,

            // Neither is trump
            (None, None) => {
                // Must follow suit if possible
                if self.suit == lead_suit && other.suit != lead_suit {
                    true // Self follows suit, other doesn't
                } else if self.suit != lead_suit && other.suit == lead_suit {
                    false // Other follows suit, self doesn't
                } else if self.suit == lead_suit && other.suit == lead_suit {
                    // Both follow suit, compare rank
                    self.non_trump_order() > other.non_trump_order()
                } else {
                    // Neither follows suit, self doesn't win
                    false
                }
            }
        }
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_codes() {
        let ace_spades = Card::new(Suit::Spades, Rank::Ace);
        assert_eq!(ace_spades.code(), "AS");

        let ten_hearts = Card::new(Suit::Hearts, Rank::Ten);
        assert_eq!(ten_hearts.code(), "10H");

        let seven_clubs = Card::new(Suit::Clubs, Rank::Seven);
        assert_eq!(seven_clubs.code(), "7C");
    }

    #[test]
    fn test_card_from_code() {
        let card = Card::from_code("AS").unwrap();
        assert_eq!(card.suit, Suit::Spades);
        assert_eq!(card.rank, Rank::Ace);

        let card = Card::from_code("10H").unwrap();
        assert_eq!(card.suit, Suit::Hearts);
        assert_eq!(card.rank, Rank::Ten);
    }

    #[test]
    fn test_permanent_trumps() {
        let club_queen = Card::new(Suit::Clubs, Rank::Queen);
        assert!(club_queen.is_permanent_trump());

        let spade_queen = Card::new(Suit::Spades, Rank::Queen);
        assert!(spade_queen.is_permanent_trump());

        let heart_jack = Card::new(Suit::Hearts, Rank::Jack);
        assert!(heart_jack.is_permanent_trump());

        let heart_ace = Card::new(Suit::Hearts, Rank::Ace);
        assert!(!heart_ace.is_permanent_trump());
    }

    #[test]
    fn test_trump_hierarchy() {
        let club_queen = Card::new(Suit::Clubs, Rank::Queen);
        let spade_queen = Card::new(Suit::Spades, Rank::Queen);
        let heart_jack = Card::new(Suit::Hearts, Rank::Jack);
        let heart_ace = Card::new(Suit::Hearts, Rank::Ace);

        // Club Queen beats all other trumps
        assert!(club_queen.trump_order(Suit::Hearts) > spade_queen.trump_order(Suit::Hearts));
        assert!(club_queen.trump_order(Suit::Hearts) > heart_jack.trump_order(Suit::Hearts));
        assert!(club_queen.trump_order(Suit::Hearts) > heart_ace.trump_order(Suit::Hearts));

        // Permanent trumps beat suit trumps
        assert!(spade_queen.trump_order(Suit::Hearts) > heart_ace.trump_order(Suit::Hearts));
        assert!(heart_jack.trump_order(Suit::Hearts) > heart_ace.trump_order(Suit::Hearts));
    }

    #[test]
    fn test_trump_vs_non_trump() {
        let club_queen = Card::new(Suit::Clubs, Rank::Queen);
        let heart_ace = Card::new(Suit::Hearts, Rank::Ace);
        let spade_ace = Card::new(Suit::Spades, Rank::Ace);

        // Permanent trump beats suit trump
        assert!(club_queen.beats(&heart_ace, Suit::Hearts, Suit::Hearts));

        // Trump beats non-trump
        assert!(heart_ace.beats(&spade_ace, Suit::Hearts, Suit::Spades));

        // Non-trump following suit beats non-trump not following suit
        assert!(spade_ace.beats(&heart_ace, Suit::Clubs, Suit::Spades));
    }

    #[test]
    fn test_point_values() {
        assert_eq!(Card::new(Suit::Hearts, Rank::Ace).point_value(), 11);
        assert_eq!(Card::new(Suit::Hearts, Rank::King).point_value(), 4);
        assert_eq!(Card::new(Suit::Hearts, Rank::Queen).point_value(), 3);
        assert_eq!(Card::new(Suit::Hearts, Rank::Jack).point_value(), 2);
        assert_eq!(Card::new(Suit::Hearts, Rank::Ten).point_value(), 10);
        assert_eq!(Card::new(Suit::Hearts, Rank::Seven).point_value(), 0);
    }

    #[test]
    fn test_suit_from_string() {
        assert_eq!(Suit::from("hearts"), Suit::Hearts);
        assert_eq!(Suit::from("h"), Suit::Hearts);
        assert_eq!(Suit::from("CLUBS"), Suit::Clubs);
        assert_eq!(Suit::from("D"), Suit::Diamonds);
    }
}
