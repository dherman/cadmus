# Comments

## Design Decision: REST + WebSocket Notifications (not CRDT)

Comments don't need CRDT replication. Each comment is owned by a single author — there's no concurrent editing of the same comment body. Joint actions (resolve/unresolve) have last-write-wins semantics, which is fine. The complexity of a second Yjs document for comments isn't justified.

Instead, comments are managed via REST API with real-time visibility provided by WebSocket notifications using y-sync's custom message support.

## Data Model

```sql
CREATE TABLE comments (
  id              UUID PRIMARY KEY,
  document_id     UUID NOT NULL REFERENCES documents(id),
  author_id       UUID NOT NULL REFERENCES users(id),
  parent_id       UUID REFERENCES comments(id),  -- for threading
  anchor_start    BYTEA,                          -- Yjs RelativePosition (binary)
  anchor_end      BYTEA,                          -- Yjs RelativePosition (binary)
  body            TEXT NOT NULL,
  status          VARCHAR(20) DEFAULT 'open',     -- open, resolved
  created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Anchoring Strategy

Comment anchors use Yjs `RelativePosition`s, which track a position through concurrent edits. This means a comment on paragraph 3 stays attached to paragraph 3 even if someone inserts content above it.

The REST API accepts integer character offsets (which are what agents and the frontend naturally have). The server converts these to `RelativePosition`s at the moment of comment creation, using the current Yrs document state. The `base_version` in the creation request ensures correct position mapping even if the document has changed slightly between the agent reading it and creating the comment.

## Real-Time Notifications

When a comment is created, updated, resolved, or deleted:

1. The REST handler performs the database mutation.
2. The handler broadcasts a `Custom(COMMENT_EVENT_TAG, payload)` message to all clients connected to that document's WebSocket `BroadcastGroup`.
3. Connected clients receive the event and update their local comment state.

Payload format:

```json
{
  "type": "created | updated | resolved | unresolve | deleted",
  "comment": { "id": "...", "author": {...}, "body": "...", ... }
}
```

## Permissions

- **Read** role: can see comments, cannot create.
- **Comment** role: can create comments, reply, edit own comments, resolve/unresolve.
- **Edit** role: same as Comment for comment operations.

Only a comment's author can edit its body. Any user with Comment or Edit permission can resolve/unresolve.

## Offline Behavior

Comments require an active server connection (REST call). Offline comment creation is not supported in the prototype. If needed later, client-side optimistic state with a retry queue can be added without changing the server architecture.

## Future: Comment Curation

The data model supports future features for making comment management more productive for document owners: filtering by status/author, bulk resolve, AI-assisted triage, comment tagging. These are UI and API additions on top of the existing model, not schema changes.
