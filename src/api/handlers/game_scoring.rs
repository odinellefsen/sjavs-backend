use crate::api::schemas::{
    CrossScores, CrossWinner, ErrorResponse, GameCompleteResponse, GameScoreInfo,
    GameScoringResult, PlayerInfo,
};
use crate::game::scoring::{GameResult, SjavsResult, SjavsScoring};
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
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Handle game completion and apply Sjavs scoring
/// This endpoint is automatically called when the 8th trick completes
#[utoipa::path(
    post,
    path = "/game/complete",
    tag = "Game Management",
    security(
        ("jwt_auth" = [])
    ),
    responses(
        (status = 200, description = "Game completed and scored", body = GameCompleteResponse),
        (status = 400, description = "Game not complete or invalid state", body = ErrorResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    summary = "Complete game and apply scoring",
    description = "Calculates final Sjavs scoring and updates cross/rubber totals. Called automatically after 8th trick."
)]
#[axum::debug_handler]
pub async fn complete_game_handler(
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

    // Get the match details
    let mut game_match = match NormalMatchRepository::get_by_id(&mut conn, &game_id).await {
        Ok(Some(game_match)) => game_match,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Game not found".to_string(),
                    message: None,
                }),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get game: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    // Get final trick state
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

    // Validate game is complete
    if !trick_state.game_complete {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Game not complete".to_string(),
                message: Some("All 8 tricks must be played before scoring".to_string()),
            }),
        )
            .into_response();
    }

    // Calculate Sjavs scoring
    let sjavs_scoring = match trick_state.get_final_scoring() {
        Ok(scoring) => scoring,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to calculate scoring: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    // Validate point totals (should always equal 120)
    if !sjavs_scoring.validate_total_points() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Invalid point totals in completed game".to_string(),
                message: Some(format!(
                    "Trump team: {}, Opponents: {}, Total: {}",
                    sjavs_scoring.trump_team_points,
                    sjavs_scoring.opponent_team_points,
                    sjavs_scoring.trump_team_points + sjavs_scoring.opponent_team_points
                )),
            }),
        )
            .into_response();
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
    if let Err(e) = NormalMatchRepository::update(&mut conn, &game_match).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to update match status: {}", e),
                message: None,
            }),
        )
            .into_response();
    }

    // Clear trick state
    if let Err(e) = TrickStateRepository::clear_trick_state(&mut conn, &game_id).await {
        eprintln!("Failed to clear trick state: {}", e);
    }

    // Broadcast game completion
    if let Err(e) = broadcast_game_complete(&mut conn, &game_id, &game_result, &cross_scores).await
    {
        eprintln!("Failed to broadcast game completion: {}", e);
    }

    let scoring_result = GameScoringResult {
        trump_team_points: sjavs_scoring.trump_team_points,
        opponent_team_points: sjavs_scoring.opponent_team_points,
        trump_team_tricks: sjavs_scoring.trump_team_tricks,
        opponent_team_tricks: sjavs_scoring.opponent_team_tricks,
        trump_suit: sjavs_scoring.trump_suit,
        result_type: format!("{:?}", game_result.result_type),
        description: game_result.description,
        trump_team_score: game_result.trump_team_score,
        opponent_team_score: game_result.opponent_team_score,
        individual_vol: sjavs_scoring.individual_vol,
    };

    let response = GameCompleteResponse {
        message: "Game completed and scored successfully".to_string(),
        game_id: game_id.clone(),
        scoring: scoring_result,
        cross_scores,
        cross_won: None,       // Will be implemented in Step 5
        new_game_ready: false, // Will be implemented in Step 5
    };

    (StatusCode::OK, Json(response)).into_response()
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
