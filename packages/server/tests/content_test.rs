//! Integration tests for the content endpoint with actual document content.
//!
//! These tests verify that the content endpoint correctly extracts ProseMirror
//! JSON from Yrs documents that have been edited, and that markdown conversion
//! works end-to-end through the sidecar.
//!
//! Requires: PostgreSQL, LocalStack S3, and sidecar (for markdown tests).

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

static TEST_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(3000);

async fn spawn_test_server() -> Option<(String, Arc<AppState>)> {
    if should_skip() {
        return None;
    }

    let db = match Database::connect(&database_url()).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Skipping content test — cannot connect to database: {e}");
            return None;
        }
    };

    if let Err(e) = sqlx::migrate!().run(&db.pool).await {
        eprintln!("Skipping content test — migration failed: {e}");
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

    let app = build_router(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Some((format!("http://127.0.0.1:{}", addr.port()), state))
}

async fn register_test_user(client: &reqwest::Client, base_url: &str) -> String {
    let n = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let unique = uuid::Uuid::new_v4();
    let resp = client
        .post(format!("{base_url}/api/auth/register"))
        .json(&serde_json::json!({
            "email": format!("content{n}-{unique}@example.com"),
            "display_name": format!("Content Test {n}"),
            "password": "password123"
        }))
        .send()
        .await
        .expect("register request failed");
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

/// Insert ProseMirror content into a document session's Yrs doc.
/// This simulates what y-prosemirror does when the user edits in the browser.
async fn inject_prosemirror_content(state: &AppState, doc_id: uuid::Uuid) {
    use yrs::types::xml::{XmlElementPrelim, XmlFragment, XmlTextPrelim};
    use yrs::{Text, Transact, WriteTxn, Xml};

    let session = state
        .document_sessions
        .get_or_load(doc_id, &state.db, &state.storage)
        .await
        .unwrap();

    let awareness = session.awareness.write().await;
    let doc = awareness.doc();
    let mut txn = doc.transact_mut();
    let frag = txn.get_or_insert_xml_fragment("prosemirror");

    // Add a heading: <heading level="1">Hello World</heading>
    let heading = frag.insert(&mut txn, 0, XmlElementPrelim::empty("heading"));
    heading.insert_attribute(&mut txn, "level", "1");
    let heading_text = heading.insert(&mut txn, 0, XmlTextPrelim::new(""));
    heading_text.push(&mut txn, "Hello World");

    // Add a paragraph with mixed formatting: "This is **bold** text"
    let para = frag.insert(&mut txn, 1, XmlElementPrelim::empty("paragraph"));
    let para_text = para.insert(&mut txn, 0, XmlTextPrelim::new(""));
    para_text.push(&mut txn, "This is ");
    let bold_attrs =
        yrs::types::Attrs::from([(std::sync::Arc::from("bold"), yrs::Any::Bool(true))]);
    para_text.insert_with_attributes(&mut txn, 8, "bold", bold_attrs);
    para_text.push(&mut txn, " text");
}

#[tokio::test]
async fn test_get_content_with_edited_document_json() {
    let Some((base_url, state)) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let token = register_test_user(&client, &base_url).await;

    // Create a document
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Edited Doc" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id: uuid::Uuid = doc["id"].as_str().unwrap().parse().unwrap();

    // Inject content into the prosemirror fragment
    inject_prosemirror_content(&state, doc_id).await;

    // Get content as JSON
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}/content"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    assert_eq!(body["format"], "json");
    let content = &body["content"];
    assert_eq!(content["type"], "doc");

    let nodes = content["content"].as_array().unwrap();
    assert_eq!(nodes.len(), 2, "should have heading + paragraph");

    // Verify heading
    assert_eq!(nodes[0]["type"], "heading");
    assert_eq!(nodes[0]["attrs"]["level"], "1");
    let heading_content = nodes[0]["content"].as_array().unwrap();
    assert!(
        heading_content.iter().any(|n| n["text"] == "Hello World"),
        "heading should contain 'Hello World'"
    );

    // Verify paragraph has text content
    let para_content = nodes[1]["content"].as_array().unwrap();
    assert!(!para_content.is_empty(), "paragraph should have text nodes");

    // Find a text node with bold mark
    let bold_node = para_content
        .iter()
        .find(|n| n["marks"].is_array())
        .expect("should have a text node with bold marks");
    assert_eq!(bold_node["marks"][0]["type"], "bold");
    let bold_text = bold_node["text"].as_str().unwrap();
    assert!(
        bold_text.contains("bold"),
        "bold text node should contain 'bold', got: {bold_text}"
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
async fn test_get_content_with_edited_document_markdown() {
    let Some((base_url, state)) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let token = register_test_user(&client, &base_url).await;

    // Create a document
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Markdown Doc" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id: uuid::Uuid = doc["id"].as_str().unwrap().parse().unwrap();

    // Inject content
    inject_prosemirror_content(&state, doc_id).await;

    // Get content as markdown (requires sidecar)
    let resp = client
        .get(format!(
            "{base_url}/api/docs/{doc_id}/content?format=markdown"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    let status = resp.status();
    if status == 200 {
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["format"], "markdown");
        let markdown = body["content"].as_str().unwrap();
        // Should contain the heading and bold text
        assert!(
            markdown.contains("Hello World"),
            "markdown should contain heading text, got: {markdown}"
        );
        assert!(
            markdown.contains("**bold"),
            "markdown should contain bold formatting, got: {markdown}"
        );
    } else {
        assert_eq!(
            status, 502,
            "Expected 200 (sidecar running) or 502 (sidecar down), got {status}"
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
async fn test_get_content_after_flush_and_reload() {
    let Some((base_url, state)) = spawn_test_server().await else {
        return;
    };

    let client = reqwest::Client::new();
    let token = register_test_user(&client, &base_url).await;

    // Create a document
    let resp = client
        .post(format!("{base_url}/api/docs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "Flush Test Doc" }))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = resp.json().await.unwrap();
    let doc_id: uuid::Uuid = doc["id"].as_str().unwrap().parse().unwrap();

    // Inject content
    inject_prosemirror_content(&state, doc_id).await;

    // Flush to S3
    {
        let session = state
            .document_sessions
            .get_or_load(doc_id, &state.db, &state.storage)
            .await
            .unwrap();
        session
            .flush(&state.db, &state.storage)
            .await
            .expect("flush failed");
    }

    // Unload from memory — force reload from S3 on next access
    state.document_sessions.unload(doc_id);

    // Get content — should reload from S3 and still work
    let resp = client
        .get(format!("{base_url}/api/docs/{doc_id}/content"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    assert_eq!(body["format"], "json");
    let nodes = body["content"]["content"].as_array().unwrap();
    assert_eq!(
        nodes.len(),
        2,
        "should still have heading + paragraph after reload"
    );
    assert_eq!(nodes[0]["type"], "heading");
    assert_eq!(nodes[1]["type"], "paragraph");

    // Cleanup
    client
        .delete(format!("{base_url}/api/docs/{doc_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .ok();
}
