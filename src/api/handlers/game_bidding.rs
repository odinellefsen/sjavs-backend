use crate::api::schemas::{
    BidDetails, BidRequest, BidResponse, BiddingCompleteResponse, BiddingGameState, ErrorResponse,
    GameStartState, PartnershipInfo, PassResponse, PlayerInfo,
};
use crate::game::deck::Deck;
use crate::game::hand::Hand;
use crate::redis::game_state::repository::GameStateRepository;
use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::redis::player::repository::{PlayerGameInfo, PlayerRepository};
use crate::redis::pubsub::broadcasting;
use crate::RedisPool;
use axum::http::StatusCode;
use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Response},
    Json,
};

/// Make a bid in the current game
///
/// Allows a player to make a trump bid during the bidding phase.
/// The bid must be higher than the current highest bid, or clubs to match same length.
/// Validates turn order and bid legality according to Sjavs rules.
#[utoipa::path(
    post,
    path = "/game/bid",
    tag = "Game Management",
    security(
        ("jwt_auth" = [])
    ),
    request_body = BidRequest,
    responses(
        (status = 200, description = "Bid made successfully", body = BidResponse),
        (status = 400, description = "Invalid bid or game state", body = ErrorResponse),
        (status = 403, description = "Not your turn to bid", body = ErrorResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    summary = "Make a bid",
    description = "Make a trump bid. Must be your turn and bid must follow Sjavs rules (higher than current, or clubs to match)."
)]
#[axum::debug_handler]
pub async fn make_bid_handler(
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
    Json(bid_request): Json<BidRequest>,
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
                    message: Some("You must be in a game to make a bid".to_string()),
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

    // Get player position
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

    let player_position = match players.iter().position(|p| p.user_id == user_id) {
        Some(pos) => pos,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Player not in game".to_string(),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    // Validate the player has the required trumps for their bid
    let hand = match GameStateRepository::get_hand(&mut conn, &game_id, player_position).await {
        Ok(Some(hand)) => hand,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
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

    // Check if player has enough trumps for their bid
    let trump_counts = hand.calculate_trump_counts();
    let player_trump_count = trump_counts.get(&bid_request.suit).copied().unwrap_or(0);

    if player_trump_count < bid_request.length {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Insufficient trumps".to_string(),
                message: Some(format!(
                    "You only have {} {} trumps, but bid {} trumps",
                    player_trump_count, bid_request.suit, bid_request.length
                )),
            }),
        )
            .into_response();
    }

    // Make the bid
    match game_match.make_bid(
        player_position,
        bid_request.length,
        bid_request.suit.clone(),
    ) {
        Ok(_) => {
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

            // Broadcast bid made event via WebSocket
            if let Err(e) = broadcasting::broadcast_bid_made(
                &mut conn,
                &game_id,
                player_position as u8,
                bid_request.length,
                &bid_request.suit,
                game_match.current_bidder.unwrap_or(0) as u8,
            )
            .await
            {
                eprintln!("Failed to broadcast bid made event: {}", e);
                // Don't fail the request if broadcasting fails
            }

            // Create bid details
            let bid_details = BidDetails {
                length: bid_request.length,
                suit: bid_request.suit.clone(),
                is_club_declaration: bid_request.suit == "clubs",
                display_text: format!("{} trumps ({})", bid_request.length, bid_request.suit),
            };

            // Create game state
            let game_state = BiddingGameState {
                id: game_match.id.clone(),
                status: game_match.status.to_string(),
                dealer_position: game_match.dealer_position.unwrap_or(0) as u8,
                current_bidder: game_match.current_bidder.unwrap_or(0) as u8,
                highest_bid_length: game_match.highest_bid_length,
                highest_bidder: game_match.highest_bidder.map(|pos| pos as u8),
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

            let response = BidResponse {
                message: "Bid made successfully".to_string(),
                game_id: game_id.clone(),
                bidder_position: player_position as u8,
                bid: bid_details,
                game_state,
                bidding_complete: false, // Will be determined by subsequent passes
            };

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid bid".to_string(),
                    message: Some(e),
                }),
            )
                .into_response();
        }
    }
}

