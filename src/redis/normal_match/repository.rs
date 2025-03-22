use crate::redis::normal_match::id::NormalMatch;
use crate::redis::player::repository::PlayerRepository;
use deadpool_redis::Connection;
use std::collections::HashMap;

pub struct NormalMatchRepository;

impl NormalMatchRepository {
    /// Create a new normal match in Redis
    pub async fn create(
        conn: &mut Connection,
        normal_match: &NormalMatch,
        host_id: &str,
    ) -> Result<(), String> {
        // 1. Set game pin
        redis::cmd("HSET")
            .arg("game_pins")
            .arg(normal_match.pin.to_string())
            .arg(&normal_match.id)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to set game pin: {}", e))?;

        // 2. Store the match data
        let redis_key = normal_match.redis_key();
        let hash_map = normal_match.to_redis_hash();

        redis::cmd("HSET")
            .arg(&redis_key)
            .arg(hash_map)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to create game: {}", e))?;

        // 3. Add host to players list
        redis::cmd("HSET")
            .arg(format!("{}:players", redis_key))
            .arg(host_id)
            .arg("host")
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to set host player: {}", e))?;

        // 4. Associate player with game - use PlayerRepository instead
        PlayerRepository::associate_with_game(conn, host_id, &normal_match.id).await?;

        Ok(())
    }

    /// Get a normal match by ID
    pub async fn get_by_id(
        conn: &mut Connection,
        game_id: &str,
    ) -> Result<Option<NormalMatch>, String> {
        let redis_key = format!("normal_match:{}", game_id);

        let hash: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&redis_key)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Redis error: {}", e))?;

        if hash.is_empty() {
            return Ok(None);
        }

        let normal_match = NormalMatch::from_redis_hash(game_id.to_string(), &hash)?;
        Ok(Some(normal_match))
    }

    /// Get a match ID by pin code
    pub async fn get_id_by_pin(
        conn: &mut Connection,
        pin_code: &str,
    ) -> Result<Option<String>, String> {
        let game_id: Option<String> = redis::cmd("HGET")
            .arg("game_pins")
            .arg(pin_code)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Redis error: {}", e))?;

        Ok(game_id)
    }

    /// Add a player to a match
    pub async fn add_player(
        conn: &mut Connection,
        game_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<(), String> {
        let redis_key = format!("normal_match:{}", game_id);

        // Add player to the players list
        redis::cmd("HSET")
            .arg(format!("{}:players", redis_key))
            .arg(user_id)
            .arg(role)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to add player: {}", e))?;

        // Associate player with game - use PlayerRepository instead
        PlayerRepository::associate_with_game(conn, user_id, game_id).await?;

        Ok(())
    }

    /// Remove a player from a match
    pub async fn remove_player(
        conn: &mut Connection,
        game_id: &str,
        user_id: &str,
    ) -> Result<bool, String> {
        let redis_key = format!("normal_match:{}", game_id);
        let players_key = format!("{}:players", redis_key);

        // Remove player from the game's player list
        redis::cmd("HDEL")
            .arg(&players_key)
            .arg(user_id)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to remove player: {}", e))?;

        // Check if there are any players left
        let remaining_players: u32 = redis::cmd("HLEN")
            .arg(&players_key)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Failed to count remaining players: {}", e))?;

        if remaining_players == 0 {
            // Get the match data to find the pin
            let match_data = Self::get_by_id(conn, game_id).await?;

            // No players left, delete the game
            redis::cmd("DEL")
                .arg(&redis_key)
                .arg(&players_key)
                .query_async::<_, ()>(&mut *conn)
                .await
                .map_err(|e| format!("Failed to delete game: {}", e))?;

            // Also remove the PIN mapping if it exists
            if let Some(m) = match_data {
                redis::cmd("HDEL")
                    .arg("game_pins")
                    .arg(m.pin.to_string())
                    .query_async::<_, ()>(&mut *conn)
                    .await
                    .map_err(|e| format!("Failed to remove pin mapping: {}", e))?;
            }

            PlayerRepository::remove_game_association(conn, user_id).await?;

            return Ok(true); // Game was deleted
        }

        PlayerRepository::remove_game_association(conn, user_id).await?;

        Ok(false) // Game still exists
    }

    // Add more methods as needed...
}
