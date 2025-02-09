use axum::{
    extract::Query,
    http::{HeaderValue, Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod auth; // Your Clerk logic is inside here
mod state; // Where `new_state()` is
mod websocket; // Your websocket handler

#[derive(Deserialize)]
pub struct CreateMatchQuery {
    token: String,
}

// Bring in your verify function from `auth.rs`
use crate::auth::verify_clerk_token;

// Example: unify the match arms by returning a `Result`
//   - Ok(...) -> 200 with JSON body
//   - Err(...) -> e.g. 401 with JSON body
async fn create_match_handler(
    Query(query): Query<CreateMatchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match verify_clerk_token(&query.token).await {
        Ok(claims) => {
            // SUCCESS path:
            //   return Ok(...) => 200 OK with JSON
            //   e.g. use claims.sub as the user ID
            Ok(Json(json!({
                "message": "Match created!",
                "status": "success",
                "user_id": claims.sub,
            })))
        }
        Err(e) => {
            // ERROR path:
            //   return Err(...) => automatically 4xx/5xx
            eprintln!("Error verifying token: {:?}", e);
            Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "message": "Invalid token",
                    "status": "error",
                })),
            ))
        }
    }
}

#[tokio::main]
async fn main() {
    let state = state::new_state(); // or however you init your state

    let app = Router::new()
        .route("/ws", get(websocket::ws_handler))
        .route("/create-match", post(create_match_handler))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin("http://192.168.178.88:5173".parse::<HeaderValue>().unwrap())
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
