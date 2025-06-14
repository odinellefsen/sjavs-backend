use super::types::GameMessage;
use crate::redis::pubsub::repository::PubSubRepository;
use crate::websocket::events::bidding::{
    handle_bid_made_event, handle_bidding_complete_event, handle_game_state_update_event,
    handle_hand_update_event, handle_pass_made_event, handle_redeal_event,
};
use crate::websocket::events::join::handle_join_event;
use crate::websocket::events::team_up_request::handle_team_up_request;
use crate::websocket::events::team_up_response::handle_team_up_response;
use crate::RedisPool;
use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Extension, State, WebSocketUpgrade},
    response::IntoResponse,
};
use dashmap::DashMap;
use futures_util::SinkExt;
use futures_util::StreamExt;
use redis;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};

// Define types for our connection registry
type UserId = String;
type GameId = String;
type MessageSender = mpsc::Sender<Message>;

// Application state to track connections
pub struct AppState {
    pub user_connections: DashMap<UserId, MessageSender>,
    pub game_players: DashMap<GameId, HashSet<UserId>>,
    pub redis_pool: RedisPool,
    pub subscribed_games: Mutex<HashSet<String>>,
    pub subscribed_players: Mutex<HashSet<String>>,
}

pub fn create_app_state(redis_pool: RedisPool) -> Arc<AppState> {
    let instance_id = PubSubRepository::generate_instance_id();

    println!("Starting server instance: {}", instance_id);

    let app_state = Arc::new(AppState {
        user_connections: DashMap::new(),
        game_players: DashMap::new(),
        redis_pool,
        subscribed_games: Mutex::new(HashSet::new()),
        subscribed_players: Mutex::new(HashSet::new()),
    });

    // Start the new PubSub listener
    start_pubsub_listener(app_state.clone());

    app_state
}

