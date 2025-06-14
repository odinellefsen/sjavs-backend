use crate::api::schemas::{ErrorResponse, GameStartState, PlayerInfo, StartGameResponse};
use crate::game::deck::Deck;
use crate::game::hand::Hand;
use crate::redis::game_state::repository::GameStateRepository;
use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::redis::player::repository::{PlayerGameInfo, PlayerRepository};
use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};
use rand::Rng;

/// Start a Sjavs game
///
/// Transitions a match from Waiting to Bidding state by dealing cards to all players.
/// Only the host can start the game, and exactly 4 players must be in the match.
/// Cards are automatically dealt until at least one player has a valid bidding hand (5+ trumps).
#[utoipa::path(
    post,
    path = "/game/start",
    tag = "Game Management", 
    security(
        ("jwt_auth" = [])
    ),
    responses(
        (status = 200, description = "Game started successfully", body = StartGameResponse),
        (status = 400, description = "Invalid game state or insufficient players", body = ErrorResponse),
        (status = 403, description = "Only host can start the game", body = ErrorResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    summary = "Start a game",
    description = "Transitions the match from waiting to bidding state. Automatically deals cards until valid hands exist. Only the host can start the game with exactly 4 players."
)]
#[axum::debug_handler]
pub async fn start_game_handler(
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
                    message: Some("You must be in a game to start it".to_string()),
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

    // Check if the user is the host
    let players = match PlayerRepository::get_players_in_game(&mut conn, &game_id).await {
        Ok(players) => players,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get players: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    let host_player = players.iter().find(|p| p.role == "host");
    if host_player.is_none() || host_player.unwrap().user_id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Only host can start the game".to_string(),
                message: Some("You must be the host to start the game".to_string()),
            }),
        )
            .into_response();
    }

    // Validate that we have exactly 4 players
    if players.len() != 4 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid player count".to_string(),
                message: Some(format!(
                    "Need exactly 4 players to start, but have {}",
                    players.len()
                )),
            }),
        )
            .into_response();
    }

    // Check if game can be started (must be in Waiting state)
    if !game_match.can_start() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Game cannot be started".to_string(),
                message: Some(format!(
                    "Game is in '{}' state, but must be 'Waiting' to start",
                    game_match.status.to_string()
                )),
            }),
        )
            .into_response();
    }

    // Choose random dealer position (0-3)
    let dealer_position = rand::thread_rng().gen_range(0..4);

    // Start dealing phase
    game_match.start_dealing(dealer_position);

    // Deal cards until we get valid hands (this may take multiple attempts)
    let dealing_start = std::time::Instant::now();
    let mut attempts = 0;

    loop {
        attempts += 1;

        // Safety check to prevent infinite loops
        if attempts > 1000 {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Unable to deal valid hands".to_string(),
                    message: Some("Exceeded maximum dealing attempts".to_string()),
                }),
            )
                .into_response();
        }

        // Deal cards
        let hands = Deck::deal_until_valid();

        // Convert to Hand objects with proper player positions
        let hand_objects: [Hand; 4] = [
            Hand::new(hands[0].clone(), 0),
            Hand::new(hands[1].clone(), 1),
            Hand::new(hands[2].clone(), 2),
            Hand::new(hands[3].clone(), 3),
        ];

        // Store hands in Redis
        if let Err(e) = GameStateRepository::store_hands(&mut conn, &game_id, &hand_objects).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to store hands: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }

        // Hands are valid by design (deal_until_valid ensures this), so we can break
        break;
    }

    let dealing_duration = dealing_start.elapsed();
    println!(
        "Dealt valid hands after {} attempts in {:?}",
        attempts, dealing_duration
    );

    // Transition to bidding state
    game_match.start_bidding();

    // Update the match in Redis
    if let Err(e) = NormalMatchRepository::update(&mut conn, &game_match).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to update game state: {}", e),
                message: None,
            }),
        )
            .into_response();
    }

    // Store hand analysis for debugging
    let analysis = Deck::analyze_hands(&[
        Hand::new(vec![], 0).cards, // We'll get the actual hands from Redis if needed
        Hand::new(vec![], 1).cards,
        Hand::new(vec![], 2).cards,
        Hand::new(vec![], 3).cards,
    ]);
    let _ = GameStateRepository::store_hand_analysis(&mut conn, &game_id, &analysis).await;

    // Prepare response
    let game_state = GameStartState {
        id: game_match.id.clone(),
        status: game_match.status.to_string(),
        dealer_position: game_match.dealer_position.unwrap_or(0) as u8,
        current_bidder: game_match.current_bidder.unwrap_or(0) as u8,
        trump_suit: game_match.trump_suit.clone(),
        trump_declarer: game_match.trump_declarer.map(|pos| pos as u8),
        players: players
            .into_iter()
            .map(|p| PlayerInfo {
                user_id: p.user_id,
                role: p.role,
            })
            .collect(),
    };

    let response = StartGameResponse {
        message: "Game started successfully".to_string(),
        game_id: game_id.clone(),
        state: game_state,
        hands_dealt: true,
        dealing_attempts: attempts,
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Get player's hand
///
/// Returns the authenticated player's hand for the current game.
/// Only returns the hand if the game is in Bidding or Playing state.
#[utoipa::path(
    get,
    path = "/game/hand",
    tag = "Game Management",
    security(
        ("jwt_auth" = [])
    ),
    responses(
        (status = 200, description = "Player hand retrieved", body = crate::api::schemas::PlayerHandResponse),
        (status = 400, description = "Invalid game state", body = ErrorResponse),
        (status = 404, description = "Game or hand not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    summary = "Get player's hand",
    description = "Returns the current player's hand with cards and bidding options"
)]
#[axum::debug_handler]
pub async fn get_player_hand_handler(
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
                    message: Some("You must be in a game to view your hand".to_string()),
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

    // Get the match to check state
    let game_match = match NormalMatchRepository::get_by_id(&mut conn, &game_id).await {
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

    // Check if game is in a state where hands should be visible
    if !game_match.is_active() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Game not active".to_string(),
                message: Some(format!(
                    "Game is in '{}' state. Hands are only visible during Bidding or Playing",
                    game_match.status.to_string()
                )),
            }),
        )
            .into_response();
    }

    // Get player position in game
    let players = match PlayerRepository::get_players_in_game(&mut conn, &game_id).await {
        Ok(players) => players,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get players: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    let player_position = players
        .iter()
        .position(|p| p.user_id == user_id)
        .unwrap_or(0);

    // Get the player's hand
    let hand = match GameStateRepository::get_hand(&mut conn, &game_id, player_position).await {
        Ok(Some(hand)) => hand,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Hand not found".to_string(),
                    message: Some("Your hand has not been dealt yet".to_string()),
                }),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get hand: {}", e),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    // Get available bids (only if in bidding state)
    let available_bids = if game_match.status.to_string() == "Bidding" {
        hand.get_available_bids(game_match.highest_bid_length)
    } else {
        vec![]
    };

    // Convert to API format
    let trump_counts = hand.calculate_trump_counts();
    let can_bid = !available_bids.is_empty();

    let api_bids: Vec<crate::api::schemas::BidOption> = available_bids
        .into_iter()
        .map(|bid| crate::api::schemas::BidOption {
            length: bid.length,
            suit: bid.suit,
            display_text: bid.display_text,
            is_club_declaration: bid.is_club_declaration,
        })
        .collect();

    let response = crate::api::schemas::PlayerHandResponse {
        message: "Hand retrieved successfully".to_string(),
        game_id: game_id.clone(),
        player_position: player_position as u8,
        cards: hand.to_codes(),
        trump_counts,
        available_bids: api_bids,
        can_bid,
    };

    (StatusCode::OK, Json(response)).into_response()
}
