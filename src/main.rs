use crate::{state::new_state, websocket::ws_handler};
use axum::http::HeaderValue;
use axum::http::Method;
use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod auth;
mod state;
mod websocket;

#[tokio::main]
async fn main() {
    let state = new_state();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state.clone())
        .layer(
            CorsLayer::new()
                .allow_origin("http://localhost:5173".parse::<HeaderValue>().unwrap())
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

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("WebSocket server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
