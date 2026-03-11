use axum::{
    extract::{ws::WebSocketUpgrade, Path, Query, State},
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: String,
}

/// Handle WebSocket upgrade for real-time document collaboration.
///
/// Flow:
/// 1. Validate the JWT token from the query parameter.
/// 2. Determine user identity and permission level.
/// 3. Load or join the document session.
/// 4. Upgrade to WebSocket.
/// 5. Run the y-sync protocol with permission enforcement.
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Path(doc_id): Path<Uuid>,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // TODO: Validate JWT from query.token
    // TODO: Look up user permissions for this document
    // TODO: Load document session via state.document_sessions.get_or_load(doc_id)

    ws.on_upgrade(move |socket| handle_ws(socket, doc_id, state))
}

async fn handle_ws(
    _socket: axum::extract::ws::WebSocket,
    _doc_id: Uuid,
    _state: Arc<AppState>,
) {
    // TODO: Split socket into sink/stream
    // TODO: Create PermissionedProtocol based on user's role
    // TODO: Subscribe to BroadcastGroup with the custom protocol
    // TODO: Run y-sync message loop:
    //   - Incoming messages: decode y-sync, check permissions, apply to Yrs doc
    //   - Outgoing messages: broadcast updates from other clients
    //   - Handle awareness updates
    //   - Handle custom messages (comment events)
    // TODO: On disconnect, decrement client count, start unload timer if last client
    tracing::info!("WebSocket connection established for document {}", _doc_id);
}
