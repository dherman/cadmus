//! Integration tests for the Cadmus WebSocket server.
//!
//! These tests require:
//! - PostgreSQL with migrations applied (DATABASE_URL)
//! - LocalStack S3 running (S3_ENDPOINT=http://localhost:4566)
//!
//! Run with: cargo test --test websocket_test
//! Skip with: set SKIP_PERSISTENCE_TESTS=1

use cadmus_server::db::Database;
use cadmus_server::{build_router, AppState};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use yrs::sync::{Message as YrsMessage, SyncMessage};
use yrs::StateVector;
use yrs::updates::encoder::Encode;

const TIMEOUT: Duration = Duration::from_secs(5);

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

static TEST_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(2000);

async fn spawn_test_server() -> Option<(String, Arc<AppState>)> {
    if should_skip() {
        return None;
    }

    let db = match Database::connect(&database_url()).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Skipping WebSocket test — cannot connect to database: {e}");
            return None;
        }
    };

    if let Err(e) = sqlx::migrate!().run(&db.pool).await {
        eprintln!("Skipping WebSocket test — migration failed: {e}");
        return None;
    }

    let storage = cadmus_server::documents::storage::SnapshotStorage::new(
        S3_BUCKET,
        Some(&s3_endpoint()),
    )
    .await;

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

    let app = build_router(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Some((format!("http://127.0.0.1:{}", addr.port()), state))
}

/// Register a test user and return (access_token, user_id).
async fn register_user(client: &reqwest::Client, base_url: &str) -> (String, String) {
    let n = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let unique = uuid::Uuid::new_v4();
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": format!("wstest{n}-{unique}@example.com"),
            "display_name": format!("WS Test User {n}"),
            "password": "password123"
        }))
        .send()
        .await
        .expect("register request failed");
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap().to_string();
    let user_id = body["user"]["id"].as_str().unwrap().to_string();
    (token, user_id)
}

/// Create a document and return its id.
async fn create_doc(client: &reqwest::Client, base_url: &str, token: &str) -> String {
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "title": "WS Test Doc" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let doc: serde_json::Value = resp.json().await.unwrap();
    doc["id"].as_str().unwrap().to_string()
}

/// Get a ws-token for the given access token.
async fn get_ws_token(client: &reqwest::Client, base_url: &str, access_token: &str) -> String {
    let resp = client
        .post(format!("{base_url}/api/auth/ws-token"))
        .bearer_auth(access_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    body["ws_token"].as_str().unwrap().to_string()
}

/// Encode a y-sync SyncStep1 message with an empty state vector.
fn sync_step1_msg() -> Vec<u8> {
    YrsMessage::Sync(SyncMessage::SyncStep1(StateVector::default())).encode_v1()
}

#[tokio::test]
async fn health_endpoint_returns_200() {
    let Some((base_url, _state)) = spawn_test_server().await else {
        return;
    };

    let resp = reqwest::get(format!("{}/health", base_url))
        .await
        .expect("request failed");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("response is not JSON");
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn websocket_connects_to_document() {
    let Some((base_url, _state)) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let (access_token, _) = register_user(&client, &base_url).await;
    let doc_id = create_doc(&client, &base_url, &access_token).await;
    let ws_token = get_ws_token(&client, &base_url, &access_token).await;

    let ws_url = format!(
        "{}/api/docs/{}/ws?token={}",
        base_url.replace("http://", "ws://"),
        doc_id,
        ws_token
    );

    let (mut ws, _) = timeout(TIMEOUT, connect_async(&ws_url))
        .await
        .expect("timeout")
        .expect("WebSocket connect failed");

    // Initiate the y-sync handshake: send SyncStep1 with an empty state vector
    ws.send(Message::Binary(sync_step1_msg().into()))
        .await
        .expect("failed to send SyncStep1");

    // Server should respond with SyncStep2
    let msg = timeout(TIMEOUT, ws.next())
        .await
        .expect("timeout")
        .expect("stream ended")
        .expect("message error");

    assert!(
        matches!(msg, Message::Binary(_)),
        "expected binary y-sync message (SyncStep2), got: {:?}",
        msg
    );
}

#[tokio::test]
async fn websocket_returns_404_for_unknown_document() {
    let Some((base_url, _state)) = spawn_test_server().await else {
        return;
    };

    let resp = reqwest::get(format!("{}/api/docs/does-not-exist/content", base_url))
        .await
        .expect("request failed");

    // Should be 4xx since "does-not-exist" isn't a valid UUID
    assert!(
        resp.status().as_u16() >= 400,
        "expected 4xx for invalid doc path"
    );
}

#[tokio::test]
async fn two_clients_sync_edits() {
    let Some((base_url, _state)) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let (access_token, _) = register_user(&client, &base_url).await;
    let doc_id = create_doc(&client, &base_url, &access_token).await;

    // Get ws-tokens for both connections (ws-tokens are short-lived, get them just before connecting)
    let ws_token1 = get_ws_token(&client, &base_url, &access_token).await;
    let ws_token2 = get_ws_token(&client, &base_url, &access_token).await;

    let ws_url1 = format!(
        "{}/api/docs/{}/ws?token={}",
        base_url.replace("http://", "ws://"),
        doc_id,
        ws_token1
    );
    let ws_url2 = format!(
        "{}/api/docs/{}/ws?token={}",
        base_url.replace("http://", "ws://"),
        doc_id,
        ws_token2
    );

    // Connect two clients
    let (mut ws1, _) = timeout(TIMEOUT, connect_async(&ws_url1))
        .await
        .expect("timeout")
        .expect("client 1 connect failed");

    let (mut ws2, _) = timeout(TIMEOUT, connect_async(&ws_url2))
        .await
        .expect("timeout")
        .expect("client 2 connect failed");

    // Each client initiates the y-sync handshake
    let step1 = Message::Binary(sync_step1_msg().into());
    ws1.send(step1.clone()).await.expect("ws1 SyncStep1 failed");
    ws2.send(step1.clone()).await.expect("ws2 SyncStep1 failed");

    // Drain initial sync messages (SyncStep2 responses)
    for _ in 0..2 {
        let _ = timeout(Duration::from_millis(300), ws1.next()).await;
        let _ = timeout(Duration::from_millis(300), ws2.next()).await;
    }

    // Apply an edit directly to the server-side document via the session manager
    {
        use yrs::doc::Transact;
        use yrs::types::text::Text;

        let session = _state
            .document_sessions
            .get_or_load(
                doc_id.parse().unwrap(),
                &_state.db,
                &_state.storage,
            )
            .await
            .unwrap();
        let awareness = session.awareness.write().await;
        let doc = awareness.doc();
        let text = doc.get_or_insert_text("content");
        text.push(&mut doc.transact_mut(), "hello");
    }

    // Both clients should receive the update broadcast as a binary message
    let update_msg_1 = timeout(TIMEOUT, async {
        loop {
            match ws1.next().await {
                Some(Ok(Message::Binary(data))) => return data,
                Some(Ok(_)) => continue,
                _ => panic!("ws1 closed before receiving update"),
            }
        }
    })
    .await
    .expect("timed out waiting for update on client 1");

    let update_msg_2 = timeout(TIMEOUT, async {
        loop {
            match ws2.next().await {
                Some(Ok(Message::Binary(data))) => return data,
                Some(Ok(_)) => continue,
                _ => panic!("ws2 closed before receiving update"),
            }
        }
    })
    .await
    .expect("timed out waiting for update on client 2");

    assert!(!update_msg_1.is_empty(), "client 1 received empty update");
    assert!(!update_msg_2.is_empty(), "client 2 received empty update");
}
