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
}
