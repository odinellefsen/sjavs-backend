use crate::api::schemas::{
    CrossScores, ErrorResponse, GameCompleteResponse, GameScoreInfo, GameScoringResult,
};
use crate::game::scoring::GameResult;
use crate::redis::normal_match::id::NormalMatchStatus;
use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::redis::player::repository::PlayerRepository;
use crate::redis::pubsub::broadcasting;
use crate::redis::trick_state::repository::TrickStateRepository;
use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};

/// Internal function to handle game completion and apply Sjavs scoring
/// Called automatically when the 8th trick completes in the card playing logic
pub async fn handle_game_completion(
    conn: &mut deadpool_redis::Connection,
    game_id: String,
) -> Result<(GameCompleteResponse, GameResult, CrossScores), String> {
    // Get the match details
    let mut game_match = match NormalMatchRepository::get_by_id(conn, &game_id).await {
        Ok(Some(game_match)) => game_match,
        Ok(None) => {
            return Err("Game not found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to get game: {}", e));
        }
    };

    // Get final trick state
    let trick_state = match TrickStateRepository::get_trick_state(conn, &game_id).await {
        Ok(Some(state)) => state,
        Ok(None) => {
            return Err("Trick state not found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to get trick state: {}", e));
        }
    };

    // Validate game is complete
    if !trick_state.game_complete {
        return Err("Game not complete - All 8 tricks must be played before scoring".to_string());
    }

    // Calculate Sjavs scoring
    let sjavs_scoring = match trick_state.get_final_scoring() {
        Ok(scoring) => scoring,
        Err(e) => {
            return Err(format!("Failed to calculate scoring: {}", e));
        }
    };

    // Validate point totals (should always equal 120)
    if !sjavs_scoring.validate_total_points() {
        return Err(format!(
            "Invalid point totals in completed game: Trump team: {}, Opponents: {}, Total: {}",
            sjavs_scoring.trump_team_points,
            sjavs_scoring.opponent_team_points,
            sjavs_scoring.trump_team_points + sjavs_scoring.opponent_team_points
        ));
    }

    let game_result = sjavs_scoring.calculate_game_result();

    // Log the game result for debugging
    println!(
        "Game {} completed: {} - trump team: {} points, opponents: {} points",
        game_id,
        game_result.description,
        game_result.trump_team_score,
        game_result.opponent_team_score
    );

    // TODO: Apply cross/rubber scoring (Step 5)
    // For now, create placeholder cross scores
    let cross_scores = CrossScores {
        trump_team_remaining: 24, // Will be updated in Step 5
        opponent_team_remaining: 24,
        trump_team_on_hook: false,
        opponent_team_on_hook: false,
        trump_team_crosses: 0,
        opponent_team_crosses: 0,
    };

    // Transition match to completed state
    game_match.status = NormalMatchStatus::Completed;
    if let Err(e) = NormalMatchRepository::update(conn, &game_match).await {
        return Err(format!("Failed to update match status: {}", e));
    }

    // Clear trick state
    if let Err(e) = TrickStateRepository::clear_trick_state(conn, &game_id).await {
        eprintln!("Failed to clear trick state: {}", e);
    }

    // Broadcast game completion
    if let Err(e) = broadcast_game_complete(conn, &game_id, &game_result, &cross_scores).await {
        eprintln!("Failed to broadcast game completion: {}", e);
    }

    let scoring_result = GameScoringResult {
        trump_team_points: sjavs_scoring.trump_team_points,
        opponent_team_points: sjavs_scoring.opponent_team_points,
        trump_team_tricks: sjavs_scoring.trump_team_tricks,
        opponent_team_tricks: sjavs_scoring.opponent_team_tricks,
        trump_suit: sjavs_scoring.trump_suit,
        result_type: format!("{:?}", game_result.result_type),
        description: game_result.description.clone(),
        trump_team_score: game_result.trump_team_score,
        opponent_team_score: game_result.opponent_team_score,
        individual_vol: sjavs_scoring.individual_vol,
    };

    let response = GameCompleteResponse {
        message: "Game completed and scored successfully".to_string(),
        game_id: game_id.clone(),
        scoring: scoring_result,
        cross_scores: cross_scores,
        cross_won: None,       // Will be implemented in Step 5
        new_game_ready: false, // Will be implemented in Step 5
    };

    // Create cross_scores copy for return
    let cross_scores_copy = CrossScores {
        trump_team_remaining: 24,
        opponent_team_remaining: 24,
        trump_team_on_hook: false,
        opponent_team_on_hook: false,
        trump_team_crosses: 0,
        opponent_team_crosses: 0,
    };

    Ok((response, game_result, cross_scores_copy))
}

/// Get current game scoring (for ongoing games)
#[utoipa::path(
    get,
    path = "/game/score",
    tag = "Game Management",
    security(
        ("jwt_auth" = [])
    ),
    responses(
        (status = 200, description = "Current game score retrieved", body = GameScoreInfo),
        (status = 404, description = "Game not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    summary = "Get current game score",
    description = "Returns current trick and point totals for the ongoing game"
)]
#[axum::debug_handler]
pub async fn get_current_score_handler(
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
) -> Response {
    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get Redis connection: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    // Get the player's current game
    let game_id = match PlayerRepository::get_player_game(&mut conn, &user_id).await {
        Ok(Some(game_id)) => game_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Not in a game".to_string(),
                    message: None,
                }),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get player game: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    // Get trick state
    let trick_state = match TrickStateRepository::get_trick_state(&mut conn, &game_id).await {
        Ok(Some(state)) => state,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Trick state not found".to_string(),
                    message: None,
                }),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get trick state: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    // Create trick info
    let trick_info = crate::api::schemas::GameTrickInfo {
        current_trick_number: trick_state.current_trick.trick_number,
        cards_played_in_trick: trick_state.current_trick.cards_played.len() as u8,
        current_player: if trick_state.current_trick.is_complete {
            None
        } else {
            Some(trick_state.current_trick.current_player as u8)
        },
        trick_complete: trick_state.current_trick.is_complete,
        trick_winner: trick_state.current_trick.trick_winner.map(|w| w as u8),
        trump_team_tricks: trick_state.tricks_won.0,
        opponent_team_tricks: trick_state.tricks_won.1,
        trump_team_points: trick_state.points_accumulated.0,
        opponent_team_points: trick_state.points_accumulated.1,
        game_complete: trick_state.game_complete,
    };

    // Placeholder cross scores for Step 5
    let cross_scores = CrossScores {
        trump_team_remaining: 24,
        opponent_team_remaining: 24,
        trump_team_on_hook: false,
        opponent_team_on_hook: false,
        trump_team_crosses: 0,
        opponent_team_crosses: 0,
    };

    let response = GameScoreInfo {
        game_id,
        trick_info,
        cross_scores,
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Broadcast game completion with final scoring
async fn broadcast_game_complete(
    conn: &mut deadpool_redis::Connection,
    game_id: &str,
    game_result: &GameResult,
    cross_scores: &CrossScores,
) -> Result<(), String> {
    let event_data = serde_json::json!({
        "type": "game_complete",
        "trump_team_score": game_result.trump_team_score,
        "opponent_team_score": game_result.opponent_team_score,
        "result_type": format!("{:?}", game_result.result_type),
        "description": game_result.description,
        "cross_scores": cross_scores,
        "timestamp": chrono::Utc::now().timestamp()
    });

    broadcasting::broadcast_to_game(conn, game_id, &event_data).await
}
