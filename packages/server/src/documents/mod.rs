pub mod anchors;
pub mod api;
pub mod comments;
pub mod permissions;
pub mod storage;
pub mod yrs_json;

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, RwLock};
use uuid::Uuid;
use yrs::updates::decoder::Decode;
use yrs::{sync::Awareness, Doc, ReadTxn, Transact, Update};
use yrs_axum::{broadcast::BroadcastGroup, AwarenessRef};

use crate::db::Database;
use crate::errors::AppError;
use storage::SnapshotStorage;

/// Manages in-memory document sessions.
///
/// Each active document has a session that holds the Yrs Doc, broadcast group,
/// and metadata. Sessions are loaded on first client connection and unloaded
/// after all clients disconnect (with a grace period).
pub struct SessionManager {
    sessions: DashMap<Uuid, Arc<DocumentSession>>,
}

/// An active document editing session.
pub struct DocumentSession {
    pub doc_id: Uuid,
    pub awareness: AwarenessRef,
    pub broadcast_group: Arc<BroadcastGroup>,
    flush_notify: Notify,
    update_count: AtomicU64,
    connection_count: AtomicU64,
}

impl DocumentSession {
    /// Create a new session with an empty Yrs Doc.
    pub async fn new(doc_id: Uuid) -> Arc<Self> {
        let doc = Doc::new();
        Self::new_with_doc(doc_id, doc).await
    }

    /// Create a new session with an existing Yrs Doc (used when loading from storage).
    pub async fn new_with_doc(doc_id: Uuid, doc: Doc) -> Arc<Self> {
        let awareness = Arc::new(RwLock::new(Awareness::new(doc)));
        let broadcast_group = Arc::new(BroadcastGroup::new(awareness.clone(), 32).await);
        Arc::new(Self {
            doc_id,
            awareness,
            broadcast_group,
            flush_notify: Notify::new(),
            update_count: AtomicU64::new(0),
            connection_count: AtomicU64::new(0),
        })
    }

    /// Start observing Yrs updates and appending them to the database update log.
    ///
    /// Must be called immediately after construction, before the session is
    /// shared with other tasks, so the `try_read()` is guaranteed to succeed.
    pub fn start_update_logging(&self, db: Database) {
        let doc_id = self.doc_id;
        let flush_notify = &self.flush_notify as *const Notify;
        let update_count = &self.update_count as *const AtomicU64;

        // try_read() is safe here because start_update_logging is called right
        // after session construction, before the session is shared — no other
        // task can hold the lock yet.
        let awareness = self
            .awareness
            .try_read()
            .expect("start_update_logging must be called before the session is shared");
        let sub = awareness.doc().observe_update_v1(move |_txn, event| {
            let update_data = event.update.to_vec();
            let db = db.clone();

            // Increment update counter and notify flush loop if threshold reached
            let count = unsafe { &*update_count }.fetch_add(1, Ordering::Relaxed) + 1;
            if count >= 100 {
                unsafe { &*flush_notify }.notify_one();
            }

            tokio::spawn(async move {
                if let Err(e) = db.append_update_log(doc_id, &update_data).await {
                    tracing::error!("Failed to log update for doc {}: {}", doc_id, e);
                }
            });
        });
        // Leak the subscription so it lives as long as the Doc
        if let Ok(sub) = sub {
            std::mem::forget(sub);
        }
    }

