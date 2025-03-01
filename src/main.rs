use deadpool_redis::{Config, Pool, Runtime};

use crate::api::routes as api_routes;
use crate::websocket::routes as ws_routes;
use axum::Router;
use hyper::http::{header, HeaderValue, Method};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod api;
mod auth;
mod auth_layer;
mod websocket;

// Update the RedisPool type to use deadpool
type RedisPool = Pool;

#[tokio::main]
async fn main() {
    // Configure and create the Redis connection pool
    let cfg = Config::from_url("redis://127.0.0.1/");
    let pool = cfg
        .create_pool(Some(Runtime::Tokio1))
        .expect("Failed to create Redis connection pool");

    // Test the connection to make sure the pool is working
    {
        let mut conn = pool
            .get()
            .await
            .expect("Failed to get Redis connection from pool");
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .expect("Failed to execute PING command");
        println!("Redis responded with: {}", pong);
    }

    println!("Successfully connected to Redis");

    let app = Router::new()
        .merge(api_routes::create_router(pool.clone()))
        .merge(ws_routes::create_router(pool))
        .layer(auth_layer::AuthLayer)
        .layer(
            CorsLayer::new()
                .allow_origin("http://192.168.1.176:5173".parse::<HeaderValue>().unwrap())
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
