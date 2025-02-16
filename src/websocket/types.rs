use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct GameMessage {
    pub event: String,
    pub data: serde_json::Value,
}
