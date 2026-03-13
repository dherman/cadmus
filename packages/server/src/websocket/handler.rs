use axum::{
    extract::{ws::WebSocketUpgrade, Path, State},
    response::IntoResponse,
};
use axum::extract::ws::Message;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use yrs::sync::Error;
use yrs_axum::ws::AxumSink;

use crate::errors::AppError;
use crate::AppState;

/// Handle WebSocket upgrade for real-time document collaboration.
///
/// Flow:
/// 1. Look up or load the document session from persistent storage.
/// 2. Upgrade to WebSocket.
/// 3. Subscribe the connection to the BroadcastGroup.
/// 4. Run the y-sync protocol until the client disconnects.
/// 5. Track connection count for unload lifecycle.
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Path(doc_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
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
        handle_ws(socket, session, document_sessions, db, storage)
    }))
}

async fn handle_ws(
    socket: axum::extract::ws::WebSocket,
    session: Arc<crate::documents::DocumentSession>,
    session_manager: Arc<crate::documents::SessionManager>,
    db: crate::db::Database,
    storage: crate::documents::storage::SnapshotStorage,
) {
    tracing::info!("WebSocket connection opened for document {}", session.doc_id);
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

    let sub = session.broadcast_group.subscribe(sink, binary_stream);
    match sub.completed().await {
        Ok(_) => tracing::info!("WebSocket connection closed for document {}", session.doc_id),
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
