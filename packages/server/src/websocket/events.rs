use serde::{Deserialize, Serialize};
use uuid::Uuid;
use yrs::sync::Message;
use yrs::updates::encoder::Encode;

use crate::documents::{DocumentSession, SessionManager};

/// Custom message tag for comment events (must not collide with y-sync's
/// built-in tags: 0 = sync, 1 = awareness-query, 2 = auth, 3 = awareness).
pub const COMMENT_EVENT_TAG: u8 = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommentEvent {
    Created { comment: CommentEventPayload },
    Updated { comment: CommentEventPayload },
    Resolved { comment: CommentEventPayload },
    Unresolved { comment: CommentEventPayload },
    Replied { comment: CommentEventPayload },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentEventPayload {
    pub id: String,
    pub document_id: String,
    pub author: CommentEventAuthor,
    pub parent_id: Option<String>,
    pub anchor_from: Option<u32>,
    pub anchor_to: Option<u32>,
    pub body: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentEventAuthor {
    pub id: String,
    pub display_name: String,
    pub email: String,
}

impl From<crate::documents::comments::CommentResponse> for CommentEventPayload {
    fn from(r: crate::documents::comments::CommentResponse) -> Self {
        CommentEventPayload {
            id: r.id.to_string(),
            document_id: r.document_id.to_string(),
            author: CommentEventAuthor {
                id: r.author.id.to_string(),
                display_name: r.author.display_name,
                email: r.author.email,
            },
            parent_id: r.parent_id.map(|id| id.to_string()),
            anchor_from: r.anchor_from,
            anchor_to: r.anchor_to,
            body: r.body,
            status: r.status,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

impl DocumentSession {
    /// Broadcast a custom message to all connected WebSocket clients.
    ///
    /// Encodes the payload as a y-sync `Message::Custom` and sends it through
    /// the `BroadcastGroup`'s channel, which delivers it to every subscriber.
    pub fn broadcast_custom(&self, tag: u8, payload: &[u8]) {
        let msg = Message::Custom(tag, payload.to_vec());
        let encoded = msg.encode_v1();
        // broadcast() returns Err only when there are no receivers, which is fine
        let _ = self.broadcast_group.broadcast(encoded);
    }
}

/// Broadcast a comment event to all clients connected to a document.
///
/// If no session is active (no clients connected), this is a no-op.
/// Broadcasting failures are logged but do not propagate errors — the REST
/// mutation is the source of truth; the WebSocket event is best-effort.
pub async fn broadcast_comment_event(
    session_manager: &SessionManager,
    document_id: Uuid,
    event: CommentEvent,
) {
    if let Some(session) = session_manager.get(document_id) {
        match serde_json::to_vec(&event) {
            Ok(json) => session.broadcast_custom(COMMENT_EVENT_TAG, &json),
            Err(e) => tracing::error!("Failed to serialize comment event: {}", e),
        }
    }
}
