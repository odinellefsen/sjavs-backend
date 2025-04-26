use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Status of a normal match
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalMatchStatus {
    Waiting,
    InProgress,
    Completed,
    Cancelled,
}

impl ToString for NormalMatchStatus {
    fn to_string(&self) -> String {
        match self {
            NormalMatchStatus::Waiting => "waiting".to_string(),
            NormalMatchStatus::InProgress => "in_progress".to_string(),
            NormalMatchStatus::Completed => "completed".to_string(),
            NormalMatchStatus::Cancelled => "cancelled".to_string(),
        }
    }
}

impl From<&str> for NormalMatchStatus {
    fn from(s: &str) -> Self {
        match s {
            "waiting" => NormalMatchStatus::Waiting,
            "in_progress" => NormalMatchStatus::InProgress,
            "completed" => NormalMatchStatus::Completed,
            "cancelled" => NormalMatchStatus::Cancelled,
            _ => NormalMatchStatus::Waiting, // Default to waiting for unknown values
        }
    }
}

/// Represents a normal match stored in Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalMatch {
    pub id: String,
    pub pin: u32,
    pub status: NormalMatchStatus,
    pub number_of_crosses: u32,
    pub current_cross: u32,
    pub created_timestamp: u64,
}

impl NormalMatch {
    /// Create a new normal match with default values
    pub fn new(id: String, pin: u32, number_of_crosses: u32) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

        Self {
            id,
            pin,
            status: NormalMatchStatus::Waiting,
            number_of_crosses,
            current_cross: 0,
            created_timestamp: now,
        }
    }

    /// Get the Redis key for this match
    pub fn redis_key(&self) -> String {
        format!("normal_match:{}", self.id)
    }

    /// Convert from Redis hash map to NormalMatch
    pub fn from_redis_hash(id: String, hash: &HashMap<String, String>) -> Result<Self, String> {
        let pin = hash
            .get("pin")
            .ok_or("Missing pin field")?
            .parse::<u32>()
            .map_err(|_| "Invalid pin format")?;

        let status = hash
            .get("status")
            .map(|s| NormalMatchStatus::from(s.as_str()))
            .unwrap_or(NormalMatchStatus::Waiting);

        let number_of_crosses = hash
            .get("number_of_crosses")
            .ok_or("Missing number_of_crosses field")?
            .parse::<u32>()
            .map_err(|_| "Invalid number_of_crosses format")?;

        let current_cross = hash
            .get("current_cross")
            .ok_or("Missing current_cross field")?
            .parse::<u32>()
            .map_err(|_| "Invalid current_cross format")?;

        let created_timestamp = hash
            .get("created_timestamp")
            .ok_or("Missing created_timestamp field")?
            .parse::<u64>()
            .map_err(|_| "Invalid created_timestamp format")?;

        Ok(Self {
            id,
            pin,
            status,
            number_of_crosses,
            current_cross,
            created_timestamp,
        })
    }

    /// Convert NormalMatch to Redis hash map
    pub fn to_redis_hash(&self) -> HashMap<String, String> {
        let mut hash = HashMap::new();
        hash.insert("id".to_string(), self.id.clone());
        hash.insert("pin".to_string(), self.pin.to_string());
        hash.insert("status".to_string(), self.status.to_string());
        hash.insert(
            "number_of_crosses".to_string(),
            self.number_of_crosses.to_string(),
        );
        hash.insert("current_cross".to_string(), self.current_cross.to_string());
        hash.insert(
            "created_timestamp".to_string(),
            self.created_timestamp.to_string(),
        );
        hash
    }
}
