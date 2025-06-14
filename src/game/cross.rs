use crate::game::scoring::GameResult;
use serde::{Deserialize, Serialize};

/// Cross state tracking for a match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossState {
    /// Current score for trump team (starts at 24, counts down)
    pub trump_team_score: i8,
    /// Current score for opponent team (starts at 24, counts down)
    pub opponent_team_score: i8,
    /// Number of crosses won by trump team
    pub trump_team_crosses: u8,
    /// Number of crosses won by opponent team
    pub opponent_team_crosses: u8,
    /// Bonus points for next game (from ties)
    pub next_game_bonus: u8,
    /// Match ID this cross state belongs to
    pub match_id: String,
    /// Whether the cross is complete
    pub cross_complete: bool,
}

impl CrossState {
    /// Create new cross state for a match
    pub fn new(match_id: String) -> Self {
        Self {
            trump_team_score: 24,
            opponent_team_score: 24,
            trump_team_crosses: 0,
            opponent_team_crosses: 0,
            next_game_bonus: 0,
            match_id,
            cross_complete: false,
        }
    }

    /// Apply game result to cross scores
    pub fn apply_game_result(&mut self, game_result: &GameResult) -> CrossResult {
        // Apply bonus points if any
        let trump_team_points = game_result.trump_team_score + self.next_game_bonus;
        let opponent_team_points = game_result.opponent_team_score;

        // Reset bonus after use
        let bonus_applied = self.next_game_bonus;
        self.next_game_bonus = 0;

        // Handle tie scenario (both teams get 60 points)
        if game_result.trump_team_score == 0 && game_result.opponent_team_score == 0 {
            // This was a tie - add 2 to next game bonus
            self.next_game_bonus = 2;
            return CrossResult {
                trump_team_old_score: self.trump_team_score,
                opponent_team_old_score: self.opponent_team_score,
                trump_team_new_score: self.trump_team_score,
                opponent_team_new_score: self.opponent_team_score,
                cross_won: None,
                bonus_applied,
                next_game_bonus: self.next_game_bonus,
                cross_complete: false,
            };
        }

        let old_trump_score = self.trump_team_score;
        let old_opponent_score = self.opponent_team_score;

        // Subtract points from respective teams
        self.trump_team_score -= trump_team_points as i8;
        self.opponent_team_score -= opponent_team_points as i8;

        // Check for cross completion
        let cross_won = self.check_cross_completion();

        CrossResult {
            trump_team_old_score: old_trump_score,
            opponent_team_old_score: old_opponent_score,
            trump_team_new_score: self.trump_team_score,
            opponent_team_new_score: self.opponent_team_score,
            cross_won,
            bonus_applied,
            next_game_bonus: self.next_game_bonus,
            cross_complete: self.cross_complete,
        }
    }

    /// Check if a cross has been completed
    fn check_cross_completion(&mut self) -> Option<CrossWinner> {
        // Check if trump team won
        if self.trump_team_score <= 0 {
            // Check for double victory
            let double_victory = self.opponent_team_score == 24;

            self.trump_team_crosses += 1;
            self.cross_complete = true;

            return Some(CrossWinner {
                winning_team: CrossTeam::TrumpTeam,
                double_victory,
                final_score: (self.trump_team_score, self.opponent_team_score),
                crosses_won: self.trump_team_crosses,
            });
        }

        // Check if opponent team won
        if self.opponent_team_score <= 0 {
            // Check for double victory
            let double_victory = self.trump_team_score == 24;

            self.opponent_team_crosses += 1;
            self.cross_complete = true;

            return Some(CrossWinner {
                winning_team: CrossTeam::OpponentTeam,
                double_victory,
                final_score: (self.trump_team_score, self.opponent_team_score),
                crosses_won: self.opponent_team_crosses,
            });
        }

        None
    }

    /// Check if teams are "on the hook" (6 points remaining)
    pub fn get_hook_status(&self) -> (bool, bool) {
        (self.trump_team_score == 6, self.opponent_team_score == 6)
    }

