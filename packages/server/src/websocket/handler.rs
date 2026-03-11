use axum::{
    extract::{ws::WebSocketUpgrade, Path, State},
    response::IntoResponse,
};
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use yrs_axum::ws::{AxumSink, AxumStream};

use crate::AppState;

/// Handle WebSocket upgrade for real-time document collaboration.
///
/// Flow:
/// 1. Look up or create the document session.
/// 2. Upgrade to WebSocket.
/// 3. Subscribe the connection to the BroadcastGroup.
/// 4. Run the y-sync protocol until the client disconnects.
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Path(doc_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let session = state.document_sessions.get_or_load(doc_id).await;
    ws.on_upgrade(move |socket| handle_ws(socket, session))
}

async fn handle_ws(
    socket: axum::extract::ws::WebSocket,
    session: Arc<crate::documents::DocumentSession>,
) {
    let (sink, stream) = socket.split();
    let sink = Arc::new(Mutex::new(AxumSink(sink)));
    let stream = AxumStream(stream);

    let sub = session.broadcast_group.subscribe(sink, stream);
    match sub.completed().await {
        Ok(_) => tracing::debug!("WebSocket connection closed for document {}", session.doc_id),
        Err(e) => tracing::warn!(
            "WebSocket connection error for document {}: {}",
            session.doc_id,
            e
        ),
    }
}
