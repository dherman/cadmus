# PR 3: WebSocket Comment Events

## Purpose

Broadcast comment mutations to all connected WebSocket clients in real time using y-sync's custom message support. After this PR, when any user creates, edits, resolves, or unresolves a comment via REST, every client with the document open sees the change immediately — no polling or page refresh required.

## Design

### Event Flow

```
User A (browser)           Server                    User B (browser)
     |                        |                           |
     |  POST /comments        |                           |
     |  { body: "..." }       |                           |
     |----------------------->|                           |
     |                        |-- INSERT into DB          |
     |                        |-- Build CommentEvent      |
     |                        |-- Broadcast via           |
     |                        |   BroadcastGroup::        |
     |                        |   send_custom()           |
     |                        |-------------------------->|
     |  201 Created           |   Custom(COMMENT_EVENT,   |
     |<-----------------------|          payload)         |
     |                        |                           |
     |  (also receives the    |                           |
     |   WS event)            |                           |
```

The REST response confirms the mutation to the caller. The WebSocket event notifies all other connected clients (and the caller too — the client deduplicates or uses the WS event as the source of truth).

### Event Types

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommentEvent {
    Created { comment: CommentResponse },
    Updated { comment: CommentResponse },
    Resolved { comment: CommentResponse },
    Unresolved { comment: CommentResponse },
    Replied { comment: CommentResponse },
}
```

Each event carries the full `CommentResponse` (including resolved anchor positions and author info) so the client can update its local state without a follow-up REST call.

### Custom Message Protocol

The y-sync protocol supports custom messages via a tag byte + arbitrary payload. We define:

```rust
const COMMENT_EVENT_TAG: u8 = 100; // Custom message tag for comment events
```

The payload is the JSON-serialized `CommentEvent`, encoded as UTF-8 bytes. On the wire:

```
[CustomMessage type byte] [COMMENT_EVENT_TAG] [length-prefixed JSON payload]
```

### Broadcasting via BroadcastGroup

The `yrs-axum` `BroadcastGroup` provides a mechanism to send custom messages to all connected clients. After each comment REST handler completes its database mutation, it:

1. Builds a `CommentEvent` with the full `CommentResponse`.
2. Serializes it to JSON bytes.
3. Calls `broadcast_group.broadcast_custom(COMMENT_EVENT_TAG, &json_bytes)` (or the equivalent `yrs-axum` API for sending custom messages to the group).

This requires the REST handlers to have access to the document's `BroadcastGroup`. The `SessionManager` already provides access to `DocumentSession`, which holds the `BroadcastGroup`.

### Client-Side Handling

The frontend's WebSocket provider receives custom messages. When a message with `COMMENT_EVENT_TAG` arrives:

1. Parse the JSON payload into a `CommentEvent`.
2. Update the local comment state based on event type:
   - `created` / `replied` → add the comment to the local list
   - `updated` → replace the comment's body in the local list
   - `resolved` / `unresolved` → update the comment's status in the local list

The client-side handling is implemented in PR 4 (`useComments` hook). This PR focuses on the server-side broadcasting infrastructure.

### Initial Comment Load

When a client connects to a document, it fetches the current comment list via `GET /api/docs/{id}/comments`. After that, it stays in sync via WebSocket events. There's no comment-specific sync step in the WebSocket handshake — REST provides the initial state, WebSocket provides the delta stream.

## Server-Side Implementation

### New module: `websocket/events.rs`

```rust
use serde::{Deserialize, Serialize};

pub const COMMENT_EVENT_TAG: u8 = 100;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommentEvent {
    Created { comment: CommentEventPayload },
    Updated { comment: CommentEventPayload },
    Resolved { comment: CommentEventPayload },
    Unresolved { comment: CommentEventPayload },
    Replied { comment: CommentEventPayload },
}

/// Mirrors CommentResponse but is owned by the events module
/// to decouple serialization from the REST response type.
#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct CommentEventAuthor {
    pub id: String,
    pub display_name: String,
    pub email: String,
}
```

### Broadcast helper

```rust
/// Broadcast a comment event to all clients connected to a document.
pub async fn broadcast_comment_event(
    session_manager: &SessionManager,
    document_id: Uuid,
    event: CommentEvent,
) -> Result<(), AppError> {
    if let Some(session) = session_manager.get(document_id) {
        let json = serde_json::to_vec(&event)?;
        session.broadcast_custom(COMMENT_EVENT_TAG, &json);
    }
    // If no active session, no connected clients to notify — skip silently
    Ok(())
}
```

### Integration with REST handlers

Each comment handler (in `api.rs`) calls `broadcast_comment_event` after the database mutation succeeds. Example for `create_comment`:

```rust
// After successful INSERT
let event = CommentEvent::Created {
    comment: response.clone().into(),
};
broadcast_comment_event(&state.session_manager, doc_id, event).await?;
```

## What's Not Included

- Client-side event handling (PR 4 — the `useComments` hook)
- Comment sidebar UI (PR 4)
- Custom message handling in `PermissionedProtocol` for inbound client messages (not needed — comment events are server-to-client only)
