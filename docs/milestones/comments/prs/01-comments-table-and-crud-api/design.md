# PR 1: Comments Table & CRUD API

## Purpose

Create the `comments` table, implement the comment data structures in Rust, and build the REST endpoints for the full comment lifecycle: list, create, reply, edit own, resolve, and unresolve. This PR replaces the existing stub endpoints with real implementations and establishes the data layer that all subsequent PRs build on.

## Database Schema

```sql
CREATE TABLE comments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id     UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    author_id       UUID NOT NULL REFERENCES users(id),
    parent_id       UUID REFERENCES comments(id),
    anchor_start    BYTEA,
    anchor_end      BYTEA,
    body            TEXT NOT NULL,
    status          VARCHAR(20) NOT NULL DEFAULT 'open',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_comments_document_id ON comments(document_id);
CREATE INDEX idx_comments_parent_id ON comments(parent_id);
CREATE INDEX idx_comments_document_status ON comments(document_id, status);
```

- `parent_id` is NULL for top-level comments, set to a top-level comment's ID for replies.
- `anchor_start` and `anchor_end` store Yjs `RelativePosition` bytes. They are NULL for replies (replies inherit the parent's anchor). In this PR, anchors are stored as raw bytes but not yet converted from character offsets — that conversion logic comes in PR 2. For now, anchor fields are nullable and may be left NULL.
- `status` is either `'open'` or `'resolved'`. Only top-level comments have meaningful status; replies inherit the parent's status.

## Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CommentRow {
    pub id: Uuid,
    pub document_id: Uuid,
    pub author_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub anchor_start: Option<Vec<u8>>,
    pub anchor_end: Option<Vec<u8>>,
    pub body: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

API response type includes the author's display name and email (joined from `users`):

```rust
#[derive(Debug, Serialize)]
pub struct CommentResponse {
    pub id: Uuid,
    pub document_id: Uuid,
    pub author: CommentAuthor,
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // anchor_from / anchor_to added in PR 2 once resolution logic exists
}

#[derive(Debug, Serialize)]
pub struct CommentAuthor {
    pub id: Uuid,
    pub display_name: String,
    pub email: String,
}
```

## REST Endpoints

### List comments

```
GET /api/docs/{id}/comments?status=open|resolved|all
```

Returns all comments for the document, filtered by status (default: `all`). Comments are returned in a flat list sorted by `created_at` ascending. The frontend groups them into threads by `parent_id`.

**Permission:** Read or higher.

### Create comment

```
POST /api/docs/{id}/comments
{
    "body": "This needs clarification",
    "anchor_from": 142,        // optional — character offset (PR 2 converts to RelativePosition)
    "anchor_to": 189,          // optional — character offset
    "base_version": "v_abc"    // optional — for anchor conversion (PR 2)
}
```

Creates a top-level comment. In this PR, `anchor_from`/`anchor_to`/`base_version` are accepted but the anchor conversion is deferred to PR 2 — the comment is created with NULL anchors.

**Permission:** Comment or Edit.

### Reply to comment

```
POST /api/docs/{id}/comments/{comment_id}/replies
{
    "body": "Good point, I'll fix this"
}
```

Creates a reply to an existing top-level comment. Sets `parent_id` to the referenced comment. Replies cannot have anchors (they inherit the parent's) and cannot themselves be replied to (single-level threading only).

**Permission:** Comment or Edit.
**Validation:** The parent comment must exist and belong to the same document. The parent must be a top-level comment (its own `parent_id` must be NULL).

### Edit comment

```
PUT /api/docs/{id}/comments/{comment_id}
{
    "body": "Updated text"
}
```

Updates the body of a comment. Only the comment's author can edit it.

**Permission:** Comment or Edit, AND must be the comment's author.

### Resolve thread

```
POST /api/docs/{id}/comments/{comment_id}/resolve
```

Sets the comment's status to `'resolved'`. Only applies to top-level comments. Any Comment/Edit user can resolve.

**Permission:** Comment or Edit.
**Validation:** The comment must be a top-level comment (not a reply).

### Unresolve thread

```
POST /api/docs/{id}/comments/{comment_id}/unresolve
```

Sets the comment's status back to `'open'`. Only applies to top-level comments.

**Permission:** Comment or Edit.

## Database Query Methods

Add to `db.rs`:

- `list_comments(document_id, status_filter) -> Vec<CommentWithAuthor>` — joins `comments` with `users` on `author_id`
- `create_comment(document_id, author_id, body, anchor_start, anchor_end) -> CommentRow`
- `create_reply(document_id, author_id, parent_id, body) -> CommentRow`
- `get_comment(comment_id) -> Option<CommentRow>`
- `update_comment_body(comment_id, body) -> CommentRow`
- `update_comment_status(comment_id, status) -> CommentRow`

## Router Changes

Replace the stub routes in `lib.rs` with:

```rust
.route("/api/docs/{id}/comments", get(list_comments).post(create_comment))
.route("/api/docs/{id}/comments/{comment_id}", put(edit_comment))
.route("/api/docs/{id}/comments/{comment_id}/replies", post(reply_to_comment))
.route("/api/docs/{id}/comments/{comment_id}/resolve", post(resolve_comment))
.route("/api/docs/{id}/comments/{comment_id}/unresolve", post(unresolve_comment))
```

## Error Cases

| Scenario                                  | Response        |
| ----------------------------------------- | --------------- |
| User lacks Comment/Edit permission        | 403 Forbidden   |
| Document not found                        | 404 Not Found   |
| Comment not found                         | 404 Not Found   |
| Reply to a non-top-level comment          | 400 Bad Request |
| Edit by non-author                        | 403 Forbidden   |
| Resolve/unresolve a reply (not top-level) | 400 Bad Request |
| Empty body                                | 400 Bad Request |

## What's Not Included

- Anchor conversion from character offsets to Yjs RelativePositions (PR 2)
- Resolved anchor positions in list response (PR 2)
- WebSocket event broadcasting on comment mutations (PR 3)
- Frontend UI (PR 4, PR 5)
- Comment deletion
