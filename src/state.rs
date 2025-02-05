use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type PlayerId = String;
pub type GameId = String;

#[derive(Default)]
pub struct AppState {
    pub games: DashMap<GameId, String>, // Example, replace with actual game struct
}

pub fn new_state() -> Arc<RwLock<AppState>> {
    Arc::new(RwLock::new(AppState::default()))
}
