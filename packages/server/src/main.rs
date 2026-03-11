use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber;

mod config;
mod db;
mod documents;
mod errors;
mod sidecar;
mod websocket;

/// Shared application state, available to all handlers.
pub struct AppState {
    pub db: db::Database,
    pub document_sessions: documents::SessionManager,
    pub sidecar: sidecar::SidecarClient,
    pub config: config::Config,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = config::Config::from_env();

    let db = db::Database::connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    let state = Arc::new(AppState {
        db,
        document_sessions: documents::SessionManager::new(),
        sidecar: sidecar::SidecarClient::new(&config.sidecar_url),
        config,
    });

    let app = Router::new()
        // Health check
        .route("/health", get(|| async { "ok" }))
        // Document REST API
        .route("/api/docs", get(documents::api::list_documents))
        .route("/api/docs", post(documents::api::create_document))
        .route("/api/docs/{id}", get(documents::api::get_document))
        .route("/api/docs/{id}/content", get(documents::api::get_content))
        .route("/api/docs/{id}/content", post(documents::api::push_content))
        // WebSocket endpoint for real-time collaboration
        .route("/api/docs/{id}/ws", get(websocket::handler::ws_upgrade))
        // Comments
        .route("/api/docs/{id}/comments", get(documents::api::list_comments))
        .route("/api/docs/{id}/comments", post(documents::api::create_comment))
        // TODO: Auth endpoints, history endpoints, token management
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind");

    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
