//! Integration tests for permission enforcement.
//!
//! These tests require:
//! - PostgreSQL with migrations applied (DATABASE_URL)
//! - LocalStack S3 running (S3_ENDPOINT=http://localhost:4566)
//!
//! Run with: cargo test --test permissions_test
//! Skip with: set SKIP_PERSISTENCE_TESTS=1

use cadmus_server::db::Database;
use cadmus_server::documents::storage::SnapshotStorage;
use cadmus_server::{build_router, AppState};
use std::sync::Arc;
use tokio::net::TcpListener;

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

static TEST_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);

async fn spawn_test_server() -> Option<String> {
    if should_skip() {
        return None;
    }

    let db = match Database::connect(&database_url()).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Skipping permissions test — cannot connect to database: {e}");
            return None;
        }
    };

    if let Err(e) = sqlx::migrate!().run(&db.pool).await {
        eprintln!("Skipping permissions test — migration failed: {e}");
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

/// Register a test user and return (access_token, user_id, email).
async fn register_user(
    client: &reqwest::Client,
    base_url: &str,
    name: &str,
) -> (String, String, String) {
    let n = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let unique = uuid::Uuid::new_v4();
    let email = format!("perm-{name}-{n}-{unique}@example.com");
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": &email,
            "display_name": name,
            "password": "password123"
        }))
        .send()
        .await
        .expect("register request failed");

    assert_eq!(resp.status(), 201, "register should succeed");
    let body: serde_json::Value = resp.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap().to_string();
    let user_id = body["user"]["id"].as_str().unwrap().to_string();
    (token, user_id, email)
}

/// Create a document and return its id.
async fn create_doc(client: &reqwest::Client, base_url: &str, token: &str) -> String {
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "title": "Permission Test Doc" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let doc: serde_json::Value = resp.json().await.unwrap();
    doc["id"].as_str().unwrap().to_string()
}

