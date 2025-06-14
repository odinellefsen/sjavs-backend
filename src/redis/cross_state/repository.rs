use crate::game::cross::CrossState;
use deadpool_redis::Connection;
use serde_json;

pub struct CrossStateRepository;

impl CrossStateRepository {
    /// Store cross state for a match
    pub async fn store_cross_state(
        conn: &mut Connection,
        match_id: &str,
        cross_state: &CrossState,
    ) -> Result<(), String> {
        let key = format!("cross_state:{}", match_id);
        let serialized = serde_json::to_string(cross_state)
            .map_err(|e| format!("Failed to serialize cross state: {}", e))?;

        redis::cmd("SET")
            .arg(&key)
            .arg(&serialized)
            .query_async::<_, ()>(conn)
            .await
            .map_err(|e| format!("Failed to store cross state: {}", e))?;

        Ok(())
    }

    /// Get cross state for a match
    pub async fn get_cross_state(
        conn: &mut Connection,
        match_id: &str,
    ) -> Result<Option<CrossState>, String> {
        let key = format!("cross_state:{}", match_id);

        match redis::cmd("GET")
            .arg(&key)
            .query_async::<_, Option<String>>(conn)
            .await
        {
            Ok(Some(serialized)) => {
                let cross_state = serde_json::from_str(&serialized)
                    .map_err(|e| format!("Failed to deserialize cross state: {}", e))?;
                Ok(Some(cross_state))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(format!("Failed to get cross state: {}", e)),
        }
    }

    /// Initialize cross state for a new match
    pub async fn initialize_cross_state(
        conn: &mut Connection,
        match_id: &str,
    ) -> Result<CrossState, String> {
        let cross_state = CrossState::new(match_id.to_string());
        Self::store_cross_state(conn, match_id, &cross_state).await?;
        Ok(cross_state)
    }

    /// Clear cross state (for match completion)
    pub async fn clear_cross_state(conn: &mut Connection, match_id: &str) -> Result<(), String> {
        let key = format!("cross_state:{}", match_id);

        redis::cmd("DEL")
            .arg(&key)
            .query_async::<_, i32>(conn)
            .await
            .map_err(|e| format!("Failed to clear cross state: {}", e))?;

        Ok(())
    }

    /// Get or create cross state
    pub async fn get_or_create_cross_state(
        conn: &mut Connection,
        match_id: &str,
    ) -> Result<CrossState, String> {
        match Self::get_cross_state(conn, match_id).await? {
            Some(state) => Ok(state),
            None => Self::initialize_cross_state(conn, match_id).await,
        }
    }

    /// Check if cross state exists for a match
    pub async fn cross_state_exists(conn: &mut Connection, match_id: &str) -> Result<bool, String> {
        let key = format!("cross_state:{}", match_id);

        match redis::cmd("EXISTS")
            .arg(&key)
            .query_async::<_, i32>(conn)
            .await
        {
            Ok(exists) => Ok(exists == 1),
            Err(e) => Err(format!("Failed to check cross state existence: {}", e)),
        }
    }

    /// Set expiration for cross state (useful for cleanup)
    pub async fn expire_cross_state(
        conn: &mut Connection,
        match_id: &str,
        seconds: u64,
    ) -> Result<(), String> {
        let key = format!("cross_state:{}", match_id);

        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(seconds)
            .query_async::<_, i32>(conn)
            .await
            .map_err(|e| format!("Failed to set cross state expiration: {}", e))?;

        Ok(())
    }
}
