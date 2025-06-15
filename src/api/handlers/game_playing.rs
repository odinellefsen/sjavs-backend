use crate::api::schemas::{
    CardPlayRequest, CardPlayResponse, ErrorResponse, GameTrickInfo, TrickSummaryResponse,
};
use crate::game::card::Card;
use crate::redis::{
    game_state::repository::GameStateRepository, normal_match::repository::NormalMatchRepository,
    player::repository::PlayerRepository, pubsub::broadcasting, trick_state::TrickStateRepository,
};
use crate::websocket::events::playing::TrickEvent;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use deadpool_redis::Connection;
use deadpool_redis::Pool as RedisPool;
use serde_json::json;

/// Play a card in the current trick
///
/// Allows a player to play a card from their hand during the trick-taking phase.
/// Validates the card play follows Sjavs rules (follow suit, legal card from hand).
#[utoipa::path(
    post,
    path = "/game/play-card",
    tag = "Game Playing",
    security(
        ("jwt_auth" = [])
    ),
    responses(
        (status = 200, description = "Card played successfully", body = CardPlayResponse),
        (status = 400, description = "Invalid card play", body = ErrorResponse),
        (status = 403, description = "Not your turn to play", body = ErrorResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    summary = "Play a card in the current trick",
    description = "Play a card from your hand. Must follow suit if possible. Auto-completes trick when 4 cards played."
)]
#[axum::debug_handler]
pub async fn play_card_handler(
    Extension(user_id): Extension<String>,
    State(redis_pool): State<RedisPool>,
    Json(card_request): Json<CardPlayRequest>,
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

    // 1. Get player's current game
    let game_id = match PlayerRepository::get_player_game(&mut conn, &user_id).await {
        Ok(Some(game_id)) => game_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Not in a game".to_string(),
                    message: Some("You must be in a game to play cards".to_string()),
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

    // 2. Validate game is in playing state
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

    if !game_match.is_playing() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Game is not in playing phase".to_string(),
                message: Some(format!("Game status: {}", game_match.status.to_string())),
            }),
        )
            .into_response();
    }

    // 3. Get player position
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

    // 4. Get current trick state
    let mut trick_state = match TrickStateRepository::get_trick_state(&mut conn, &game_id).await {
        Ok(Some(state)) => state,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No active trick found".to_string(),
                    message: Some("Trick state not initialized".to_string()),
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

    // 5. Validate it's the player's turn
    if trick_state.current_trick.current_player != player_position {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "Not your turn to play".to_string(),
                message: Some(format!(
                    "Current player to play is position {}",
                    trick_state.current_trick.current_player
                )),
            }),
        )
            .into_response();
    }

    // 6. Get player's hand and validate card
    let mut player_hand =
        match GameStateRepository::get_hand(&mut conn, &game_id, player_position).await {
            Ok(Some(hand)) => hand,
            Ok(None) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Player hand not found".to_string(),
                        message: None,
                    }),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to get player hand: {}", e),
                        message: None,
                    }),
                )
                    .into_response();
            }
        };

    // 7. Validate player has the card they want to play
    let card_to_play = match Card::from_code(&card_request.card) {
        Ok(card) => card,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid card code".to_string(),
                    message: Some(e),
                }),
            )
                .into_response();
        }
    };
    if !player_hand.has_card(&card_to_play) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Card not in hand".to_string(),
                message: Some(format!(
                    "You don't have the {} in your hand",
                    card_request.card
                )),
            }),
        )
            .into_response();
    }

    // 8. Validate it's a legal play (follow suit rules)
    let legal_cards = trick_state
        .current_trick
        .get_legal_cards(&player_hand.cards);
    if !legal_cards.contains(&card_to_play) {
        let lead_suit = match trick_state.current_trick.lead_suit {
            Some(suit) => suit.to_string(),
            None => "None".to_string(),
        };

        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Illegal card play".to_string(),
                message: Some(format!(
                    "Must follow suit ({}). Legal cards: {:?}",
                    lead_suit,
                    legal_cards
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                )),
            }),
        )
            .into_response();
    }

    // 9. Play the card in the trick
    if let Err(e) = trick_state
        .current_trick
        .play_card(player_position, card_to_play.clone())
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to play card: {}", e),
                message: None,
            }),
        )
            .into_response();
    }

    // 10. Remove card from player's hand
    player_hand.remove_card(&card_to_play);

    // 11. Save updated hand
    if let Err(e) =
        GameStateRepository::update_hand(&mut conn, &game_id, player_position, &player_hand).await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to update hand: {}", e),
                message: None,
            }),
        )
            .into_response();
    }

    // 12. Check if trick is complete
    let mut trick_complete = false;
    let mut game_complete = false;
    let mut trick_winner = None;
    let mut points_won = 0;

    if trick_state.current_trick.is_complete {
        // Complete the trick
        match trick_state.complete_trick() {
            Ok(result) => {
                trick_complete = true;
                trick_winner = Some(result.winner);
                points_won = result.points;
                game_complete = result.game_complete;

                // Store completed trick for history
                if let Err(e) = TrickStateRepository::store_completed_trick(
                    &mut conn,
                    &game_id,
                    &trick_state.completed_tricks.last().unwrap(),
                )
                .await
                {
                    eprintln!("Failed to store completed trick: {}", e);
                }
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to complete trick: {}", e),
                        message: None,
                    }),
                )
                    .into_response();
            }
        }
    }

    // 13. Save updated trick state
    if let Err(e) = TrickStateRepository::store_trick_state(&mut conn, &game_id, &trick_state).await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save trick state: {}", e),
                message: None,
            }),
        )
            .into_response();
    }

    // 14. Broadcast card played event via WebSocket
    let trick_event = TrickEvent::CardPlayed {
        game_id: game_id.clone(),
        player_position: player_position as u8,
        card: card_to_play.to_string(),
        trick_number: trick_state.current_trick.trick_number,
        cards_in_trick: trick_state.current_trick.cards_played.len() as u8,
        next_player: if trick_complete {
            None
        } else {
            Some(trick_state.current_trick.current_player as u8)
        },
        trick_complete,
        trick_winner: trick_winner.map(|w| w as u8),
        points_won,
    };

    if let Err(e) = broadcast_trick_event(&mut conn, &trick_event).await {
        eprintln!("Failed to broadcast card played event: {}", e);
        // Don't fail the request if broadcasting fails
    }

    // 15. If trick complete, broadcast trick completion
    if trick_complete {
        let completion_event = TrickEvent::TrickCompleted {
            game_id: game_id.clone(),
            trick_number: trick_state.completed_tricks.last().unwrap().trick_number,
            winner: trick_winner.unwrap() as u8,
            points: points_won,
            trump_team_score: trick_state.points_accumulated.0,
            opponent_team_score: trick_state.points_accumulated.1,
            game_complete,
        };

        if let Err(e) = broadcast_trick_event(&mut conn, &completion_event).await {
            eprintln!("Failed to broadcast trick completion event: {}", e);
        }
    }

    // 16. Prepare response
    let response = CardPlayResponse {
        message: if trick_complete {
            if game_complete {
                "Card played - Game complete!".to_string()
            } else {
                format!(
                    "Card played - Trick {} complete!",
                    trick_state.completed_tricks.len()
                )
            }
        } else {
            "Card played successfully".to_string()
        },
        game_id: game_id.clone(),
        card_played: card_to_play.to_string(),
        player_position: player_position as u8,
        trick_info: GameTrickInfo {
            current_trick_number: trick_state.current_trick.trick_number,
            cards_played_in_trick: trick_state.current_trick.cards_played.len() as u8,
            current_player: if trick_complete {
                None
            } else {
                Some(trick_state.current_trick.current_player as u8)
            },
            trick_complete,
            trick_winner: trick_winner.map(|w| w as u8),
            trump_team_tricks: trick_state.tricks_won.0,
            opponent_team_tricks: trick_state.tricks_won.1,
            trump_team_points: trick_state.points_accumulated.0,
            opponent_team_points: trick_state.points_accumulated.1,
            game_complete,
        },
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Get current trick information
///
/// Returns information about the current trick in progress and overall game state.
#[utoipa::path(
    get,
    path = "/game/trick",
    tag = "Game Playing",
    security(
        ("jwt_auth" = [])
    ),
    responses(
        (status = 200, description = "Trick information retrieved", body = TrickSummaryResponse),
        (status = 404, description = "Game not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    summary = "Get current trick information",
    description = "Get detailed information about the current trick and overall game progress."
)]
#[axum::debug_handler]
pub async fn get_trick_info_handler(
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

    // Get player's current game
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
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No active trick found".to_string(),
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

    let response = TrickSummaryResponse {
        game_id: game_id.clone(),
        game_info: GameTrickInfo {
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
        },
    };

    (StatusCode::OK, Json(response)).into_response()
}

// Helper function to broadcast trick events
async fn broadcast_trick_event(
    conn: &mut Connection,
    event: &TrickEvent,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let event_data = match event {
        TrickEvent::CardPlayed {
            game_id,
            player_position,
            card,
            trick_number,
            cards_in_trick,
            next_player,
            trick_complete,
            trick_winner,
            points_won,
        } => json!({
            "event": "card_played",
            "game_id": game_id,
            "player_position": player_position,
            "card": card,
            "trick_number": trick_number,
            "cards_in_trick": cards_in_trick,
            "next_player": next_player,
            "trick_complete": trick_complete,
            "trick_winner": trick_winner,
            "points_won": points_won
        }),
        TrickEvent::TrickCompleted {
            game_id,
            trick_number,
            winner,
            points,
            trump_team_score,
            opponent_team_score,
            game_complete,
        } => json!({
            "event": "trick_completed",
            "game_id": game_id,
            "trick_number": trick_number,
            "winner": winner,
            "points": points,
            "trump_team_score": trump_team_score,
            "opponent_team_score": opponent_team_score,
            "game_complete": game_complete
        }),
    };

    broadcasting::publish_event(conn, &event.game_id(), &event_data).await
}
