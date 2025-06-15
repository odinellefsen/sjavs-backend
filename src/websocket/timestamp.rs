use chrono::{DateTime, Utc};

pub struct TimestampManager;

impl TimestampManager {
    /// Generate current timestamp in milliseconds
    pub fn now() -> i64 {
        Utc::now().timestamp_millis()
    }

    /// Generate timestamp for initial state snapshot
    /// Adds small buffer to ensure it's newer than any concurrent events
    pub fn snapshot_timestamp() -> i64 {
        Self::now() + 1 // 1ms buffer
    }

    /// Check if timestamp is newer than a reference
    pub fn is_newer(timestamp: i64, reference: i64) -> bool {
        timestamp > reference
    }

    /// Format timestamp for logging
    pub fn format(timestamp: i64) -> String {
        DateTime::from_timestamp_millis(timestamp)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
            .unwrap_or_else(|| "Invalid timestamp".to_string())
    }
}

/// Trait for timestamped events
pub trait Timestamped {
    fn timestamp(&self) -> i64;
    fn set_timestamp(&mut self, timestamp: i64);

    fn is_newer_than(&self, other: &dyn Timestamped) -> bool {
        self.timestamp() > other.timestamp()
    }
}

impl Timestamped for crate::websocket::types::GameMessage {
    fn timestamp(&self) -> i64 {
        self.timestamp
    }

    fn set_timestamp(&mut self, timestamp: i64) {
        self.timestamp = timestamp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_timestamp_generation() {
        let ts1 = TimestampManager::now();
        std::thread::sleep(Duration::from_millis(2));
        let ts2 = TimestampManager::now();

        assert!(ts2 > ts1);
    }

    #[test]
    fn test_snapshot_timestamp_ordering() {
        let now = TimestampManager::now();
        let snapshot = TimestampManager::snapshot_timestamp();

        assert!(snapshot > now);
    }

    #[test]
    fn test_timestamp_comparison() {
        let ts1 = TimestampManager::now();
        let ts2 = ts1 + 1000;

        assert!(TimestampManager::is_newer(ts2, ts1));
        assert!(!TimestampManager::is_newer(ts1, ts2));
    }
}
