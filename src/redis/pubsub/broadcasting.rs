use crate::redis::player::repository::PlayerRepository;
use deadpool_redis::Connection;
use serde_json::{json, Value};

/// Broadcast a bid made event to all players in the game
pub async fn broadcast_bid_made(
    redis_conn: &mut Connection,
    game_id: &str,
    bidder_position: u8,
    bid_length: u8,
    bid_suit: &str,
    current_bidder: u8,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;
    let affected_players: Vec<String> = players.iter().map(|p| p.user_id.clone()).collect();

    let event_data = json!({
        "event": "bid_made",
        "game_id": game_id,
        "bidder_position": bidder_position,
        "bid_length": bid_length,
        "bid_suit": bid_suit,
        "current_bidder": current_bidder,
        "affected_players": affected_players,
        "message": "Bid made"
    });

    publish_event(redis_conn, game_id, &event_data).await
}

/// Broadcast a pass made event to all players in the game
pub async fn broadcast_pass_made(
    redis_conn: &mut Connection,
    game_id: &str,
    passer_position: u8,
    current_bidder: u8,
    all_passed: bool,
    bidding_complete: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;
    let affected_players: Vec<String> = players.iter().map(|p| p.user_id.clone()).collect();

    let event_data = json!({
        "event": "pass_made",
        "game_id": game_id,
        "passer_position": passer_position,
        "current_bidder": current_bidder,
        "all_passed": all_passed,
        "bidding_complete": bidding_complete,
        "affected_players": affected_players,
        "message": "Pass made"
    });

    publish_event(redis_conn, game_id, &event_data).await
}

/// Broadcast a redeal event to all players in the game
pub async fn broadcast_redeal(
    redis_conn: &mut Connection,
    game_id: &str,
    dealer_position: u8,
    current_bidder: u8,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;
    let affected_players: Vec<String> = players.iter().map(|p| p.user_id.clone()).collect();

    let event_data = json!({
        "event": "redeal",
        "game_id": game_id,
        "dealer_position": dealer_position,
        "current_bidder": current_bidder,
        "affected_players": affected_players,
        "message": "Cards redealt"
    });

    publish_event(redis_conn, game_id, &event_data).await
}

/// Broadcast a bidding complete event to all players in the game
pub async fn broadcast_bidding_complete(
    redis_conn: &mut Connection,
    game_id: &str,
    trump_declarer: u8,
    trump_suit: &str,
    bid_length: u8,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;
    let affected_players: Vec<String> = players.iter().map(|p| p.user_id.clone()).collect();

    let event_data = json!({
        "event": "bidding_complete",
        "game_id": game_id,
        "trump_declarer": trump_declarer,
        "trump_suit": trump_suit,
        "bid_length": bid_length,
        "affected_players": affected_players,
        "message": "Bidding complete"
    });

    publish_event(redis_conn, game_id, &event_data).await
}

/// Broadcast a hand update to a specific player
pub async fn broadcast_hand_update(
    redis_conn: &mut Connection,
    game_id: &str,
    player_id: &str,
    hand_data: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let event_data = json!({
        "event": "hand_update",
        "game_id": game_id,
        "player_id": player_id,
        "hand_data": hand_data,
        "affected_players": [player_id],
        "message": "Hand updated"
    });

    publish_event(redis_conn, game_id, &event_data).await
}

/// Broadcast a game state update to all players in the game
pub async fn broadcast_game_state_update(
    redis_conn: &mut Connection,
    game_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let players = PlayerRepository::get_players_in_game(redis_conn, game_id).await?;
    let affected_players: Vec<String> = players.iter().map(|p| p.user_id.clone()).collect();

    let event_data = json!({
        "event": "game_state_update",
        "game_id": game_id,
        "affected_players": affected_players,
        "message": "Game state updated"
    });

    publish_event(redis_conn, game_id, &event_data).await
}

/// Helper function to publish an event to Redis PubSub
async fn publish_event(
    redis_conn: &mut Connection,
    game_id: &str,
    event_data: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let channel = format!("game:{}", game_id);
    let message = serde_json::to_string(event_data)?;

    redis::cmd("PUBLISH")
        .arg(&channel)
        .arg(&message)
        .query_async::<_, i32>(redis_conn)
        .await?;

    Ok(())
}
