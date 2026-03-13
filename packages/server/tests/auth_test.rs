//! Integration tests for the Auth API.
//!
//! These tests require:
//! - PostgreSQL with migrations applied (DATABASE_URL)
//! - LocalStack S3 running (S3_ENDPOINT=http://localhost:4566)
//!
//! Run with: cargo test --test auth_test
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

async fn spawn_test_server() -> Option<String> {
    if should_skip() {
        return None;
    }

    let db = match Database::connect(&database_url()).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Skipping auth test — cannot connect to database: {e}");
            return None;
        }
    };

    if let Err(e) = sqlx::migrate!().run(&db.pool).await {
        eprintln!("Skipping auth test — migration failed: {e}");
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

/// Generate a unique email for test isolation.
fn unique_email() -> String {
    format!("test-{}@example.com", uuid::Uuid::new_v4())
}

#[tokio::test]
async fn test_register_new_user() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let email = unique_email();

    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": email,
            "display_name": "Alice",
            "password": "securepassword123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["user"]["email"], email);
    assert_eq!(body["user"]["display_name"], "Alice");
    assert!(body["user"]["id"].is_string());
    assert!(body["access_token"].is_string());
    assert!(body["refresh_token"].is_string());
    assert_eq!(body["expires_in"], 900);
}

#[tokio::test]
async fn test_register_duplicate_email() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let email = unique_email();

    // Register first time
    client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": email,
            "display_name": "Alice",
            "password": "securepassword123"
        }))
        .send()
        .await
        .unwrap();

    // Register again with same email
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": email,
            "display_name": "Bob",
            "password": "anotherpassword123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_register_invalid_fields() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    // Missing @ in email
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": "not-an-email",
            "display_name": "Alice",
            "password": "securepassword123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Short password
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": unique_email(),
            "display_name": "Alice",
            "password": "short"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_login_valid_credentials() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let email = unique_email();

    // Register
    client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": email,
            "display_name": "Alice",
            "password": "securepassword123"
        }))
        .send()
        .await
        .unwrap();

    // Login
    let resp = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&serde_json::json!({
            "email": email,
            "password": "securepassword123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["user"]["email"], email);
    assert!(body["access_token"].is_string());
    assert!(body["refresh_token"].is_string());
}

#[tokio::test]
async fn test_login_wrong_password() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let email = unique_email();

    // Register
    client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": email,
            "display_name": "Alice",
            "password": "securepassword123"
        }))
        .send()
        .await
        .unwrap();

    // Login with wrong password
    let resp = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&serde_json::json!({
            "email": email,
            "password": "wrongpassword123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_login_nonexistent_email() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&serde_json::json!({
            "email": "nobody@example.com",
            "password": "securepassword123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_refresh_valid_token() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let email = unique_email();

    // Register and get tokens
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": email,
            "display_name": "Alice",
            "password": "securepassword123"
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let refresh_token = body["refresh_token"].as_str().unwrap();

    // Refresh
    let resp = client
        .post(format!("{base_url}/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["access_token"].is_string());
    assert_eq!(body["expires_in"], 900);
}

#[tokio::test]
async fn test_refresh_invalid_token() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/api/auth/refresh"))
        .json(&serde_json::json!({ "refresh_token": "invalid-token" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_me_with_valid_token() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let email = unique_email();

    // Register
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": email,
            "display_name": "Alice",
            "password": "securepassword123"
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let access_token = body["access_token"].as_str().unwrap();

    // GET /me
    let resp = client
        .get(format!("{base_url}/api/auth/me"))
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let profile: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(profile["email"], email);
    assert_eq!(profile["display_name"], "Alice");
}

#[tokio::test]
async fn test_me_without_token() {
    let Some(base_url) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base_url}/api/auth/me"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}
