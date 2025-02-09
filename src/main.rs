use axum::{
    extract::Extension,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use hyper::http::{header, HeaderValue, Method};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod auth;
mod auth_layer;
mod state;
mod websocket;

async fn create_match_handler(Extension(user_id): Extension<String>) -> impl IntoResponse {
    Json(serde_json::json!({
        "message": format!("Welcome, user {user_id}. Match created!")
    }))
}

#[tokio::main]
async fn main() {
    let state = state::new_state();

    // Build your router
    let app = Router::new()
        .route("/ws", get(websocket::ws_handler))
        .route("/create-match", post(create_match_handler))
        .with_state(state)
        // 1) Attach AuthLayer inside
        .layer(auth_layer::AuthLayer)
        // 2) Attach CorsLayer last, so it's the outer layer
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
