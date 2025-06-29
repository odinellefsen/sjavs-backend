use deadpool_redis::{Config, Pool, Runtime};

use crate::api::routes as api_routes;
use crate::websocket::handler::create_app_state;
use crate::websocket::routes as ws_routes;
use axum::Router;
use hyper::http::{header, HeaderValue, Method};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod api;
mod auth;
mod auth_layer;
mod game;
mod redis;
mod websocket;

// Update the RedisPool type to use deadpool
type RedisPool = Pool;

#[tokio::main]
async fn main() {
    // Configure and create the Redis connection pool with custom settings
    let mut cfg = Config::from_url("redis://127.0.0.1/");

    // Set pool configuration
    cfg.pool = Some(deadpool_redis::PoolConfig::new(30)); // max pool size 30

    let pool = cfg
        .create_pool(Some(Runtime::Tokio1))
        .expect("Failed to create Redis connection pool");

    println!("Successfully connected to Redis with pool size: 30");

    // Create the shared app state
    let app_state = create_app_state(pool.clone());

    let app = Router::new()
        // Public routes (no authentication required)
        .merge(api_routes::create_public_router(pool.clone()))
        // Protected routes (authentication required)
        .merge(api_routes::create_protected_router(pool.clone()).layer(auth_layer::AuthLayer))
        // WebSocket routes (authentication required)
        .merge(ws_routes::create_router(app_state).layer(auth_layer::AuthLayer))
        .layer(
            CorsLayer::new()
                .allow_origin("http://192.168.1.198:5173".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
                .allow_credentials(true),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Server listening on {addr}");
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
