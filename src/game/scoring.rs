use serde::{Deserialize, Serialize};

/// Sjavs scoring rules implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SjavsScoring {
    /// Points won by trump declaring team
    pub trump_team_points: u8,
    /// Points won by opponent team
    pub opponent_team_points: u8,
    /// Tricks won by trump declaring team
    pub trump_team_tricks: u8,
    /// Tricks won by opponent team
    pub opponent_team_tricks: u8,
    /// Trump suit for this game
    pub trump_suit: String,
    /// Whether a single player from trump team won all tricks
    pub individual_vol: bool,
}

impl SjavsScoring {
    /// Calculate the game result using authentic Sjavs scoring rules
    pub fn calculate_game_result(&self) -> GameResult {
        let is_clubs = self.trump_suit == "clubs";

        // Check for "Vol" (all tricks)
        if self.trump_team_tricks == 8 {
            if self.individual_vol {
                // Single player from trump team won all tricks
                GameResult {
                    trump_team_score: if is_clubs { 24 } else { 16 },
                    opponent_team_score: 0,
                    result_type: SjavsResult::IndividualVol,
                    description: format!(
                        "Individual Vol - {} points for trump team",
                        if is_clubs { 24 } else { 16 }
                    ),
                }
            } else {
                // Trump team won all tricks
                GameResult {
                    trump_team_score: if is_clubs { 16 } else { 12 },
                    opponent_team_score: 0,
                    result_type: SjavsResult::Vol,
                    description: format!(
                        "Vol - {} points for trump team",
                        if is_clubs { 16 } else { 12 }
                    ),
                }
            }
        }
        // Check if opponents won all tricks
        else if self.opponent_team_tricks == 8 {
            GameResult {
                trump_team_score: 0,
                opponent_team_score: 16, // Always 16 regardless of trump suit
                result_type: SjavsResult::OpponentVol,
                description: "Opponents won all tricks - 16 points".to_string(),
            }
        }
        // Check for tie (both teams have 60 points)
        else if self.trump_team_points == 60 && self.opponent_team_points == 60 {
            GameResult {
                trump_team_score: 0,
                opponent_team_score: 0,
                result_type: SjavsResult::Tie,
                description: "Tie at 60-60 - no score, next game worth 2 extra points".to_string(),
            }
        }
        // Normal scoring based on trump team points
        else {
            match self.trump_team_points {
                90..=120 => GameResult {
                    trump_team_score: if is_clubs { 8 } else { 4 },
                    opponent_team_score: 0,
                    result_type: SjavsResult::TrumpTeamWin,
                    description: format!(
                        "Trump team 90-120 points - {} points",
                        if is_clubs { 8 } else { 4 }
                    ),
                },
                61..=89 => GameResult {
                    trump_team_score: if is_clubs { 4 } else { 2 },
                    opponent_team_score: 0,
                    result_type: SjavsResult::TrumpTeamWin,
                    description: format!(
                        "Trump team 61-89 points - {} points",
                        if is_clubs { 4 } else { 2 }
                    ),
                },
                31..=59 => {
                    // Trump team failed but avoided double loss ("at vera javnfrujjur")
                    GameResult {
                        trump_team_score: 0,
                        opponent_team_score: if is_clubs { 8 } else { 4 },
                        result_type: SjavsResult::OpponentWin,
                        description: format!(
                            "Trump team 31-59 points (avoided double) - opponents get {} points",
                            if is_clubs { 8 } else { 4 }
                        ),
                    }
                }
                1..=30 => {
                    // Trump team suffered double loss
                    GameResult {
                        trump_team_score: 0,
                        opponent_team_score: if is_clubs { 16 } else { 8 },
                        result_type: SjavsResult::OpponentDoubleWin,
                        description: format!(
                            "Trump team 0-30 points (double loss) - opponents get {} points",
                            if is_clubs { 16 } else { 8 }
                        ),
                    }
                }
                0 => {
                    // Trump team got no points at all
                    GameResult {
                        trump_team_score: 0,
                        opponent_team_score: if is_clubs { 16 } else { 8 },
                        result_type: SjavsResult::OpponentDoubleWin,
                        description: format!(
                            "Trump team 0 points - opponents get {} points",
                            if is_clubs { 16 } else { 8 }
                        ),
                    }
                }
                _ => unreachable!("Invalid point total"),
            }
        }
    }

    /// Check if this qualifies as "at vera javnfrujjur" (avoiding double loss)
    pub fn is_avoiding_double_loss(&self) -> bool {
        self.trump_team_points >= 31 && self.trump_team_points <= 59
    }

    /// Validate that points add up to 120 (total card points)
    pub fn validate_total_points(&self) -> bool {
        self.trump_team_points + self.opponent_team_points == 120
    }
}

/// Result of a completed Sjavs game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResult {
    /// Points scored by trump declaring team
    pub trump_team_score: u8,
    /// Points scored by opponent team
    pub opponent_team_score: u8,
    /// Type of result achieved
    pub result_type: SjavsResult,
    /// Human-readable description
    pub description: String,
}