    /// Get cross summary for display
    pub fn get_summary(&self) -> CrossSummary {
        let (trump_on_hook, opponent_on_hook) = self.get_hook_status();

        CrossSummary {
            trump_team_score: self.trump_team_score,
            opponent_team_score: self.opponent_team_score,
            trump_team_crosses: self.trump_team_crosses,
            opponent_team_crosses: self.opponent_team_crosses,
            trump_team_on_hook: trump_on_hook,
            opponent_team_on_hook: opponent_on_hook,
            next_game_bonus: self.next_game_bonus,
            cross_complete: self.cross_complete,
        }
    }

    /// Start new game within cross (if not complete)
    pub fn start_new_game(&mut self) -> Result<(), String> {
        if self.cross_complete {
            return Err("Cross is complete - cannot start new game".to_string());
        }
        // Scores remain as they are for next game
        // Only bonus points are reset (handled in apply_game_result)
        Ok(())
    }

    /// Reset for completely new cross
    pub fn reset_for_new_cross(&mut self) {
        self.trump_team_score = 24;
        self.opponent_team_score = 24;
        self.trump_team_crosses = 0;
        self.opponent_team_crosses = 0;
        self.next_game_bonus = 0;
        self.cross_complete = false;
    }
}

/// Result of applying a game result to cross state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossResult {
    /// Trump team score before this game
    pub trump_team_old_score: i8,
    /// Opponent team score before this game
    pub opponent_team_old_score: i8,
    /// Trump team score after this game
    pub trump_team_new_score: i8,
    /// Opponent team score after this game
    pub opponent_team_new_score: i8,
    /// Cross winner if any
    pub cross_won: Option<CrossWinner>,
    /// Bonus points applied this game
    pub bonus_applied: u8,
    /// Bonus points for next game
    pub next_game_bonus: u8,
    /// Whether the cross is complete
    pub cross_complete: bool,
}

/// Information about cross winner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossWinner {
    /// Which team won the cross
    pub winning_team: CrossTeam,
    /// Whether it was a double victory (opponent still at 24)
    pub double_victory: bool,
    /// Final scores when cross was won
    pub final_score: (i8, i8), // (trump_team, opponent_team)
    /// Number of crosses this team has won
    pub crosses_won: u8,
}

/// Teams in cross scoring
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrossTeam {
    TrumpTeam,
    OpponentTeam,
}

