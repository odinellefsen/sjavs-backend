use crate::api::schemas::{CreateMatchResponse, ErrorResponse, MatchState};
use crate::redis::normal_match::id::NormalMatch;
use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::redis::player::repository::PlayerRepository;
use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};
use rand::Rng;

/// Create a new Sjavs match
///
/// Creates a new match with a unique game ID and 4-digit PIN code.
/// The requesting user becomes the host of the match.
/// Only one active match per player is allowed.
#[utoipa::path(
    post,
    path = "/normal-match",
    tag = "Match Management",
    security(
        ("jwt_auth" = [])
    ),
    responses(
        (status = 201, description = "Match created successfully", body = CreateMatchResponse),
        (status = 409, description = "Player already in an active game", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    summary = "Create a new match",
    description = "Creates a new Sjavs match. The authenticated user becomes the host. Returns a PIN code that other players can use to join."
)]
#[axum::debug_handler]
pub async fn create_match_handler(
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
) -> Response {
    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get Redis connection from pool: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    // Check if player is already in a game using repository
    match PlayerRepository::get_player_game(&mut conn, &user_id).await {
        Ok(Some(game_id)) => {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "Already in game".to_string(),
                    message: Some(format!(
                        "You are already in game {}. Please leave or finish your current game before creating a new one.",
                        game_id
                    )),
                }),
            ).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to check player game status: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
        _ => {} // Continue if player is not in a game
    }

    // Generate new game ID (timestamp + random suffix)
    let game_id = format!(
        "game_{}_{:x}",
        chrono::Utc::now().timestamp(),
        rand::random::<u16>()
    );

    // Generate random 4-digit PIN code
    let pin_code = rand::thread_rng().gen_range(1000..=9999);

    // Create a new NormalMatch instance
    let normal_match = NormalMatch::new(
        game_id.clone(),
        pin_code,
        3, // Default number of crosses - adjust as needed
    );

    // Use repository to create the match in Redis
    if let Err(e) = NormalMatchRepository::create(&mut conn, &normal_match, &user_id).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create match: {}", e),
                message: None,
            }),
        )
            .into_response();
    }

    // Verify the match was created successfully
    match NormalMatchRepository::get_by_id(&mut conn, &game_id).await {
        Ok(Some(stored_match)) => {
            // Check if player is properly associated with the game
            match PlayerRepository::get_player_game(&mut conn, &user_id).await {
                Ok(Some(player_game)) if player_game == game_id => {
                    // All verifications passed, return success
                    let response = CreateMatchResponse {
                        message: "Game created and verified".to_string(),
                        game_id: game_id.clone(),
                        game_pin: pin_code,
                        state: MatchState {
                            id: stored_match.id,
                            pin: stored_match.pin,
                            status: stored_match.status.to_string(),
                            number_of_crosses: stored_match.number_of_crosses,
                            current_cross: stored_match.current_cross,
                            created_timestamp: stored_match.created_timestamp,
                            host: user_id,
                        },
                    };

                    return (StatusCode::CREATED, Json(response)).into_response();
                }
                _ => {
                    // Player-game association verification failed
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Game creation verification failed".to_string(),
                            message: Some("Player-game association not found".to_string()),
                        }),
                    )
                        .into_response();
                }
            }
        }
        Ok(None) => {
            // Game not found after creation
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Game creation verification failed".to_string(),
                    message: Some("Game not found after creation".to_string()),
                }),
            )
                .into_response();
        }
        Err(e) => {
            // Error during verification
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Game creation verification failed: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    }
}
