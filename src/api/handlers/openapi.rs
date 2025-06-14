use axum::response::IntoResponse;
use axum::Json;
use utoipa::OpenApi;

use crate::api::schemas::*;

/// OpenAPI specification for the Sjavs Backend API
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Sjavs Backend API",
        version = "0.1.0",
        description = "A high-performance, real-time backend server for Sjavs, a traditional Faroese card game",
        contact(
            name = "Sjavs Backend Team",
            email = "contact@sjavs.game"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    paths(
        crate::api::handlers::normal_match::create_match_handler,
        get_openapi_json
    ),
    components(schemas(
        CreateMatchResponse,
        MatchState,
        JoinMatchRequest,
        JoinMatchResponse,
        PlayerInfo,
        LeaveMatchResponse,
        ErrorResponse,
        DebugResponse,
        GameMessage,
        JoinEventData,
        TeamUpRequestData,
        TeamUpResponseData
    )),
    tags(
        (name = "Match Management", description = "Endpoints for creating, joining, and leaving matches"),
        (name = "Debug", description = "Debug utilities for development"),
        (name = "Documentation", description = "API documentation endpoints")
    ),
    servers(
        (url = "http://localhost:3000", description = "Local development server"),
        (url = "https://api.sjavs.game", description = "Production server")
    )
)]
pub struct ApiDoc;

/// Get OpenAPI specification in JSON format
/// 
/// Returns the complete OpenAPI 3.0 specification for the Sjavs Backend API.
/// This includes all endpoints, schemas, and documentation.
#[utoipa::path(
    get,
    path = "/openapi.json",
    tag = "Documentation",
    responses(
        (
            status = 200, 
            description = "OpenAPI specification retrieved successfully",
            content_type = "application/json"
        )
    ),
    security(
        ("jwt_auth" = [])
    )
)]
pub async fn get_openapi_json() -> impl IntoResponse {
    Json(ApiDoc::openapi())
} 