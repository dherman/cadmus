# PR 3: WebSocket Comment Events — Implementation Plan

## Prerequisites

- [x] PR 1 (Comments Table & CRUD API) is merged

## Steps

### 1. Create the events module

- [x] Create `packages/server/src/websocket/events.rs` with:
  - `COMMENT_EVENT_TAG` constant (`u8 = 100`)
  - `CommentEvent` enum (Created, Updated, Resolved, Unresolved, Replied) with serde tag serialization
  - `CommentEventPayload` and `CommentEventAuthor` structs
  - `broadcast_comment_event` async function

- [x] Add `pub mod events;` to `packages/server/src/websocket/mod.rs`.

### 2. Add custom message broadcasting to DocumentSession

- [x] Verify that the `yrs-axum` `BroadcastGroup` supports sending custom messages. Check the `yrs-axum` API for the correct method — likely `broadcast_custom` or a method on the `BroadcastGroup` that accepts a custom tag and payload bytes.

- [x] If `BroadcastGroup` doesn't directly expose custom message broadcasting, add a helper method to `DocumentSession` that encodes a custom message in the y-sync wire format and sends it through the group's sink. The y-sync custom message format is:

  ```
  [MSG_CUSTOM tag byte] [custom_tag: u8] [payload: bytes]
  ```

- [x] Add a `broadcast_custom(tag: u8, payload: &[u8])` method to `DocumentSession`:

```rust
impl DocumentSession {
    pub fn broadcast_custom(&self, tag: u8, payload: &[u8]) {
        // Encode as y-sync custom message and broadcast
        // to all connected clients via the BroadcastGroup
    }
}
```

### 3. Wire broadcasting into comment REST handlers

- [x] Modify each comment handler in `packages/server/src/documents/api.rs` to broadcast after mutation:

**`create_comment`:**

```rust
// After successful DB insert and response construction
let event = CommentEvent::Created { comment: payload };
broadcast_comment_event(&state.session_manager, doc_id, event).await.ok();
```

**`reply_to_comment`:**

```rust
let event = CommentEvent::Replied { comment: payload };
broadcast_comment_event(&state.session_manager, doc_id, event).await.ok();
```

**`edit_comment`:**

```rust
let event = CommentEvent::Updated { comment: payload };
broadcast_comment_event(&state.session_manager, doc_id, event).await.ok();
```

**`resolve_comment`:**

```rust
let event = CommentEvent::Resolved { comment: payload };
broadcast_comment_event(&state.session_manager, doc_id, event).await.ok();
```

**`unresolve_comment`:**

```rust
let event = CommentEvent::Unresolved { comment: payload };
broadcast_comment_event(&state.session_manager, doc_id, event).await.ok();
```

Note: broadcasting failures are logged but don't fail the REST response (`.ok()` swallows errors). The REST mutation is the source of truth; the WebSocket event is best-effort notification.

### 4. Add a From impl for CommentResponse → CommentEventPayload

- [x] Implement `From<CommentResponse>` for `CommentEventPayload` to avoid manual field mapping in every handler:

```rust
impl From<CommentResponse> for CommentEventPayload {
    fn from(r: CommentResponse) -> Self {
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
```

### 5. Test broadcasting

Manual testing replaced by automated integration tests in `packages/server/tests/websocket_test.rs`:

- [x] `comment_create_broadcasts_event` — connects a WS client, creates a comment via REST, decodes the y-sync `Custom(100, ...)` frame, asserts the JSON payload
- [x] `comment_reply_broadcasts_event` — creates a parent comment, replies, verifies `Replied` event with correct `parent_id`
- [x] `comment_edit_broadcasts_event` — edits a comment, verifies `Updated` event with new body
- [x] `comment_resolve_unresolve_broadcasts_events` — resolves then unresolves, verifies both `Resolved` and `Unresolved` events
- [x] `comment_no_broadcast_when_no_ws_clients` — creates a comment with no WS connections, verifies the REST response succeeds (no errors)

### 6. Build and verify

- [x] Run `cargo build` — compiles without errors.
- [x] Run `cargo test` — all tests pass.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [x] Creating a comment broadcasts a `created` event to all connected WS clients
- [x] Replying to a comment broadcasts a `replied` event
- [x] Editing a comment broadcasts an `updated` event
- [x] Resolving a comment broadcasts a `resolved` event
- [x] Unresolving a comment broadcasts an `unresolved` event
- [x] Event payloads contain the full comment data (author info, anchors, body, status)
- [x] Broadcasting failure doesn't fail the REST response
- [x] No events are sent when no clients are connected (no-op, no errors)
- [x] Events are valid JSON that can be parsed client-side

## Files Modified

| File                                      | Change                                           |
| ----------------------------------------- | ------------------------------------------------ |
| `packages/server/src/websocket/events.rs` | New: event types, broadcast helper               |
| `packages/server/src/websocket/mod.rs`    | Add `pub mod events`                             |
| `packages/server/src/documents/api.rs`    | Add broadcast calls to all comment handlers      |
| `packages/server/src/documents/mod.rs`    | Minor: may need to expose DocumentSession method |