/// Summary of cross state for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSummary {
    /// Current trump team score
    pub trump_team_score: i8,
    /// Current opponent team score
    pub opponent_team_score: i8,
    /// Crosses won by trump team
    pub trump_team_crosses: u8,
    /// Crosses won by opponent team
    pub opponent_team_crosses: u8,
    /// Whether trump team is on the hook
    pub trump_team_on_hook: bool,
    /// Whether opponent team is on the hook
    pub opponent_team_on_hook: bool,
    /// Bonus for next game
    pub next_game_bonus: u8,
    /// Whether cross is complete
    pub cross_complete: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::scoring::{GameResult, SjavsResult};

    #[test]
    fn test_new_cross_state() {
        let cross = CrossState::new("test_match".to_string());
        assert_eq!(cross.trump_team_score, 24);
        assert_eq!(cross.opponent_team_score, 24);
        assert_eq!(cross.trump_team_crosses, 0);
        assert_eq!(cross.opponent_team_crosses, 0);
        assert!(!cross.cross_complete);
    }

    #[test]
    fn test_apply_normal_win() {
        let mut cross = CrossState::new("test".to_string());

        let game_result = GameResult {
            trump_team_score: 4,
            opponent_team_score: 0,
            result_type: SjavsResult::TrumpTeamWin,
            description: "Trump team won".to_string(),
        };

        let result = cross.apply_game_result(&game_result);
        assert_eq!(cross.trump_team_score, 20); // 24 - 4
        assert_eq!(cross.opponent_team_score, 24); // unchanged
        assert!(result.cross_won.is_none());
    }

    #[test]
    fn test_cross_completion() {
        let mut cross = CrossState::new("test".to_string());
        cross.trump_team_score = 4; // Close to winning

        let game_result = GameResult {
            trump_team_score: 8,
            opponent_team_score: 0,
            result_type: SjavsResult::TrumpTeamWin,
            description: "Trump team won big".to_string(),
        };

        let result = cross.apply_game_result(&game_result);
        assert_eq!(cross.trump_team_score, -4); // 4 - 8 = -4 (won)
        assert!(result.cross_won.is_some());
        assert_eq!(result.cross_won.unwrap().winning_team, CrossTeam::TrumpTeam);
        assert!(cross.cross_complete);
    }

    #[test]
    fn test_double_victory() {
        let mut cross = CrossState::new("test".to_string());
        cross.trump_team_score = 4;
        // opponent_team_score stays at 24

        let game_result = GameResult {
            trump_team_score: 8,
            opponent_team_score: 0,
            result_type: SjavsResult::TrumpTeamWin,
            description: "Trump team won".to_string(),
        };

        let result = cross.apply_game_result(&game_result);
        let winner = result.cross_won.unwrap();
        assert!(winner.double_victory); // Opponent still at 24
    }

    #[test]
    fn test_tie_bonus() {
        let mut cross = CrossState::new("test".to_string());

        // First game: tie
        let tie_result = GameResult {
            trump_team_score: 0,
            opponent_team_score: 0,
            result_type: SjavsResult::Tie,
            description: "Tie game".to_string(),
        };

        let result = cross.apply_game_result(&tie_result);
        assert_eq!(cross.next_game_bonus, 2);
        assert_eq!(result.next_game_bonus, 2);

        // Second game: trump team wins with bonus
        let win_result = GameResult {
            trump_team_score: 4,
            opponent_team_score: 0,
            result_type: SjavsResult::TrumpTeamWin,
            description: "Trump team won".to_string(),
        };

        let result2 = cross.apply_game_result(&win_result);
        assert_eq!(cross.trump_team_score, 18); // 24 - (4 + 2 bonus) = 18
        assert_eq!(result2.bonus_applied, 2);
        assert_eq!(cross.next_game_bonus, 0); // Reset after use
    }

    #[test]
    fn test_on_the_hook() {
        let mut cross = CrossState::new("test".to_string());
        cross.trump_team_score = 6;
        cross.opponent_team_score = 6;

        let (trump_hook, opponent_hook) = cross.get_hook_status();
        assert!(trump_hook);
        assert!(opponent_hook);
    }

    #[test]
    fn test_vol_scenario() {
        let mut cross = CrossState::new("test".to_string());
        cross.trump_team_score = 8; // Close to winning

        // Vol in clubs (16 points)
        let vol_result = GameResult {
            trump_team_score: 16,
            opponent_team_score: 0,
            result_type: SjavsResult::Vol,
            description: "Vol in clubs".to_string(),
        };

        let result = cross.apply_game_result(&vol_result);
        assert_eq!(cross.trump_team_score, -8); // 8 - 16 = -8 (won decisively)
        assert!(result.cross_won.is_some());
        assert!(cross.cross_complete);
    }

    #[test]
    fn test_opponent_vol() {
        let mut cross = CrossState::new("test".to_string());
        cross.opponent_team_score = 10; // Close to winning

        // Opponents win all tricks
        let opponent_vol_result = GameResult {
            trump_team_score: 0,
            opponent_team_score: 16,
            result_type: SjavsResult::OpponentVol,
            description: "Opponents won all tricks".to_string(),
        };

        let result = cross.apply_game_result(&opponent_vol_result);
        assert_eq!(cross.opponent_team_score, -6); // 10 - 16 = -6 (won)
        assert!(result.cross_won.is_some());
        assert_eq!(
            result.cross_won.unwrap().winning_team,
            CrossTeam::OpponentTeam
        );
        assert!(cross.cross_complete);
    }
}
