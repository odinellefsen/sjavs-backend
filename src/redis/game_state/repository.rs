use crate::game::hand::Hand;
use deadpool_redis::Connection;
use serde_json;

pub struct GameStateRepository;

impl GameStateRepository {
    /// Store player hands for a game
    pub async fn store_hands(
        conn: &mut Connection,
        game_id: &str,
        hands: &[Hand; 4],
    ) -> Result<(), String> {
        for (i, hand) in hands.iter().enumerate() {
            let key = format!("game:{}:hand:{}", game_id, i);
            let hand_data = serde_json::to_string(hand)
                .map_err(|e| format!("Failed to serialize hand: {}", e))?;

            redis::cmd("SET")
                .arg(&key)
                .arg(&hand_data)
                .query_async::<_, ()>(&mut *conn)
                .await
                .map_err(|e| format!("Failed to store hand: {}", e))?;
        }

        Ok(())
    }

    /// Retrieve a player's hand
    pub async fn get_hand(
        conn: &mut Connection,
        game_id: &str,
        player_position: usize,
    ) -> Result<Option<Hand>, String> {
        let key = format!("game:{}:hand:{}", game_id, player_position);

        let hand_data: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Failed to retrieve hand: {}", e))?;

        match hand_data {
            Some(data) => {
                let hand: Hand = serde_json::from_str(&data)
                    .map_err(|e| format!("Failed to deserialize hand: {}", e))?;
                Ok(Some(hand))
            }
            None => Ok(None),
        }
    }

    /// Get all hands for a game
    pub async fn get_all_hands(
        conn: &mut Connection,
        game_id: &str,
    ) -> Result<Vec<Option<Hand>>, String> {
        let mut hands = Vec::new();

        for i in 0..4 {
            let hand = Self::get_hand(conn, game_id, i).await?;
            hands.push(hand);
        }

        Ok(hands)
    }

    /// Update a specific player's hand (for when cards are played)
    pub async fn update_hand(
        conn: &mut Connection,
        game_id: &str,
        player_position: usize,
        hand: &Hand,
    ) -> Result<(), String> {
        let key = format!("game:{}:hand:{}", game_id, player_position);
        let hand_data =
            serde_json::to_string(hand).map_err(|e| format!("Failed to serialize hand: {}", e))?;

        redis::cmd("SET")
            .arg(&key)
            .arg(&hand_data)
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to update hand: {}", e))?;

        Ok(())
    }

    /// Remove all hands for a game (cleanup or redeal)
    pub async fn clear_hands(conn: &mut Connection, game_id: &str) -> Result<(), String> {
        for i in 0..4 {
            let key = format!("game:{}:hand:{}", game_id, i);
            redis::cmd("DEL")
                .arg(&key)
                .query_async::<_, ()>(&mut *conn)
                .await
                .map_err(|e| format!("Failed to delete hand: {}", e))?;
        }

        Ok(())
    }

    /// Store game analysis data for debugging/statistics
    pub async fn store_hand_analysis(
        conn: &mut Connection,
        game_id: &str,
        analysis: &crate::game::deck::HandAnalysis,
    ) -> Result<(), String> {
        let key = format!("game:{}:analysis", game_id);
        let analysis_data = format!(
            "players_with_bids:{},best_bid:{},suits:{:?}",
            analysis.players_with_valid_bids, analysis.best_bid_length, analysis.best_bid_suits
        );

        redis::cmd("SET")
            .arg(&key)
            .arg(&analysis_data)
            .arg("EX")
            .arg(3600) // Expire after 1 hour
            .query_async::<_, ()>(&mut *conn)
            .await
            .map_err(|e| format!("Failed to store analysis: {}", e))?;

        Ok(())
    }

    /// Get game analysis data
    pub async fn get_hand_analysis(
        conn: &mut Connection,
        game_id: &str,
    ) -> Result<Option<String>, String> {
        let key = format!("game:{}:analysis", game_id);

        redis::cmd("GET")
            .arg(&key)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Failed to retrieve analysis: {}", e))
    }

    /// Check if hands exist for a game (to verify game state)
    pub async fn hands_exist(conn: &mut Connection, game_id: &str) -> Result<bool, String> {
        for i in 0..4 {
            let key = format!("game:{}:hand:{}", game_id, i);
            let exists: bool = redis::cmd("EXISTS")
                .arg(&key)
                .query_async(&mut *conn)
                .await
                .map_err(|e| format!("Failed to check hand existence: {}", e))?;

            if !exists {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Set expiration for all hands (cleanup after game completion)
    pub async fn expire_hands(
        conn: &mut Connection,
        game_id: &str,
        ttl_seconds: u64,
    ) -> Result<(), String> {
        for i in 0..4 {
            let key = format!("game:{}:hand:{}", game_id, i);
            redis::cmd("EXPIRE")
                .arg(&key)
                .arg(ttl_seconds)
                .query_async::<_, ()>(&mut *conn)
                .await
                .map_err(|e| format!("Failed to set expiration: {}", e))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These are integration tests that would need a Redis instance
    // For now, they're commented out but show the intended usage

    /*
    #[tokio::test]
    async fn test_store_and_retrieve_hands() {
        // This would test the full flow:
        // 1. Create test hands
        // 2. Store in Redis
        // 3. Retrieve and verify

        let test_hands = [
            Hand::new(vec![Card::new(Suit::Hearts, Rank::Ace)], 0),
            Hand::new(vec![Card::new(Suit::Spades, Rank::King)], 1),
            Hand::new(vec![Card::new(Suit::Clubs, Rank::Queen)], 2),
            Hand::new(vec![Card::new(Suit::Diamonds, Rank::Jack)], 3),
        ];

        // Would need Redis connection setup here
        // let mut conn = get_test_redis_connection().await;
        //
        // GameStateRepository::store_hands(&mut conn, "test_game", &test_hands).await.unwrap();
        //
        // let retrieved_hands = GameStateRepository::get_all_hands(&mut conn, "test_game").await.unwrap();
        //
        // for (i, hand) in retrieved_hands.iter().enumerate() {
        //     assert!(hand.is_some());
        //     assert_eq!(hand.as_ref().unwrap().player_position, i);
        // }
    }
    */
}
