# PR 2: Comment Anchoring

## Purpose

Implement the server-side logic to convert character offsets to Yjs RelativePositions when a comment is created, store them as binary data, and resolve them back to absolute offsets when listing comments. This is the critical piece that makes comment anchors survive concurrent edits — a comment attached to paragraph 3 stays on paragraph 3 even if someone inserts content above it.

## How Yjs RelativePositions Work

A Yjs `RelativePosition` identifies a position in a Y.Doc by referencing the ID of an adjacent item (the internal CRDT element) rather than an absolute character offset. Because CRDT item IDs are immutable, a RelativePosition remains valid even as the document changes around it. To use a RelativePosition:

1. **Create** it from an absolute offset + the current Y.Doc state → produces a binary blob.
2. **Store** the binary blob in the database.
3. **Resolve** it later against the (possibly changed) Y.Doc state → produces a new absolute offset.

The `yrs` crate provides `RelativePosition::from(...)` to create and `RelativePosition::to_absolute(...)` to resolve.

## Anchor Conversion Flow

### On comment creation

```
Client                          Server
  |                               |
  |  POST /api/docs/{id}/comments |
  |  { anchor_from: 142,         |
  |    anchor_to: 189,           |
  |    body: "..." }             |
  |------------------------------>|
  |                               |-- Load Yrs Doc from SessionManager
  |                               |-- Convert offset 142 → RelativePosition (binary)
  |                               |-- Convert offset 189 → RelativePosition (binary)
  |                               |-- INSERT comment with binary anchors
  |<------------------------------|
  |  { id: "...", anchor_from: 142, anchor_to: 189, ... }
```

The server uses the live Yrs document to create RelativePositions. The character offsets refer to the document's text content as a flat string (the same positions ProseMirror reports for selections).

### On comment listing

```
Client                          Server
  |                               |
  |  GET /api/docs/{id}/comments  |
  |------------------------------>|
  |                               |-- Load Yrs Doc from SessionManager
  |                               |-- For each comment with anchors:
  |                               |   Resolve RelativePosition → absolute offset
  |                               |-- Return comments with resolved offsets
  |<------------------------------|
  |  [{ id: "...", anchor_from: 142, anchor_to: 190, ... }]
```

The resolved offsets may differ from the original offsets if the document has been edited since the comment was created. This is the whole point — the positions track through edits.

## ProseMirror Position → Yrs Position Mapping

The frontend provides ProseMirror character positions (from the editor's selection). These need to be mapped to Yrs text positions. The Yrs document stores content as an XML fragment that mirrors ProseMirror's node tree. The mapping traverses the Yrs XML structure to find the text offset that corresponds to the ProseMirror position.

This mapping function lives in the server's Yrs integration code:

```rust
/// Convert a ProseMirror character offset to a Yrs text offset
/// within the document's XmlFragment.
fn pm_offset_to_yrs_position(
    txn: &TransactionMut,
    xml_fragment: &XmlFragmentRef,
    pm_offset: u32,
) -> Option<RelativePosition> {
    // Walk the XML tree, counting text positions until we reach pm_offset
    // Then create a RelativePosition at that point
}

/// Convert a Yrs RelativePosition back to a ProseMirror character offset.
fn yrs_position_to_pm_offset(
    txn: &Transaction,
    xml_fragment: &XmlFragmentRef,
    rel_pos: &RelativePosition,
) -> Option<u32> {
    // Resolve the RelativePosition to an absolute Yrs offset
    // Walk the XML tree to convert back to ProseMirror position
}
```

## Orphaned Anchors

If the text that a comment was anchored to has been entirely deleted, the RelativePosition resolves to the nearest valid position (typically the position where the deleted content used to be). The comment isn't lost — it just points to a potentially unexpected location. The frontend should handle this gracefully (e.g., by showing the comment in the sidebar without a highlight if the anchor range is zero-width or invalid).

## Changes to Comment Response

The `CommentResponse` struct gains anchor fields:

```rust
#[derive(Debug, Serialize)]
pub struct CommentResponse {
    pub id: Uuid,
    pub document_id: Uuid,
    pub author: CommentAuthor,
    pub parent_id: Option<Uuid>,
    pub anchor_from: Option<u32>,  // resolved absolute offset
    pub anchor_to: Option<u32>,    // resolved absolute offset
    pub body: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

Replies (which have NULL anchors) return `null` for `anchor_from`/`anchor_to`.

## Changes to Create Handler

The `create_comment` handler now:

1. Reads `anchor_from` and `anchor_to` from the request body.
2. If both are present, obtains the Yrs document from the SessionManager.
3. Converts both offsets to `RelativePosition`s using the live document state.
4. Stores the binary `RelativePosition`s in the `anchor_start` and `anchor_end` columns.
5. Returns the original offsets in the response (since we just created them, they match the current doc state).

If the document session is not active (no connected clients), the server loads the Yrs document from S3 to perform the conversion. This ensures comments can be created via REST even when no one has the document open in the editor.

## Changes to List Handler

The `list_comments` handler now:

1. Obtains the Yrs document from the SessionManager (or loads from S3 if not active).
2. For each comment with non-NULL anchors, resolves the `RelativePosition`s to absolute offsets.
3. Includes `anchor_from` and `anchor_to` in the response.

## What's Not Included

- Frontend anchor highlighting (PR 4)
- WebSocket broadcasting of anchor positions (PR 3 handles event structure)
- Anchor conversion for agent-provided offsets with `base_version` (deferred — agents use the same character offsets as the frontend for now)
