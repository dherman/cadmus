use axum::extract::ws::Message;
use axum::{
    extract::{ws::WebSocketUpgrade, Path, Query, State},
    response::IntoResponse,
};
use futures_util::StreamExt;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use yrs::sync::Error;
use yrs_axum::ws::AxumSink;

use crate::documents::permissions::{require_permission, Permission};
use crate::errors::AppError;
use crate::AppState;

use super::protocol::PermissionedProtocol;

#[derive(Deserialize)]
pub struct WsQueryParams {
    pub token: String,
}

/// Handle WebSocket upgrade for real-time document collaboration.
///
/// Flow:
/// 1. Validate the ws-token from the query parameter.
/// 2. Check the user's permission on the document.
/// 3. Look up or load the document session from persistent storage.
/// 4. Upgrade to WebSocket.
/// 5. Subscribe the connection to the BroadcastGroup with permission-gated protocol.
/// 6. Track connection count for unload lifecycle.
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Path(doc_id): Path<Uuid>,
    Query(params): Query<WsQueryParams>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    // Validate ws-token
    let claims = crate::auth::jwt::validate_token(&params.token, "ws", &state.config.jwt_secret)?;
    let user_id =
        Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized("Invalid token".into()))?;

    // Check document permission
    let permission = require_permission(&state.db, user_id, doc_id, Permission::Read).await?;

    // get_or_load checks the in-memory cache first, then falls back to
    // loading from the database + S3. If the document doesn't exist in either
    // place, it returns a 404.
    let session = state
        .document_sessions
        .get_or_load(doc_id, &state.db, &state.storage)
        .await?;

    // Track connection
    session.add_connection();

    let document_sessions = state.document_sessions.clone();
    let db = state.db.clone();
    let storage = state.storage.clone();

    Ok(ws.on_upgrade(move |socket| {
        handle_ws(socket, session, permission, document_sessions, db, storage)
    }))
}

async fn handle_ws(
    socket: axum::extract::ws::WebSocket,
    session: Arc<crate::documents::DocumentSession>,
    permission: Permission,
    session_manager: Arc<crate::documents::SessionManager>,
    db: crate::db::Database,
    storage: crate::documents::storage::SnapshotStorage,
) {
    tracing::info!(
        "WebSocket connection opened for document {}",
        session.doc_id
    );
    let (sink, stream) = socket.split();
    let sink = Arc::new(Mutex::new(AxumSink(sink)));

    // Filter the stream to only pass binary frames to the y-sync protocol.
    // Ping/pong/close/text frames are not y-sync messages and will cause
    // deserialization errors if passed through.
    let binary_stream = stream.filter_map(|msg| {
        let result = match msg {
            Ok(Message::Binary(data)) => Some(Ok(data.to_vec())),
            Ok(Message::Close(_)) => None,
            Ok(_) => None, // skip ping, pong, text frames
            Err(e) => Some(Err(Error::Other(e.into()))),
        };
        std::future::ready(result)
    });
    // Pin the stream so it satisfies Unpin
    let binary_stream = Box::pin(binary_stream);

    let protocol = PermissionedProtocol { permission };
    let sub = session
        .broadcast_group
        .subscribe_with(sink, binary_stream, protocol);
    match sub.completed().await {
        Ok(_) => tracing::info!(
            "WebSocket connection closed for document {}",
            session.doc_id
        ),
        Err(e) => tracing::warn!(
            "WebSocket connection error for document {}: {}",
            session.doc_id,
            e
        ),
    }

    // Track disconnection
    let remaining = session.remove_connection();
    if remaining == 0 {
        tracing::info!(
            "Last client disconnected from document {}, starting unload timer",
            session.doc_id
        );
        session_manager.start_unload_timer(session, db, storage);
    }
}
