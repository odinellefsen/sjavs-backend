use axum::{Router, routing::get};
use std::net::SocketAddr;
use tower_http::cors::{CorsLayer, Any};
use crate::{websocket::ws_handler, state::new_state};

mod websocket;
mod state;

#[tokio::main]
async fn main() {
    let state = new_state();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state.clone())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("WebSocket server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
