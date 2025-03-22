use super::handler::{ws_handler, AppState};
use axum::{routing::get, Router};
use std::sync::Arc;

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(app_state)
}
