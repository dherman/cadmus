pub mod api;

use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;
use yrs::Doc;

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
    pub doc: tokio::sync::RwLock<Doc>,
    // TODO: Add BroadcastGroup from yrs-axum
    // TODO: Add Awareness
    // TODO: Add flush state tracking (last flush time, pending update count)
    // TODO: Add connected client count
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    /// Get or create a session for a document.
    /// If the session doesn't exist, loads the document from storage.
    pub async fn get_or_load(&self, doc_id: Uuid) -> Arc<DocumentSession> {
        if let Some(session) = self.sessions.get(&doc_id) {
            return session.clone();
        }

        // TODO: Load document state from S3 + update log
        // For now, create an empty document
        let doc = Doc::new();

        let session = Arc::new(DocumentSession {
            doc_id,
            doc: tokio::sync::RwLock::new(doc),
        });

        self.sessions.insert(doc_id, session.clone());
        session
    }

    /// Flush a document's current state to persistent storage.
    pub async fn flush(&self, _doc_id: Uuid) {
        // TODO: Compact the Yrs doc, write snapshot to S3,
        // append updates to the update log in Postgres
    }

    /// Unload a document session from memory.
    /// Should only be called after flushing and after the grace period.
    pub async fn unload(&self, doc_id: Uuid) {
        self.sessions.remove(&doc_id);
    }
}
