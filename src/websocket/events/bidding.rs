use crate::redis::normal_match::repository::NormalMatchRepository;
use crate::redis::player::repository::PlayerRepository;
use crate::websocket::handler::AppState;
use crate::websocket::types::GameMessage;
use deadpool_redis::Connection;
use serde_json::{json, Value};
use std::sync::Arc;

/// Handle bid made event - broadcasts when a player makes a bid
pub async fn handle_bid_made_event(
    state: &Arc<AppState>,
    data: &Value,
    redis_conn: &mut Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let game_id = data
        .get("game_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing game_id")?;

    let bidder_position = data
        .get("bidder_position")
        .and_then(|v| v.as_u64())
        .ok_or("Missing bidder_position")? as u8;

    let bid_length = data
        .get("bid_length")
        .and_then(|v| v.as_u64())
        .ok_or("Missing bid_length")? as u8;

    let bid_suit = data
        .get("bid_suit")
        .and_then(|v| v.as_str())
        .ok_or("Missing bid_suit")?;

    let current_bidder = data
        .get("current_bidder")
        .and_then(|v| v.as_u64())
        .ok_or("Missing current_bidder")? as u8;

    // Get player information
    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;

    if bidder_position as usize >= players.len() {
        return Err("Invalid bidder position".into());
    }

    let bidder_username =
        get_player_username(redis_conn, &players[bidder_position as usize].user_id).await?;
    let next_bidder_username =
        get_player_username(redis_conn, &players[current_bidder as usize].user_id).await?;

    // Create bid event message
    let bid_event = GameMessage::new(
        "bid_made".to_string(),
        json!({
            "game_id": game_id,
            "bidder_position": bidder_position,
            "bidder_username": bidder_username,
            "bid": {
                "length": bid_length,
                "suit": bid_suit,
                "is_club_declaration": bid_suit == "clubs",
                "display_text": format!("{} trumps ({})", bid_length, bid_suit)
            },
            "current_bidder": current_bidder,
            "current_bidder_username": next_bidder_username,
            "message": format!("{} bid {} {} trumps", bidder_username, bid_length, bid_suit)
        }),
    )
    .with_game_id(game_id.to_string())
    .with_phase("bidding".to_string());

    // Broadcast to all players in the game
    broadcast_to_game_players(state, game_id, &players, &bid_event).await?;

    Ok(())
}

