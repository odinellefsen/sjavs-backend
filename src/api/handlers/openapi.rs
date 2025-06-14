use axum::response::IntoResponse;
use axum::Json;
use utoipa::{OpenApi, Modify};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

use crate::api::schemas::*;

/// Security scheme modifier for JWT authentication
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Add JWT security scheme
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "jwt_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }
        
        // Note: OpenAPI version is controlled by utoipa library version
        // The version should ideally be 3.0.x for better validator compatibility
    }
}

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
    modifiers(&SecurityAddon),
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
/// 
/// This endpoint is publicly accessible and does not require authentication.
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
    )
)]
pub async fn get_openapi_json() -> impl IntoResponse {
    Json(ApiDoc::openapi())
} 