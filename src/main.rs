use redis::aio::Connection;

use crate::api::routes as api_routes;
use crate::websocket::routes as ws_routes;
use axum::Router;
use hyper::http::{header, HeaderValue, Method};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

mod api;
mod auth;
mod auth_layer;
mod websocket;

type RedisPool = Arc<Mutex<Connection>>;

#[tokio::main]
async fn main() {
    let redis_client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let redis_conn = redis_client.get_async_connection().await.unwrap();
    let redis_pool = Arc::new(Mutex::new(redis_conn));

    let app = Router::new()
        .merge(api_routes::create_router(redis_pool.clone()))
        .merge(ws_routes::create_router(redis_pool))
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
