use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Response when a match is successfully created
#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateMatchResponse {
    /// Success message
    pub message: String,
    /// The created game ID
    pub game_id: String,
    /// The 4-digit PIN code to join the game
    pub game_pin: u32,
    /// Current game state information
    pub state: MatchState,
}

/// Current state of a match
#[derive(Serialize, Deserialize, ToSchema)]
pub struct MatchState {
    /// Unique game identifier
    pub id: String,
    /// 4-digit PIN code for joining
    pub pin: u32,
    /// Current status of the match
    pub status: String,
    /// Number of crosses (games) in this rubber match
    pub number_of_crosses: u32,
    /// Current cross being played
    pub current_cross: u32,
    /// Timestamp when the match was created
    pub created_timestamp: u64,
    /// Host player ID
    pub host: String,
}

/// Request to join a match
#[derive(Serialize, Deserialize, ToSchema)]
pub struct JoinMatchRequest {
    /// 4-digit PIN code of the match to join
    pub pin: u32,
}

/// Response when successfully joining a match
#[derive(Serialize, Deserialize, ToSchema)]
pub struct JoinMatchResponse {
    /// Success message
    pub message: String,
    /// The game ID that was joined
    pub game_id: String,
    /// Current game state
    pub state: MatchState,
    /// List of players in the match
    pub players: Vec<PlayerInfo>,
}

/// Information about a player in the match
#[derive(Serialize, Deserialize, ToSchema)]
pub struct PlayerInfo {
    /// Player's user ID
    pub user_id: String,
    /// Player's role in the match (host, player, etc.)
    pub role: String,
}

/// Response when successfully leaving a match
#[derive(Serialize, Deserialize, ToSchema)]
pub struct LeaveMatchResponse {
    /// Success message
    pub message: String,
    /// Whether the entire game was deleted (true if host left)
    pub game_deleted: bool,
    /// List of players affected by the leave action
    pub affected_players: Vec<String>,
}

/// Standard error response
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// Error message describing what went wrong
    pub error: String,
    /// Optional additional details about the error (can be null)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Success response for debug operations
#[derive(Serialize, Deserialize, ToSchema)]
pub struct DebugResponse {
    /// Operation success message
    pub message: String,
}

/// WebSocket game message structure
#[derive(Serialize, Deserialize, ToSchema)]
pub struct GameMessage {
    /// Type of game event (join, team_up_request, etc.)
    pub event: String,
    /// Event-specific data payload
    pub data: serde_json::Value,
}

/// WebSocket join event data
#[derive(Serialize, Deserialize, ToSchema)]
pub struct JoinEventData {
    /// Game ID to join
    pub game_id: String,
}

/// WebSocket team up request data
#[derive(Serialize, Deserialize, ToSchema)]
pub struct TeamUpRequestData {
    /// Target player to team up with
    pub target_player: String,
    /// Game ID where the team up is requested
    pub game_id: String,
}

/// WebSocket team up response data
#[derive(Serialize, Deserialize, ToSchema)]
pub struct TeamUpResponseData {
    /// Whether the team up request was accepted
    pub accepted: bool,
    /// ID of the player who sent the original request
    pub requesting_player: String,
    /// Game ID where the team up was requested
    pub game_id: String,
}
