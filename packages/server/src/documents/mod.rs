pub mod api;

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use yrs::{sync::Awareness, Doc};
use yrs_axum::{broadcast::BroadcastGroup, AwarenessRef};

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
}

impl DocumentSession {
    pub async fn new(doc_id: Uuid) -> Arc<Self> {
        let doc = Doc::new();
        let awareness = Arc::new(RwLock::new(Awareness::new(doc)));
        let broadcast_group = Arc::new(BroadcastGroup::new(awareness.clone(), 32).await);
        Arc::new(Self {
            doc_id,
            awareness,
            broadcast_group,
        })
    }
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
        let session = DocumentSession::new(doc_id).await;
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
