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

/// A unique counter to generate unique emails per test invocation.
static TEST_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

/// Register a test user and return the access token.
async fn register_test_user(client: &reqwest::Client, base_url: &str) -> String {
    let n = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let unique = uuid::Uuid::new_v4();
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": format!("testuser{n}-{unique}@example.com"),
            "display_name": format!("Test User {n}"),
            "password": "password123"
        }))
        .send()
        .await
        .expect("register request failed");

    assert_eq!(resp.status(), 201, "register should succeed");
    let body: serde_json::Value = resp.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_create_and_list_documents() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let token = register_test_user(&client, &base_url).await;

    // Create a document
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
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
        .bearer_auth(&token)
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
        .bearer_auth(&token)
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
    let token = register_test_user(&client, &base_url).await;

    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
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
    let token = register_test_user(&client, &base_url).await;

    // Create a doc
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Get Test" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    // Get it
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
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
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
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
    let token = register_test_user(&client, &base_url).await;

    // Create a doc
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Delete Test" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    // Delete it
    let resp = client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify it's gone
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Delete nonexistent → 404
    let resp = client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
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
    let token = register_test_user(&client, &base_url).await;

    // Create a doc
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Original Title" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    // Rename it
    let resp = client
        .patch(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
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
        .bearer_auth(&token)
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
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_unauthenticated_request_returns_401() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    // No Authorization header → 401
    let resp = client
        .get(format!("{base_url}/api/docs"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_expired_token_returns_401() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    // Manually create an expired token
    use chrono::Utc;
    use jsonwebtoken::{encode, EncodingKey, Header};

    #[derive(serde::Serialize)]
    struct Claims {
        sub: String,
        email: Option<String>,
        name: Option<String>,
        #[serde(rename = "type")]
        token_type: String,
        iat: i64,
        exp: i64,
    }

    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: uuid::Uuid::new_v4().to_string(),
        email: Some("expired@example.com".to_string()),
        name: Some("Expired User".to_string()),
        token_type: "access".to_string(),
        iat: now - 1000,
        exp: now - 500, // expired
    };
    let expired_token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(b"test-secret"),
    )
    .unwrap();

    let resp = client
        .get(format!("{base_url}/api/docs"))
        .bearer_auth(&expired_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_refresh_token_rejected_as_access_token() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    // Register to get a refresh token
    let n = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let unique = uuid::Uuid::new_v4();
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": format!("refreshtest{n}-{unique}@example.com"),
            "display_name": "Refresh Tester",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let refresh_token = body["refresh_token"].as_str().unwrap();

    // Use refresh token as access token → 401
    let resp = client
        .get(format!("{base_url}/api/docs"))
        .bearer_auth(refresh_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
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

// --- Content endpoint tests ---

#[tokio::test]
async fn test_get_content_returns_json_by_default() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let token = register_test_user(&client, &base_url).await;

    // Create a document
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Content Test" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    // Get content (default format = json)
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}/content"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["format"], "json");
    assert_eq!(body["content"]["type"], "doc");
    assert!(body["content"]["content"].is_array());

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_content_explicit_json_format() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let token = register_test_user(&client, &base_url).await;

    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "JSON Format Test" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}/content?format=json"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["format"], "json");
    assert_eq!(body["content"]["type"], "doc");

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_content_markdown_format() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let token = register_test_user(&client, &base_url).await;

    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Markdown Format Test" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    // This test requires the sidecar to be running.
    // If it fails with 502, that's expected when sidecar is down.
    let resp = client
        .get(format!(
            "{base_url}/api/docs/{doc_id}/content?format=markdown"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    // Accept either 200 (sidecar running) or 502 (sidecar not running)
    let status = resp.status();
    if status == 200 {
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["format"], "markdown");
        assert!(body["content"].is_string());
    } else {
        assert_eq!(
            status, 502,
            "Expected 200 (sidecar running) or 502 (sidecar not running), got {status}"
        );
    }

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_content_invalid_format_returns_400() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let token = register_test_user(&client, &base_url).await;

    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Invalid Format Test" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    let resp = client
        .get(format!(
            "{base_url}/api/docs/{doc_id}/content?format=invalid"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_content_unauthorized_returns_403() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let owner_token = register_test_user(&client, &base_url).await;
    let other_token = register_test_user(&client, &base_url).await;

    // Owner creates a document
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "title": "Private Doc" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    // Other user tries to get content — should be 403
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}/content"))
        .bearer_auth(&other_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&owner_token)
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_content_unauthenticated_returns_401() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    // Try to get content without auth
    let resp = client
        .get(format!(
            "{base_url}/api/docs/00000000-0000-0000-0000-000000000000/content"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_get_content_empty_doc_returns_default() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let token = register_test_user(&client, &base_url).await;

    // Create a document (no edits through WebSocket)
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Empty Doc" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id = doc["id"].as_str().unwrap();

    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}/content"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    // Should return the default empty doc
    assert_eq!(body["format"], "json");
    assert_eq!(body["content"]["type"], "doc");
    let content = body["content"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "paragraph");

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .ok();
}