// New PubSub based event listener
fn start_pubsub_listener(app_state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            // Get a copy of our current subscriptions
            let game_ids = app_state.subscribed_games.lock().await.clone();
            let player_ids = app_state.subscribed_players.lock().await.clone();

            if game_ids.is_empty() && player_ids.is_empty() {
                // If we have no subscriptions yet, wait and try again
                sleep(Duration::from_secs(1)).await;
                continue;
            }

            // Get a new redis client for pubsub
            let client = match redis::Client::open("redis://127.0.0.1/") {
                Ok(client) => client,
                Err(e) => {
                    eprintln!("Failed to create Redis client for PubSub: {}", e);
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            // Get pubsub connection
            let connection = match client.get_async_connection().await {
                Ok(connection) => connection,
                Err(e) => {
                    eprintln!("Failed to establish Redis connection for PubSub: {}", e);
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            // Subscribe to channels and get PubSub object
            let mut pubsub =
                match PubSubRepository::subscribe_to_channels(connection, &game_ids, &player_ids)
                    .await
                {
                    Ok(pubsub) => pubsub,
                    Err(e) => {
                        eprintln!("Failed to subscribe to channels: {}", e);
                        sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

            println!(
                "PubSub subscribed to {} games and {} players",
                game_ids.len(),
                player_ids.len()
            );

            // Process received messages with dynamic re-subscription
            let app_state_clone = app_state.clone();
            let mut msg_stream = pubsub.on_message();
            let prev_game_ids = game_ids.clone();
            let prev_player_ids = player_ids.clone();
            loop {
                match timeout(Duration::from_secs(1), msg_stream.next()).await {
                    Ok(Some(msg)) => {
                        let payload: String = match msg.get_payload() {
                            Ok(p) => p,
                            Err(e) => {
                                eprintln!("Failed to get payload: {}", e);
                                continue;
                            }
                        };
                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&payload) {
                            if let Some(arr) = event["affected_players"].as_array() {
                                let players: Vec<String> = arr
                                    .iter()
                                    .filter_map(|v| v.as_str().map(str::to_string))
                                    .collect();
                                let event_type =
                                    event["event"].as_str().unwrap_or("game_update").to_string();
                                let gid = event["game_id"].as_str().unwrap_or("").to_string();
                                let msg_txt = event["message"]
                                    .as_str()
                                    .unwrap_or("Game update")
                                    .to_string();
                                let mut data = json!({"message": msg_txt, "game_id": gid});
                                if let Some(obj) = data.as_object_mut() {
                                    for (k, v) in
                                        event.as_object().unwrap_or(&serde_json::Map::new())
                                    {
                                        if !["event", "affected_players", "message", "game_id"]
                                            .contains(&k.as_str())
                                        {
                                            obj.insert(k.clone(), v.clone());
                                        }
                                    }
                                }
                                for pid in players {
                                    if let Some(tx) = app_state_clone.user_connections.get(&pid) {
                                        let gm = GameMessage {
                                            event: event_type.clone(),
                                            data: data.clone(),
                                        };
                                        let _ = tx
                                            .send(Message::Text(
                                                serde_json::to_string(&gm).unwrap_or_default(),
                                            ))
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    Ok(None) => break, // connection closed
                    Err(_) => {
                        let new_g = app_state.subscribed_games.lock().await.clone();
                        let new_p = app_state.subscribed_players.lock().await.clone();
                        if new_g != prev_game_ids || new_p != prev_player_ids {
                            break; // subscription set changed, re-subscribe
                        }
                    }
                }
            }

            // If we get here, our PubSub connection was closed or subscriptions changed
            eprintln!("PubSub connection restarting (connection closed or subscriptions changed), reconnecting in 1s");
            sleep(Duration::from_secs(1)).await;
        }
    });
}

// Register a user to receive events for a game
pub async fn subscribe_user_to_game(state: &Arc<AppState>, game_id: &str, user_id: &str) {
    // 1. Add the game to in-memory tracking for WebSocket broadcasting
    state
        .game_players
        .entry(game_id.to_string())
        .or_insert_with(HashSet::new)
        .insert(user_id.to_string());

    // 2. Update our Redis PubSub subscriptions
    {
        let mut subscribed_games = state.subscribed_games.lock().await;
        subscribed_games.insert(game_id.to_string());
    }

    {
        let mut subscribed_players = state.subscribed_players.lock().await;
        subscribed_players.insert(user_id.to_string());
    }
}

#[axum::debug_handler]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(user_id): Extension<String>,
    State(app_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, app_state))
}

pub async fn handle_socket(socket: WebSocket, user_id: String, state: Arc<AppState>) {
    // Create independent clones for each use
    let forward_user_id = user_id.clone();
    let cleanup_user_id = user_id.clone();

    let (mut sender, mut receiver) = socket.split();

    // Create a channel for sending messages to this client
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    // Store the sender in our connection registry
    state.user_connections.insert(forward_user_id.clone(), tx);

    // Subscribe to events for this player
    {
        let mut subscribed_players = state.subscribed_players.lock().await;
        subscribed_players.insert(user_id.clone());
    }

    // Task to forward messages from the channel to the WebSocket
    let forward_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let Err(e) = sender.send(message).await {
                eprintln!("Failed to send message to user {}: {}", forward_user_id, e);
                break;
            }
        }
    });

    // Get Redis connection
    let mut redis_conn = match state.redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Failed to get Redis connection: {}", e);
            return;
        }
    };

    // Process incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        // Clone directly from original user_id
        let handler_user_id = user_id.clone();

        if let Message::Text(text) = msg {
            if let Ok(game_msg) = serde_json::from_str::<GameMessage>(&text) {
                match game_msg.event.as_str() {
                    "join" => {
                        if let Err(e) = handle_join_event(
                            &state,
                            &handler_user_id,
                            &game_msg.data,
                            &mut redis_conn,
                        )
                        .await
                        {
                            eprintln!("Join event error: {}", e);
                        }
                    }
                    "team_up_request" => {
                        if let Err(e) = handle_team_up_request(
                            &state,
                            &handler_user_id,
                            &game_msg.data,
                            &mut redis_conn,
                        )
                        .await
                        {
                            eprintln!("Team up request error: {}", e);
                        }
                    }
                    "team_up_response" => {
                        if let Err(e) = handle_team_up_response(
                            &state,
                            &handler_user_id,
                            &game_msg.data,
                            &mut redis_conn,
                        )
                        .await
                        {
                            eprintln!("Team up response error: {}", e);
                        }
                    }
                    "bid_made" => {
                        if let Err(e) =
                            handle_bid_made_event(&state, &game_msg.data, &mut redis_conn).await
                        {
                            eprintln!("Bid made event error: {}", e);
                        }
                    }
                    "pass_made" => {
                        if let Err(e) =
                            handle_pass_made_event(&state, &game_msg.data, &mut redis_conn).await
                        {
                            eprintln!("Pass made event error: {}", e);
                        }
                    }
                    "redeal" => {
                        if let Err(e) =
                            handle_redeal_event(&state, &game_msg.data, &mut redis_conn).await
                        {
                            eprintln!("Redeal event error: {}", e);
                        }
                    }
                    "bidding_complete" => {
                        if let Err(e) =
                            handle_bidding_complete_event(&state, &game_msg.data, &mut redis_conn)
                                .await
                        {
                            eprintln!("Bidding complete event error: {}", e);
                        }
                    }
                    "hand_update" => {
                        if let Err(e) =
                            handle_hand_update_event(&state, &game_msg.data, &mut redis_conn).await
                        {
                            eprintln!("Hand update event error: {}", e);
                        }
                    }
                    "game_state_update" => {
                        if let Err(e) =
                            handle_game_state_update_event(&state, &game_msg.data, &mut redis_conn)
                                .await
                        {
                            eprintln!("Game state update event error: {}", e);
                        }
                    }
                    _ => {
                        // Handle unknown event types
                        println!("Received unknown event type: {}", game_msg.event);
                    }
                }
            }
        }
    }

    // WebSocket closed, clean up
    println!("WebSocket connection closed for user {}", cleanup_user_id);

    // Remove user from connection registry
    state.user_connections.remove(&cleanup_user_id);

    // Remove user from subscribed players
    {
        let mut subscribed_players = state.subscribed_players.lock().await;
        subscribed_players.remove(&cleanup_user_id);
    }

    // Remove user from any games they were in and check if game subscriptions can be cleaned up
    let mut games_to_remove = Vec::new();

    for mut game_entry in state.game_players.iter_mut() {
        let game_id = game_entry.key().clone();
        let players = game_entry.value_mut();

        // Remove this user from the game
        players.retain(|id| id != &cleanup_user_id);

        // If no more players in this game on this instance, mark for removal from subscriptions
        if players.is_empty() {
            games_to_remove.push(game_id);
        }
    }

    // Clean up game subscriptions that are no longer needed
    if !games_to_remove.is_empty() {
        let mut subscribed_games = state.subscribed_games.lock().await;
        for game_id in games_to_remove {
            subscribed_games.remove(&game_id);
            // Also remove the empty entry from game_players
            state.game_players.remove(&game_id);
        }
    }

    // Abort the forward task
    forward_task.abort();
}
