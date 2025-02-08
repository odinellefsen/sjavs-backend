use crate::{state::new_state, websocket::ws_handler};
use axum::{
    http::{HeaderValue, Method},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod auth;
mod state;
mod websocket;

// Example handler for creating a match.
// Put this in your auth or some other module.
async fn create_match_handler() -> Json<serde_json::Value> {
    Json(json!({
        "message": "Match created!",
        "status": "success"
    }))
}

#[tokio::main]
async fn main() {
    let state = new_state();

    let app = Router::new()
        // WebSocket route
        .route("/ws", get(ws_handler))
        // REST route
        .route("/create-match", post(create_match_handler))
        .with_state(state.clone())
        .layer(
            CorsLayer::new()
                .allow_origin("http://192.168.1.171:5173".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([
                    axum::http::HeaderName::from_static("authorization"),
                    axum::http::HeaderName::from_static("content-type"),
                    axum::http::HeaderName::from_static("upgrade"),
                    axum::http::HeaderName::from_static("connection"),
                    axum::http::HeaderName::from_static("sec-websocket-key"),
                    axum::http::HeaderName::from_static("sec-websocket-version"),
                ])
                .allow_credentials(true),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