    /// Flush the current document state to S3 and clear the update log.
    pub async fn flush(&self, db: &Database, storage: &SnapshotStorage) -> Result<(), AppError> {
        // Encode the full doc state. The transaction and awareness must be
        // dropped before any .await because yrs::Transaction is !Send.
        let state = {
            let awareness = self.awareness.read().await;
            let doc = awareness.doc();
            let txn = doc.transact();
            txn.encode_state_as_update_v1(&yrs::StateVector::default())
        };

        // Upload snapshot to S3
        let key = storage.upload_snapshot(self.doc_id, &state).await?;

        // Update the document row with the new snapshot key
        db.update_document_snapshot(self.doc_id, &key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to update snapshot key: {}", e)))?;

        // Clear the update log (now redundant)
        db.clear_update_log(self.doc_id)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to clear update log: {}", e)))?;

        // Reset update counter
        self.update_count.store(0, Ordering::Relaxed);

        tracing::info!("Flushed document {} to S3", self.doc_id);
        Ok(())
    }

    /// Increment the connection counter.
    pub fn add_connection(&self) {
        self.connection_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the connection counter. Returns the new count.
    pub fn remove_connection(&self) -> u64 {
        self.connection_count.fetch_sub(1, Ordering::Relaxed) - 1
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    /// Get a session if it is already loaded in memory (no I/O).
    pub fn get(&self, doc_id: Uuid) -> Option<Arc<DocumentSession>> {
        self.sessions.get(&doc_id).map(|r| r.clone())
    }

    /// Insert a pre-built session into the cache (for testing).
    pub fn preload(&self, doc_id: Uuid, session: Arc<DocumentSession>) {
        self.sessions.insert(doc_id, session);
    }

    /// Get or load a session for a document from persistent storage.
    ///
    /// If the session is already in memory, returns it. Otherwise, loads the
    /// document from Postgres metadata + S3 snapshot + update log replay.
    pub async fn get_or_load(
        &self,
        doc_id: Uuid,
        db: &Database,
        storage: &SnapshotStorage,
    ) -> Result<Arc<DocumentSession>, AppError> {
        if let Some(session) = self.sessions.get(&doc_id) {
            return Ok(session.clone());
        }

        // Load document metadata from Postgres
        let doc_row = db
            .get_document(doc_id)
            .await
            .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

        // Create a new Yrs Doc
        let doc = Doc::new();

        // If a snapshot exists, load it
        if let Some(ref key) = doc_row.snapshot_key {
            if let Some(snapshot_data) = storage.download_snapshot(key).await? {
                let update = Update::decode_v1(&snapshot_data)
                    .map_err(|e| AppError::Internal(format!("Failed to decode snapshot: {}", e)))?;
                doc.transact_mut().apply_update(update);
            }
        }

        // Replay any update log entries (crash recovery)
        let updates = db
            .get_update_log(doc_id)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get update log: {}", e)))?;
        for update_data in updates {
            let update = Update::decode_v1(&update_data)
                .map_err(|e| AppError::Internal(format!("Failed to decode update: {}", e)))?;
            doc.transact_mut().apply_update(update);
        }

        // Create session
        let session = DocumentSession::new_with_doc(doc_id, doc).await;

        // Start update logging
        session.start_update_logging(db.clone());

        // Spawn flush loop
        Self::spawn_flush_loop(session.clone(), db.clone(), storage.clone());

        // Insert into cache
        self.sessions.insert(doc_id, session.clone());
        Ok(session)
    }

    /// Spawn a background task that flushes the document periodically.
    fn spawn_flush_loop(session: Arc<DocumentSession>, db: Database, storage: SnapshotStorage) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {
                        // Inactivity timeout — flush if there are pending updates
                        if session.update_count.load(Ordering::Relaxed) > 0 {
                            if let Err(e) = session.flush(&db, &storage).await {
                                tracing::error!("Flush failed for doc {}: {}", session.doc_id, e);
                            }
                        }
                    }
                    _ = session.flush_notify.notified() => {
                        // Update count threshold reached
                        if let Err(e) = session.flush(&db, &storage).await {
                            tracing::error!("Flush failed for doc {}: {}", session.doc_id, e);
                        }
                    }
                }
            }
        });
    }

    /// Start the unload timer for a document after the last client disconnects.
    pub fn start_unload_timer(
        self: &Arc<Self>,
        session: Arc<DocumentSession>,
        db: Database,
        storage: SnapshotStorage,
    ) {
        let manager = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;

            // Check if anyone reconnected during the grace period
            if session.connection_count.load(Ordering::Relaxed) == 0 {
                // Final flush
                if let Err(e) = session.flush(&db, &storage).await {
                    tracing::error!("Final flush failed for doc {}: {}", session.doc_id, e);
                }
                // Remove from memory
                manager.unload(session.doc_id);
                tracing::info!("Unloaded document {} from memory", session.doc_id);
            }
        });
    }

    /// Remove a document session from memory.
    pub fn unload(&self, doc_id: Uuid) {
        self.sessions.remove(&doc_id);
    }
}
