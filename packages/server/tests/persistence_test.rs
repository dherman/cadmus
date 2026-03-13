//! Integration tests for document persistence lifecycle.
//!
//! These tests require:
//! - PostgreSQL with migrations applied (DATABASE_URL)
//! - LocalStack S3 running (S3_ENDPOINT=http://localhost:4566)
//!
//! Run with: cargo test --test persistence_test
//! Skip with: set SKIP_PERSISTENCE_TESTS=1

use cadmus_server::db::Database;
use cadmus_server::documents::storage::SnapshotStorage;
use cadmus_server::documents::SessionManager;
use std::sync::Arc;
use uuid::Uuid;
use yrs::types::text::Text;
use yrs::{GetString, Transact};

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

async fn setup() -> Option<(Database, SnapshotStorage)> {
    if should_skip() {
        return None;
    }

    let db = match Database::connect(&database_url()).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Skipping persistence test — cannot connect to database: {e}");
            return None;
        }
    };

    // Run migrations
    if let Err(e) = sqlx::migrate!().run(&db.pool).await {
        eprintln!("Skipping persistence test — migration failed: {e}");
        return None;
    }

    let storage = SnapshotStorage::new(S3_BUCKET, Some(&s3_endpoint())).await;

    Some((db, storage))
}

#[tokio::test]
async fn test_document_persists_across_restart() {
    let Some((db, storage)) = setup().await else {
        return;
    };

    // Create a document in the database
    let doc_id = Uuid::new_v4();
    db.create_document(doc_id, "Persistence Test", None)
        .await
        .expect("failed to create document");

    // Load document into a session
    let manager = Arc::new(SessionManager::new());
    let session = manager
        .get_or_load(doc_id, &db, &storage)
        .await
        .expect("failed to load document");

    // Apply an edit
    {
        let awareness = session.awareness.write().await;
        let doc = awareness.doc();
        let text = doc.get_or_insert_text("content");
        text.push(&mut doc.transact_mut(), "hello persistence");
    }

    // Flush to S3
    session
        .flush(&db, &storage)
        .await
        .expect("flush failed");

    // Verify snapshot key is set
    let doc_row = db
        .get_document(doc_id)
        .await
        .expect("failed to get document")
        .expect("document not found");
    assert!(
        doc_row.snapshot_key.is_some(),
        "snapshot_key should be set after flush"
    );

    // Simulate restart — drop all sessions
    manager.unload(doc_id);

    // Load again from storage
    let manager2 = Arc::new(SessionManager::new());
    let session2 = manager2
        .get_or_load(doc_id, &db, &storage)
        .await
        .expect("failed to reload document");

    // Verify the content survived
    let awareness = session2.awareness.read().await;
    let doc = awareness.doc();
    let text = doc.get_or_insert_text("content");
    let txn = doc.transact();
    let content = text.get_string(&txn);
    assert_eq!(content, "hello persistence", "document content should survive restart");

    // Cleanup
    db.delete_document(doc_id).await.ok();
}

#[tokio::test]
async fn test_update_log_crash_recovery() {
    let Some((db, storage)) = setup().await else {
        return;
    };

    // Create a document
    let doc_id = Uuid::new_v4();
    db.create_document(doc_id, "Crash Recovery Test", None)
        .await
        .expect("failed to create document");

    // Load into session (this starts update logging)
    let manager = Arc::new(SessionManager::new());
    let session = manager
        .get_or_load(doc_id, &db, &storage)
        .await
        .expect("failed to load document");

    // Apply edits — these will be logged to the update_log table via the observer
    {
        let awareness = session.awareness.write().await;
        let doc = awareness.doc();
        let text = doc.get_or_insert_text("content");
        text.push(&mut doc.transact_mut(), "crash recovery test");
    }

    // Give the async update log insert a moment to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // DON'T flush — simulate a crash before compaction
    // Drop all sessions without flushing
    manager.unload(doc_id);

    // Verify update log has entries
    let updates = db
        .get_update_log(doc_id)
        .await
        .expect("failed to get update log");
    assert!(
        !updates.is_empty(),
        "update_log should have entries before crash recovery"
    );

    // Reload from snapshot (empty) + update log (should have our edits)
    let manager2 = Arc::new(SessionManager::new());
    let session2 = manager2
        .get_or_load(doc_id, &db, &storage)
        .await
        .expect("failed to reload document");

    // Verify the content was recovered from the update log
    let awareness = session2.awareness.read().await;
    let doc = awareness.doc();
    let text = doc.get_or_insert_text("content");
    let txn = doc.transact();
    let content = text.get_string(&txn);
    assert_eq!(
        content, "crash recovery test",
        "document content should be recovered from update log"
    );

    // Cleanup
    db.clear_update_log(doc_id).await.ok();
    db.delete_document(doc_id).await.ok();
}

#[tokio::test]
async fn test_flush_clears_update_log() {
    let Some((db, storage)) = setup().await else {
        return;
    };

    // Create a document
    let doc_id = Uuid::new_v4();
    db.create_document(doc_id, "Flush Clears Log Test", None)
        .await
        .expect("failed to create document");

    // Load into session
    let manager = Arc::new(SessionManager::new());
    let session = manager
        .get_or_load(doc_id, &db, &storage)
        .await
        .expect("failed to load document");

    // Apply an edit
    {
        let awareness = session.awareness.write().await;
        let doc = awareness.doc();
        let text = doc.get_or_insert_text("content");
        text.push(&mut doc.transact_mut(), "flush test");
    }

    // Wait for update log to be written
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify update log has entries
    let updates = db
        .get_update_log(doc_id)
        .await
        .expect("failed to get update log");
    assert!(!updates.is_empty(), "update_log should have entries before flush");

    // Flush
    session
        .flush(&db, &storage)
        .await
        .expect("flush failed");

    // Verify update log is cleared
    let updates = db
        .get_update_log(doc_id)
        .await
        .expect("failed to get update log");
    assert!(
        updates.is_empty(),
        "update_log should be empty after flush"
    );

    // Cleanup
    db.delete_document(doc_id).await.ok();
}
