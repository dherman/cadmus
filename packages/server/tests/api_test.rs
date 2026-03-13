//! Integration tests for the Document CRUD API.
//!
//! These tests require:
//! - PostgreSQL with migrations applied (DATABASE_URL)
//! - LocalStack S3 running (S3_ENDPOINT=http://localhost:4566)
//!
//! Run with: cargo test --test api_test
//! Skip with: set SKIP_PERSISTENCE_TESTS=1

use cadmus_server::db::Database;
use cadmus_server::documents::storage::SnapshotStorage;
use cadmus_server::{build_router, AppState};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Check if persistence tests should be skipped (no infrastructure available).
fn should_skip() -> bool {
    std::env::var("SKIP_PERSISTENCE_TESTS").is_ok()
}

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/cadmus_test".to_string())
}

fn s3_endpoint() -> String {
    std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:4566".to_string())
}

const S3_BUCKET: &str = "cadmus-documents";

async fn spawn_test_server() -> Option<String> {
    if should_skip() {
        return None;
    }

    let db = match Database::connect(&database_url()).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Skipping API test — cannot connect to database: {e}");
            return None;
        }
    };

    // Run migrations
    if let Err(e) = sqlx::migrate!().run(&db.pool).await {
        eprintln!("Skipping API test — migration failed: {e}");
        return None;
    }

    let storage = SnapshotStorage::new(S3_BUCKET, Some(&s3_endpoint())).await;

    let state = Arc::new(AppState {
        db,
        document_sessions: Arc::new(cadmus_server::documents::SessionManager::new()),
        storage,
        sidecar: cadmus_server::sidecar::SidecarClient::new("http://localhost:3001"),
        config: cadmus_server::config::Config {
            database_url: database_url(),
            sidecar_url: "http://localhost:3001".to_string(),
            jwt_secret: "test-secret".to_string(),
            s3_bucket: S3_BUCKET.to_string(),
            s3_endpoint: Some(s3_endpoint()),
            port: 0,
        },
    });

    let app = build_router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Some(format!("http://127.0.0.1:{}", addr.port()))
}

#[tokio::test]
async fn test_create_and_list_documents() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    // Create a document
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .json(&serde_json::json!({ "title": "Test Doc" }))
        .send()
        .await
        .expect("create request failed");

    assert_eq!(resp.status(), 201);
    let doc: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(doc["title"], "Test Doc");
    let doc_id = doc["id"].as_str().unwrap();
    assert!(!doc_id.is_empty());
    assert!(doc["created_at"].is_string());
    assert!(doc["updated_at"].is_string());

    // List documents — should include the created one
    let resp = client
        .get(format!("{base_url}/api/docs"))
        .send()
        .await
        .expect("list request failed");

    assert_eq!(resp.status(), 200);
    let docs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(
        docs.iter().any(|d| d["id"].as_str() == Some(doc_id)),
        "created document should appear in list"
    );

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_create_document_rejects_empty_title() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/api/docs"))
        .json(&serde_json::json!({ "title": "   " }))
        .send()
        .await
        .expect("create request failed");

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_get_document() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    // Create a doc
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .json(&serde_json::json!({ "title": "Get Test" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    // Get it
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let fetched: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(fetched["title"], "Get Test");
    assert_eq!(fetched["id"].as_str(), Some(doc_id));

    // Get nonexistent
    let resp = client
        .get(format!(
            "{base_url}/api/docs/00000000-0000-0000-0000-000000000000"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_delete_document() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    // Create a doc
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .json(&serde_json::json!({ "title": "Delete Test" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    // Delete it
    let resp = client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify it's gone
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Delete nonexistent → 404
    let resp = client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_update_document_title() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    // Create a doc
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .json(&serde_json::json!({ "title": "Original Title" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    // Rename it
    let resp = client
        .patch(format!("{base_url}/api/docs/{doc_id}"))
        .json(&serde_json::json!({ "title": "Renamed Title" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let updated: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(updated["title"], "Renamed Title");
    assert_eq!(updated["id"].as_str(), Some(doc_id));

    // Rename with empty title → 400
    let resp = client
        .patch(format!("{base_url}/api/docs/{doc_id}"))
        .json(&serde_json::json!({ "title": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Rename nonexistent → 404
    let resp = client
        .patch(format!(
            "{base_url}/api/docs/00000000-0000-0000-0000-000000000000"
        ))
        .json(&serde_json::json!({ "title": "Nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_websocket_rejects_nonexistent_document() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    // Attempt WebSocket upgrade for a document that doesn't exist in the DB
    let ws_url = format!(
        "{}/api/docs/00000000-0000-0000-0000-000000000000/ws",
        base_url.replace("http://", "ws://")
    );

    let result = tokio_tungstenite::connect_async(&ws_url).await;
    assert!(
        result.is_err(),
        "WebSocket upgrade should fail for nonexistent document"
    );
}
