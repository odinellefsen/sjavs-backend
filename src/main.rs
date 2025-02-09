use redis::aio::Connection;

use crate::state::AppState;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::{
    extract::{Extension, State},
    routing::{get, post},
    Json, Router,
};
use chrono;
use hyper::http::{header, HeaderValue, Method};
use rand;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

mod auth;
mod auth_layer;
mod state;
mod websocket;

type RedisPool = Arc<Mutex<Connection>>;

async fn create_match_handler(
    Extension(user_id): Extension<String>,
    State((redis_pool, _)): State<(RedisPool, Arc<RwLock<AppState>>)>,
) -> Response {
    let mut conn = redis_pool.lock().await;

    // Check if player is already in a game
    let player_game: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(&user_id)
        .query_async::<_, Option<String>>(&mut *conn)
        .await
        .unwrap_or(None);

    if let Some(game_id) = player_game {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "Already in game",
                "message": "You are already in an active game. Please leave or finish your current game before creating a new one.",
                "game_id": game_id
            })),
        )
            .into_response();
    }

    // Generate new game ID (timestamp + random suffix)
    let game_id = format!(
        "game_{}_{:x}",
        chrono::Utc::now().timestamp(),
        rand::random::<u16>()
    );

    // Create game entry with initial state
    let game_state = json!({
        "game_id": game_id,
        "host": user_id,
        "players": [user_id],
        "status": "waiting",
        "created_at": chrono::Utc::now().to_rfc3339(),
    });

    // Store game state
    match redis::cmd("HSET")
        .arg("games")
        .arg(&game_id)
        .arg(game_state.to_string())
        .query_async::<_, ()>(&mut *conn)
        .await
    {
        Ok(_) => (),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create game: {}", e)})),
            )
                .into_response();
        }
    }

    // Associate player with game
    match redis::cmd("HSET")
        .arg("player_games")
        .arg(&user_id)
        .arg(&game_id)
        .query_async::<_, ()>(&mut *conn)
        .await
    {
        Ok(_) => (),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to associate player: {}", e)})),
            )
                .into_response();
        }
    }

    // After creating the game, verify it exists
    let stored_game: Option<String> = redis::cmd("HGET")
        .arg("games")
        .arg(&game_id)
        .query_async(&mut *conn)
        .await
        .unwrap_or(None);

    let stored_player_game: Option<String> = redis::cmd("HGET")
        .arg("player_games")
        .arg(&user_id)
        .query_async(&mut *conn)
        .await
        .unwrap_or(None);

    match (stored_game, stored_player_game) {
        (Some(game), Some(player_game)) if player_game == game_id => (
            StatusCode::CREATED,
            Json(json!({
                "message": "Game created and verified",
                "game_id": game_id,
                "state": serde_json::from_str::<serde_json::Value>(&game).unwrap()
            })),
        )
            .into_response(),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Game creation verification failed"
            })),
        )
            .into_response(),
    }
}

#[tokio::main]
async fn main() {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();

    // an async connection
    let conn = client.get_async_connection().await.unwrap();

    // wrapped connection in Arc<Mutex<Connection>>
    let redis_pool = Arc::new(Mutex::new(conn));

    let state = state::new_state();

    let app = Router::new()
        .route("/ws", get(websocket::ws_handler))
        .route("/create-match", post(create_match_handler))
        .with_state((redis_pool, state))
        .layer(auth_layer::AuthLayer)
        .layer(
            CorsLayer::new()
                .allow_origin("http://192.168.178.88:5173".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
                .allow_credentials(true),
        );

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Server listening on {addr}");
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