/// Types of Sjavs game results
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SjavsResult {
    /// Trump team won normally
    TrumpTeamWin,
    /// Opponent team won normally
    OpponentWin,
    /// Opponent team won with double points
    OpponentDoubleWin,
    /// Trump team won all tricks
    Vol,
    /// Single player from trump team won all tricks
    IndividualVol,
    /// Opponent team won all tricks
    OpponentVol,
    /// Both teams got exactly 60 points
    Tie,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vol_scoring() {
        let scoring = SjavsScoring {
            trump_team_points: 120,
            opponent_team_points: 0,
            trump_team_tricks: 8,
            opponent_team_tricks: 0,
            trump_suit: "hearts".to_string(),
            individual_vol: false,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 12);
        assert_eq!(result.result_type, SjavsResult::Vol);
    }

    #[test]
    fn test_vol_scoring_clubs() {
        let scoring = SjavsScoring {
            trump_team_points: 120,
            opponent_team_points: 0,
            trump_team_tricks: 8,
            opponent_team_tricks: 0,
            trump_suit: "clubs".to_string(),
            individual_vol: false,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 16);
        assert_eq!(result.result_type, SjavsResult::Vol);
    }

    #[test]
    fn test_individual_vol() {
        let scoring = SjavsScoring {
            trump_team_points: 120,
            opponent_team_points: 0,
            trump_team_tricks: 8,
            opponent_team_tricks: 0,
            trump_suit: "hearts".to_string(),
            individual_vol: true,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 16);
        assert_eq!(result.result_type, SjavsResult::IndividualVol);
    }

    #[test]
    fn test_individual_vol_clubs() {
        let scoring = SjavsScoring {
            trump_team_points: 120,
            opponent_team_points: 0,
            trump_team_tricks: 8,
            opponent_team_tricks: 0,
            trump_suit: "clubs".to_string(),
            individual_vol: true,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 24);
        assert_eq!(result.result_type, SjavsResult::IndividualVol);
    }

    #[test]
    fn test_tie_scoring() {
        let scoring = SjavsScoring {
            trump_team_points: 60,
            opponent_team_points: 60,
            trump_team_tricks: 4,
            opponent_team_tricks: 4,
            trump_suit: "hearts".to_string(),
            individual_vol: false,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 0);
        assert_eq!(result.opponent_team_score, 0);
        assert_eq!(result.result_type, SjavsResult::Tie);
    }

    #[test]
    fn test_double_loss() {
        let scoring = SjavsScoring {
            trump_team_points: 25,
            opponent_team_points: 95,
            trump_team_tricks: 1,
            opponent_team_tricks: 7,
            trump_suit: "hearts".to_string(),
            individual_vol: false,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 0);
        assert_eq!(result.opponent_team_score, 8);
        assert_eq!(result.result_type, SjavsResult::OpponentDoubleWin);
    }

    #[test]
    fn test_avoiding_double_loss() {
        let scoring = SjavsScoring {
            trump_team_points: 35,
            opponent_team_points: 85,
            trump_team_tricks: 2,
            opponent_team_tricks: 6,
            trump_suit: "hearts".to_string(),
            individual_vol: false,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 0);
        assert_eq!(result.opponent_team_score, 4);
        assert_eq!(result.result_type, SjavsResult::OpponentWin);
        assert!(scoring.is_avoiding_double_loss());
    }

    #[test]
    fn test_trump_team_high_win() {
        let scoring = SjavsScoring {
            trump_team_points: 95,
            opponent_team_points: 25,
            trump_team_tricks: 6,
            opponent_team_tricks: 2,
            trump_suit: "hearts".to_string(),
            individual_vol: false,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 4);
        assert_eq!(result.opponent_team_score, 0);
        assert_eq!(result.result_type, SjavsResult::TrumpTeamWin);
    }

    #[test]
    fn test_trump_team_normal_win() {
        let scoring = SjavsScoring {
            trump_team_points: 75,
            opponent_team_points: 45,
            trump_team_tricks: 5,
            opponent_team_tricks: 3,
            trump_suit: "hearts".to_string(),
            individual_vol: false,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 2);
        assert_eq!(result.opponent_team_score, 0);
        assert_eq!(result.result_type, SjavsResult::TrumpTeamWin);
    }

    #[test]
    fn test_opponent_vol() {
        let scoring = SjavsScoring {
            trump_team_points: 0,
            opponent_team_points: 120,
            trump_team_tricks: 0,
            opponent_team_tricks: 8,
            trump_suit: "hearts".to_string(),
            individual_vol: false,
        };

        let result = scoring.calculate_game_result();
        assert_eq!(result.trump_team_score, 0);
        assert_eq!(result.opponent_team_score, 16);
        assert_eq!(result.result_type, SjavsResult::OpponentVol);
    }

    #[test]
    fn test_point_validation() {
        let valid_scoring = SjavsScoring {
            trump_team_points: 75,
            opponent_team_points: 45,
            trump_team_tricks: 5,
            opponent_team_tricks: 3,
            trump_suit: "hearts".to_string(),
            individual_vol: false,
        };

        assert!(valid_scoring.validate_total_points());

        let invalid_scoring = SjavsScoring {
            trump_team_points: 75,
            opponent_team_points: 50, // Should be 45 to total 120
            trump_team_tricks: 5,
            opponent_team_tricks: 3,
            trump_suit: "hearts".to_string(),
            individual_vol: false,
        };

        assert!(!invalid_scoring.validate_total_points());
    }
}
