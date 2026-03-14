pub mod auth;
pub mod config;
pub mod db;
pub mod documents;
pub mod errors;
pub mod sidecar;
pub mod websocket;

use axum::{
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Shared application state, available to all handlers.
pub struct AppState {
    pub db: db::Database,
    pub document_sessions: Arc<documents::SessionManager>,
    pub storage: documents::storage::SnapshotStorage,
    pub sidecar: sidecar::SidecarClient,
    pub config: config::Config,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health check
        .route(
            "/health",
            get(|| async { Json(json!({ "status": "ok" })) }),
        )
        // Document REST API
        .route("/api/docs", get(documents::api::list_documents))
        .route("/api/docs", post(documents::api::create_document))
        .route("/api/docs/{id}", get(documents::api::get_document))
        .route(
            "/api/docs/{id}",
            delete(documents::api::delete_document),
        )
        .route(
            "/api/docs/{id}",
            patch(documents::api::update_document),
        )
        .route("/api/docs/{id}/content", get(documents::api::get_content))
        .route(
            "/api/docs/{id}/content",
            post(documents::api::push_content),
        )
        // WebSocket endpoint for real-time collaboration
        .route("/api/docs/{id}/ws", get(websocket::handler::ws_upgrade))
        // Sharing / permissions
        .route(
            "/api/docs/{id}/permissions",
            get(documents::permissions::list_permissions),
        )
        .route(
            "/api/docs/{id}/permissions",
            post(documents::permissions::add_permission),
        )
        .route(
            "/api/docs/{id}/permissions/{user_id}",
            patch(documents::permissions::update_permission_handler),
        )
        .route(
            "/api/docs/{id}/permissions/{user_id}",
            delete(documents::permissions::delete_permission_handler),
        )
        // Comments
        .route(
            "/api/docs/{id}/comments",
            get(documents::api::list_comments),
        )
        .route(
            "/api/docs/{id}/comments",
            post(documents::api::create_comment),
        )
        // Auth endpoints
        .route("/api/auth/register", post(auth::handlers::register))
        .route("/api/auth/login", post(auth::handlers::login))
        .route("/api/auth/refresh", post(auth::handlers::refresh))
        .route("/api/auth/ws-token", post(auth::handlers::ws_token))
        .route("/api/auth/me", get(auth::handlers::me))
        // TODO: history endpoints, token management
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