/// Handle pass made event - broadcasts when a player passes
pub async fn handle_pass_made_event(
    state: &Arc<AppState>,
    data: &Value,
    redis_conn: &mut Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let game_id = data
        .get("game_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing game_id")?;

    let passer_position = data
        .get("passer_position")
        .and_then(|v| v.as_u64())
        .ok_or("Missing passer_position")? as u8;

    let current_bidder = data
        .get("current_bidder")
        .and_then(|v| v.as_u64())
        .ok_or("Missing current_bidder")? as u8;

    let all_passed = data
        .get("all_passed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let bidding_complete = data
        .get("bidding_complete")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Get player information
    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;

    if passer_position as usize >= players.len() {
        return Err("Invalid passer position".into());
    }

    let passer_username =
        get_player_username(redis_conn, &players[passer_position as usize].user_id).await?;

    let message = if all_passed {
        format!(
            "{} passed - All players passed, redealing cards...",
            passer_username
        )
    } else if bidding_complete {
        format!("{} passed - Bidding complete!", passer_username)
    } else {
        let next_bidder_username =
            get_player_username(redis_conn, &players[current_bidder as usize].user_id).await?;
        format!(
            "{} passed - {} to bid",
            passer_username, next_bidder_username
        )
    };

    // Create pass event message
    let pass_event = GameMessage::new(
        "pass_made".to_string(),
        json!({
            "game_id": game_id,
            "passer_position": passer_position,
            "passer_username": passer_username,
            "current_bidder": current_bidder,
            "all_passed": all_passed,
            "bidding_complete": bidding_complete,
            "message": message
        }),
    )
    .with_game_id(game_id.to_string())
    .with_phase("bidding".to_string());

    // Broadcast to all players in the game
    broadcast_to_game_players(state, game_id, &players, &pass_event).await?;

    Ok(())
}

/// Handle redeal event - broadcasts when cards are redealt due to all players passing
pub async fn handle_redeal_event(
    state: &Arc<AppState>,
    data: &Value,
    redis_conn: &mut Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let game_id = data
        .get("game_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing game_id")?;

    let dealer_position = data
        .get("dealer_position")
        .and_then(|v| v.as_u64())
        .ok_or("Missing dealer_position")? as u8;

    let current_bidder = data
        .get("current_bidder")
        .and_then(|v| v.as_u64())
        .ok_or("Missing current_bidder")? as u8;

    // Get player information
    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;

    let dealer_username =
        get_player_username(redis_conn, &players[dealer_position as usize].user_id).await?;
    let first_bidder_username =
        get_player_username(redis_conn, &players[current_bidder as usize].user_id).await?;

    // Create redeal event message
    let redeal_event = GameMessage::new(
        "cards_redealt".to_string(),
        json!({
            "game_id": game_id,
            "dealer_position": dealer_position,
            "dealer_username": dealer_username,
            "current_bidder": current_bidder,
            "current_bidder_username": first_bidder_username,
            "message": format!("Cards redealt! {} dealing, {} to bid first", dealer_username, first_bidder_username)
        })
    ).with_game_id(game_id.to_string()).with_phase("bidding".to_string());

    // Broadcast to all players in the game
    broadcast_to_game_players(state, game_id, &players, &redeal_event).await?;

    Ok(())
}

/// Handle bidding complete event - broadcasts when bidding is finished and trump is declared
pub async fn handle_bidding_complete_event(
    state: &Arc<AppState>,
    data: &Value,
    redis_conn: &mut Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let game_id = data
        .get("game_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing game_id")?;

    let trump_declarer = data
        .get("trump_declarer")
        .and_then(|v| v.as_u64())
        .ok_or("Missing trump_declarer")? as u8;

    let trump_suit = data
        .get("trump_suit")
        .and_then(|v| v.as_str())
        .ok_or("Missing trump_suit")?;

    let bid_length = data
        .get("bid_length")
        .and_then(|v| v.as_u64())
        .ok_or("Missing bid_length")? as u8;

    // Get player information
    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;

    let trump_declarer_username =
        get_player_username(redis_conn, &players[trump_declarer as usize].user_id).await?;

    // Determine partnership (trump declarer + partner with highest trump)
    // For now, use opposite player as partner (will be enhanced in trick-taking phase)
    let partner_position = (trump_declarer + 2) % 4;
    let partner_username =
        get_player_username(redis_conn, &players[partner_position as usize].user_id).await?;

    let opponents = vec![
        (
            (trump_declarer + 1) % 4,
            get_player_username(
                redis_conn,
                &players[((trump_declarer + 1) % 4) as usize].user_id,
            )
            .await?,
        ),
        (
            (trump_declarer + 3) % 4,
            get_player_username(
                redis_conn,
                &players[((trump_declarer + 3) % 4) as usize].user_id,
            )
            .await?,
        ),
    ];

    // Create bidding complete event message
    let bidding_complete_event = GameMessage::new(
        "bidding_complete".to_string(),
        json!({
            "game_id": game_id,
            "trump_declarer": trump_declarer,
            "trump_declarer_username": trump_declarer_username,
            "trump_suit": trump_suit,
            "bid_length": bid_length,
            "partnership": {
                "trump_declarer": trump_declarer,
                "trump_declarer_username": trump_declarer_username,
                "partner": partner_position,
                "partner_username": partner_username,
                "opponents": opponents.iter().map(|(pos, name)| json!({
                    "position": pos,
                    "username": name
                })).collect::<Vec<_>>()
            },
            "message": format!(
                "🎉 {} wins bidding with {} {} trumps! Partnership: {} & {} vs {} & {}",
                trump_declarer_username,
                bid_length,
                trump_suit,
                trump_declarer_username,
                partner_username,
                opponents[0].1,
                opponents[1].1
            )
        }),
    )
    .with_game_id(game_id.to_string())
    .with_phase("bidding".to_string());

    // Broadcast to all players in the game
    broadcast_to_game_players(state, game_id, &players, &bidding_complete_event).await?;

    Ok(())
}

/// Handle hand update event - sends updated hand information to a specific player
pub async fn handle_hand_update_event(
    state: &Arc<AppState>,
    data: &Value,
    _redis_conn: &mut Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let game_id = data
        .get("game_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing game_id")?;

    let player_id = data
        .get("player_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing player_id")?;

    let hand_data = data.get("hand_data").ok_or("Missing hand_data")?;

    // Create hand update event message (sent only to specific player)
    let hand_update_event = GameMessage::new(
        "hand_updated".to_string(),
        json!({
            "game_id": game_id,
            "hand": hand_data,
            "message": "Your hand has been updated"
        }),
    )
    .with_game_id(game_id.to_string())
    .with_phase("bidding".to_string());

    // Send only to the specific player
    if let Some(tx) = state.user_connections.get(player_id) {
        let msg = serde_json::to_string(&hand_update_event)?;
        let _ = tx.send(axum::extract::ws::Message::Text(msg)).await;
    }

    Ok(())
}

/// Handle game state update event - broadcasts general game state changes
pub async fn handle_game_state_update_event(
    state: &Arc<AppState>,
    data: &Value,
    redis_conn: &mut Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let game_id = data
        .get("game_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing game_id")?;

    // Get current game state
    let game_match = NormalMatchRepository::get_by_id(redis_conn, game_id)
        .await?
        .ok_or("Game not found")?;

    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;

    // Create game state update event
    let game_state_event = GameMessage::new(
        "game_state_updated".to_string(),
        json!({
            "game_id": game_id,
            "status": game_match.status.to_string(),
            "dealer_position": game_match.dealer_position,
            "current_bidder": game_match.current_bidder,
            "trump_suit": game_match.trump_suit,
            "trump_declarer": game_match.trump_declarer,
            "highest_bid_length": game_match.highest_bid_length,
            "highest_bidder": game_match.highest_bidder,
            "highest_bid_suit": game_match.highest_bid_suit,
            "message": "Game state updated"
        }),
    )
    .with_game_id(game_id.to_string())
    .with_phase("bidding".to_string());

    // Broadcast to all players in the game
    broadcast_to_game_players(state, game_id, &players, &game_state_event).await?;

    Ok(())
}

/// Helper function to get player username
async fn get_player_username(
    redis_conn: &mut Connection,
    user_id: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let username: String = redis::cmd("HGET")
        .arg("usernames")
        .arg(user_id)
        .query_async(redis_conn)
        .await
        .unwrap_or_else(|_| "Unknown Player".to_string());

    Ok(username)
}

/// Helper function to broadcast a message to all players in a game
async fn broadcast_to_game_players(
    state: &Arc<AppState>,
    game_id: &str,
    players: &[crate::redis::player::repository::PlayerGameInfo],
    message: &GameMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let msg_text = serde_json::to_string(message)?;

    for player in players {
        if let Some(tx) = state.user_connections.get(&player.user_id) {
            let _ = tx
                .send(axum::extract::ws::Message::Text(msg_text.clone()))
                .await;
        }
    }

    Ok(())
}
