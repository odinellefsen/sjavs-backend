use deadpool_redis::Connection;

pub struct PlayerRepository;

impl PlayerRepository {
    /// Check if a player is in any game
    pub async fn get_player_game(
        conn: &mut Connection,
        user_id: &str,
    ) -> Result<Option<String>, String> {
        let game_id: Option<String> = redis::cmd("HGET")
            .arg("player_games")
            .arg(user_id)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Redis error: {}", e))?;

        Ok(game_id)
    }

    /// Associate a player with a game
    pub async fn associate_with_game(
        conn: &mut Connection,
        user_id: &str,
        game_id: &str,
    ) -> Result<(), String> {
        redis::cmd("HSET")
            .arg("player_games")
            .arg(user_id)
            .arg(game_id)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to associate player with game: {}", e))?;

        Ok(())
    }

    /// Remove a player's association with any game
    pub async fn remove_game_association(
        conn: &mut Connection,
        user_id: &str,
    ) -> Result<(), String> {
        redis::cmd("HDEL")
            .arg("player_games")
            .arg(user_id)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to remove player-game association: {}", e))?;

        Ok(())
    }

    /// Get all players in a specific game with their roles
    pub async fn get_players_in_game(
        conn: &mut Connection,
        game_id: &str,
    ) -> Result<Vec<PlayerGameInfo>, String> {
        use std::collections::HashMap;

        let players_key = format!("normal_match:{}:players", game_id);

        let players_hash: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&players_key)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Failed to get players: {}", e))?;

        let players = players_hash
            .into_iter()
            .map(|(user_id, role)| PlayerGameInfo { user_id, role })
            .collect();

        Ok(players)
    }
}

/// Information about a player in a game
#[derive(Debug, Clone)]
pub struct PlayerGameInfo {
    pub user_id: String,
    pub role: String,
}