/// Pass on bidding in the current game
///
/// Allows a player to pass their turn during the bidding phase.
/// If all players pass, triggers a redeal. If 3 players pass after a bid, completes bidding.
#[utoipa::path(
    post,
    path = "/game/pass",
    tag = "Game Management",
    security(
        ("jwt_auth" = [])
    ),
    responses(
        (status = 200, description = "Pass recorded successfully", body = PassResponse),
        (status = 400, description = "Invalid game state", body = ErrorResponse),
        (status = 403, description = "Not your turn to bid", body = ErrorResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    summary = "Pass on bidding",
    description = "Pass your turn during bidding. May trigger redeal (all pass) or complete bidding (3 pass after bid)."
)]
#[axum::debug_handler]
pub async fn pass_bid_handler(
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
                    message: Some("You must be in a game to pass".to_string()),
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

    // Get player position
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

    let player_position = match players.iter().position(|p| p.user_id == user_id) {
        Some(pos) => pos,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Player not in game".to_string(),
                    message: None,
                }),
            )
                .into_response();
        }
    };

    // Make the pass
    match game_match.make_pass(player_position) {
        Ok((all_passed, bidding_complete)) => {
            if all_passed {
                // All players passed - need to redeal
                game_match.reset_for_redeal();

                // Clear existing hands
                if let Err(e) = GameStateRepository::clear_hands(&mut conn, &game_id).await {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to clear hands for redeal: {}", e),
                            message: None,
                        }),
                    )
                        .into_response();
                }

                // Deal new hands
                let hands = Deck::deal_until_valid();
                let hand_objects: [Hand; 4] = [
                    Hand::new(hands[0].clone(), 0),
                    Hand::new(hands[1].clone(), 1),
                    Hand::new(hands[2].clone(), 2),
                    Hand::new(hands[3].clone(), 3),
                ];

                if let Err(e) =
                    GameStateRepository::store_hands(&mut conn, &game_id, &hand_objects).await
                {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to store redealt hands: {}", e),
                            message: None,
                        }),
                    )
                        .into_response();
                }

                // Restart bidding
                game_match.start_bidding();
            } else if bidding_complete {
                // Bidding is complete - transition to playing
                if let Err(e) = game_match.finish_bidding() {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to complete bidding: {}", e),
                            message: None,
                        }),
                    )
                        .into_response();
                }
            }

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

            // Broadcast appropriate events via WebSocket
            if all_passed {
                // Broadcast redeal event
                if let Err(e) = broadcasting::broadcast_redeal(
                    &mut conn,
                    &game_id,
                    game_match.dealer_position.unwrap_or(0) as u8,
                    game_match.current_bidder.unwrap_or(0) as u8,
                )
                .await
                {
                    eprintln!("Failed to broadcast redeal event: {}", e);
                }
            } else if bidding_complete {
                // Broadcast bidding complete event
                if let (Some(trump_suit), Some(trump_declarer), Some(bid_length)) = (
                    &game_match.trump_suit,
                    game_match.trump_declarer,
                    game_match.highest_bid_length,
                ) {
                    if let Err(e) = broadcasting::broadcast_bidding_complete(
                        &mut conn,
                        &game_id,
                        trump_declarer as u8,
                        trump_suit,
                        bid_length,
                    )
                    .await
                    {
                        eprintln!("Failed to broadcast bidding complete event: {}", e);
                    }
                }
            } else {
                // Broadcast normal pass event
                if let Err(e) = broadcasting::broadcast_pass_made(
                    &mut conn,
                    &game_id,
                    player_position as u8,
                    game_match.current_bidder.unwrap_or(0) as u8,
                    all_passed,
                    bidding_complete,
                )
                .await
                {
                    eprintln!("Failed to broadcast pass made event: {}", e);
                }
            }

            // Create game state
            let game_state = BiddingGameState {
                id: game_match.id.clone(),
                status: game_match.status.to_string(),
                dealer_position: game_match.dealer_position.unwrap_or(0) as u8,
                current_bidder: game_match.current_bidder.unwrap_or(0) as u8,
                highest_bid_length: game_match.highest_bid_length,
                highest_bidder: game_match.highest_bidder.map(|pos| pos as u8),
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

            let response = PassResponse {
                message: if all_passed {
                    "All players passed - cards redealt".to_string()
                } else if bidding_complete {
                    "Bidding complete - game started".to_string()
                } else {
                    "Pass recorded".to_string()
                },
                game_id: game_id.clone(),
                passer_position: player_position as u8,
                game_state,
                all_passed,
                bidding_complete,
            };

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid pass".to_string(),
                    message: Some(e),
                }),
            )
                .into_response();
        }
    }
}
