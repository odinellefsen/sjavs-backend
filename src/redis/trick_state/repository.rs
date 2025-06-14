use crate::game::trick::{GameTrickState, TrickState};
use deadpool_redis::Connection;
use serde_json;

pub struct TrickStateRepository;

impl TrickStateRepository {
    /// Store current trick state
    pub async fn store_trick_state(
        conn: &mut Connection,
        game_id: &str,
        trick_state: &GameTrickState,
    ) -> Result<(), String> {
        let key = format!("game_trick_state:{}", game_id);
        let serialized = serde_json::to_string(trick_state)
            .map_err(|e| format!("Failed to serialize trick state: {}", e))?;

        redis::cmd("SET")
            .arg(&key)
            .arg(&serialized)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to store trick state: {}", e))?;

        Ok(())
    }

    /// Get current trick state
    pub async fn get_trick_state(
        conn: &mut Connection,
        game_id: &str,
    ) -> Result<Option<GameTrickState>, String> {
        let key = format!("game_trick_state:{}", game_id);

        let serialized: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Failed to retrieve trick state: {}", e))?;

        match serialized {
            Some(data) => {
                let trick_state = serde_json::from_str(&data)
                    .map_err(|e| format!("Failed to deserialize trick state: {}", e))?;
                Ok(Some(trick_state))
            }
            None => Ok(None),
        }
    }

    /// Clear trick state (for game completion)
    pub async fn clear_trick_state(conn: &mut Connection, game_id: &str) -> Result<(), String> {
        let key = format!("game_trick_state:{}", game_id);

        redis::cmd("DEL")
            .arg(&key)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to clear trick state: {}", e))?;

        Ok(())
    }

    /// Store completed trick for history
    pub async fn store_completed_trick(
        conn: &mut Connection,
        game_id: &str,
        trick: &TrickState,
    ) -> Result<(), String> {
        let key = format!("game_trick_history:{}:{}", game_id, trick.trick_number);
        let serialized = serde_json::to_string(trick)
            .map_err(|e| format!("Failed to serialize trick: {}", e))?;

        redis::cmd("SET")
            .arg(&key)
            .arg(&serialized)
            .arg("EX")
            .arg(3600) // Expire after 1 hour
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to store completed trick: {}", e))?;

        Ok(())
    }

    /// Get trick history for a game
    pub async fn get_trick_history(
        conn: &mut Connection,
        game_id: &str,
    ) -> Result<Vec<TrickState>, String> {
        let mut tricks = Vec::new();

        for trick_number in 1..=8 {
            let key = format!("game_trick_history:{}:{}", game_id, trick_number);

            let serialized: Option<String> = redis::cmd("GET")
                .arg(&key)
                .query_async(&mut *conn)
                .await
                .map_err(|e| format!("Failed to retrieve trick history: {}", e))?;

            if let Some(data) = serialized {
                let trick: TrickState = serde_json::from_str(&data)
                    .map_err(|e| format!("Failed to deserialize trick: {}", e))?;
                tricks.push(trick);
            } else {
                break; // No more tricks
            }
        }

        Ok(tricks)
    }

    /// Check if trick state exists for a game
    pub async fn trick_state_exists(conn: &mut Connection, game_id: &str) -> Result<bool, String> {
        let key = format!("game_trick_state:{}", game_id);

        let exists: bool = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Failed to check trick state existence: {}", e))?;

        Ok(exists)
    }

    /// Set expiration for trick state (cleanup after game completion)
    pub async fn expire_trick_state(
        conn: &mut Connection,
        game_id: &str,
        ttl_seconds: u64,
    ) -> Result<(), String> {
        let key = format!("game_trick_state:{}", game_id);

        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(ttl_seconds)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to set expiration: {}", e))?;

        Ok(())
    }
}