/// Share a document with a user by email.
async fn share_doc(
    client: &reqwest::Client,
    base_url: &str,
    owner_token: &str,
    doc_id: &str,
    email: &str,
    role: &str,
) {
    let resp = client
        .post(format!("{base_url}/api/docs/{doc_id}/permissions"))
        .bearer_auth(owner_token)
        .json(&serde_json::json!({ "email": email, "role": role }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        201,
        "sharing should succeed: {}",
        resp.text().await.unwrap_or_default()
    );
}

// ────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_no_permission_returns_403() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let (other_token, _, _) = register_user(&client, &base_url, "other").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;

    // Other user has no permission — GET should 403
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&other_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_read_user_can_get_but_not_edit() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let (reader_token, _, reader_email) = register_user(&client, &base_url, "reader").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;
    share_doc(
        &client,
        &base_url,
        &owner_token,
        &doc_id,
        &reader_email,
        "read",
    )
    .await;

    // Read user can GET
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&reader_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Read user cannot PATCH (edit)
    let resp = client
        .patch(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&reader_token)
        .json(&serde_json::json!({ "title": "hacked" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    // Read user cannot DELETE
    let resp = client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&reader_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_comment_user_can_get_and_view_comments() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let (commenter_token, _, commenter_email) =
        register_user(&client, &base_url, "commenter").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;
    share_doc(
        &client,
        &base_url,
        &owner_token,
        &doc_id,
        &commenter_email,
        "comment",
    )
    .await;

    // Comment user can GET doc
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&commenter_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Comment user can list comments
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}/comments"))
        .bearer_auth(&commenter_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Comment user cannot PATCH (edit metadata)
    let resp = client
        .patch(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&commenter_token)
        .json(&serde_json::json!({ "title": "hacked" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_edit_user_can_edit_but_not_delete() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let (editor_token, _, editor_email) = register_user(&client, &base_url, "editor").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;
    share_doc(
        &client,
        &base_url,
        &owner_token,
        &doc_id,
        &editor_email,
        "edit",
    )
    .await;

    // Edit user can GET
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&editor_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Edit user can PATCH
    let resp = client
        .patch(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&editor_token)
        .json(&serde_json::json!({ "title": "Renamed by editor" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Edit user cannot DELETE (non-owner)
    let resp = client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&editor_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_owner_can_delete() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;

    let resp = client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_owner_can_invite_change_role_remove() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let (_, bob_id, bob_email) = register_user(&client, &base_url, "bob").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;

    // Invite bob with read
    share_doc(
        &client,
        &base_url,
        &owner_token,
        &doc_id,
        &bob_email,
        "read",
    )
    .await;

    // List permissions
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}/permissions"))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let perms: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(perms.len() >= 2); // owner + bob

    // Change bob's role to edit
    let resp = client
        .patch(format!(
            "{base_url}/api/docs/{doc_id}/permissions/{bob_id}"
        ))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "role": "edit" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Remove bob's access
    let resp = client
        .delete(format!(
            "{base_url}/api/docs/{doc_id}/permissions/{bob_id}"
        ))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_non_owner_cannot_manage_permissions() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let (editor_token, _, editor_email) = register_user(&client, &base_url, "editor").await;
    let (_, _, charlie_email) = register_user(&client, &base_url, "charlie").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;
    share_doc(
        &client,
        &base_url,
        &owner_token,
        &doc_id,
        &editor_email,
        "edit",
    )
    .await;

    // Editor cannot invite
    let resp = client
        .post(format!("{base_url}/api/docs/{doc_id}/permissions"))
        .bearer_auth(&editor_token)
        .json(&serde_json::json!({ "email": charlie_email, "role": "read" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_document_listing_scoped_to_user() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (alice_token, _, _) = register_user(&client, &base_url, "alice").await;
    let (bob_token, _, _) = register_user(&client, &base_url, "bob").await;

    // Alice creates a doc
    let alice_doc = create_doc(&client, &base_url, &alice_token).await;

    // Bob creates a doc
    let bob_doc = create_doc(&client, &base_url, &bob_token).await;

    // Alice's list should include her doc but NOT bob's
    let resp = client
        .get(format!("{base_url}/api/docs"))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let docs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(docs
        .iter()
        .any(|d| d["id"].as_str() == Some(&alice_doc)));
    assert!(!docs.iter().any(|d| d["id"].as_str() == Some(&bob_doc)));

    // Bob's list should include his doc but NOT alice's
    let resp = client
        .get(format!("{base_url}/api/docs"))
        .bearer_auth(&bob_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let docs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(docs.iter().any(|d| d["id"].as_str() == Some(&bob_doc)));
    assert!(!docs.iter().any(|d| d["id"].as_str() == Some(&alice_doc)));
}

#[tokio::test]
async fn test_websocket_upgrade_without_token_fails() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;

    // Attempt WebSocket upgrade without token query param
    let ws_url = format!(
        "{}/api/docs/{doc_id}/ws",
        base_url.replace("http://", "ws://")
    );

    let result = tokio_tungstenite::connect_async(&ws_url).await;
    assert!(
        result.is_err(),
        "WebSocket upgrade should fail without token"
    );
}

#[tokio::test]
async fn test_websocket_upgrade_with_valid_token() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;

    // Get a ws-token
    let resp = client
        .post(format!("{base_url}/api/auth/ws-token"))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let ws_token = body["ws_token"].as_str().unwrap();

    // Attempt WebSocket upgrade with valid token
    let ws_url = format!(
        "{}/api/docs/{doc_id}/ws?token={ws_token}",
        base_url.replace("http://", "ws://")
    );

    let result = tokio_tungstenite::connect_async(&ws_url).await;
    assert!(
        result.is_ok(),
        "WebSocket upgrade should succeed with valid token"
    );
}

#[tokio::test]
async fn test_websocket_upgrade_no_permission_fails() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let (other_token, _, _) = register_user(&client, &base_url, "other").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;

    // Get ws-token for other user
    let resp = client
        .post(format!("{base_url}/api/auth/ws-token"))
        .bearer_auth(&other_token)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let ws_token = body["ws_token"].as_str().unwrap();

    // Attempt WebSocket upgrade — user has no permission on doc
    let ws_url = format!(
        "{}/api/docs/{doc_id}/ws?token={ws_token}",
        base_url.replace("http://", "ws://")
    );

    let result = tokio_tungstenite::connect_async(&ws_url).await;
    assert!(
        result.is_err(),
        "WebSocket upgrade should fail when user has no permission"
    );
}

#[tokio::test]
async fn test_duplicate_share_returns_409() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };
    let client = reqwest::Client::new();
    let (owner_token, _, _) = register_user(&client, &base_url, "owner").await;
    let (_, _, bob_email) = register_user(&client, &base_url, "bob").await;
    let doc_id = create_doc(&client, &base_url, &owner_token).await;

    // First share — 201
    share_doc(
        &client,
        &base_url,
        &owner_token,
        &doc_id,
        &bob_email,
        "read",
    )
    .await;

    // Second share — 409
    let resp = client
        .post(format!("{base_url}/api/docs/{doc_id}/permissions"))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "email": bob_email, "role": "edit" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}
