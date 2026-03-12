use cadmus_server::{build_router, AppState};
use cadmus_server::{config, db, documents, sidecar};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cfg = config::Config::from_env();

    let database = db::Database::connect(&cfg.database_url)
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!()
        .run(&database.pool)
        .await
        .expect("Failed to run database migrations");

    let state = Arc::new(AppState {
        db: database,
        document_sessions: documents::SessionManager::new(),
        sidecar: sidecar::SidecarClient::new(&cfg.sidecar_url),
        config: cfg,
    });

    let port = state.config.port;
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("Failed to bind");

    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, build_router(state)).await.unwrap();
}
