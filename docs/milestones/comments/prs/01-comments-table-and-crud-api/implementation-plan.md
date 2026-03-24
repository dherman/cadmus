# PR 1: Comments Table & CRUD API — Implementation Plan

## Prerequisites

- [ ] Milestone 4 (Node Sidecar and Markdown Export) is merged

## Steps

### 1. Create the comments migration

- [ ] Create `packages/server/migrations/20260312000003_comments.sql`:

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

- [ ] Start the dev database (`pnpm dev:infra`) and verify the migration runs on server startup.

### 2. Add comment data structures

- [ ] Create `packages/server/src/documents/comments.rs` with:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}

#[derive(Debug, Serialize)]
pub struct CommentAuthor {
    pub id: Uuid,
    pub display_name: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub body: String,
    pub anchor_from: Option<u64>,
    pub anchor_to: Option<u64>,
    pub base_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReplyRequest {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct EditCommentRequest {
    pub body: String,
}
```

- [ ] Add `pub mod comments;` to `packages/server/src/documents/mod.rs`.

### 3. Add database query methods

- [ ] Add to `packages/server/src/db.rs` (or a new `db/comments.rs` if the file is getting large):

```rust
// Struct for the joined query result
#[derive(Debug, sqlx::FromRow)]
pub struct CommentWithAuthor {
    // comment fields
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
    // author fields (aliased)
    pub author_display_name: String,
    pub author_email: String,
}
```

Implement these query functions:

- `list_comments(pool, document_id, status_filter) -> Result<Vec<CommentWithAuthor>>` — `SELECT c.*, u.display_name AS author_display_name, u.email AS author_email FROM comments c JOIN users u ON c.author_id = u.id WHERE c.document_id = $1 [AND c.status = $2] ORDER BY c.created_at ASC`
- `create_comment(pool, document_id, author_id, body, anchor_start, anchor_end) -> Result<CommentRow>` — `INSERT ... RETURNING *`
- `create_reply(pool, document_id, author_id, parent_id, body) -> Result<CommentRow>` — `INSERT ... (with parent_id, NULL anchors) RETURNING *`
- `get_comment(pool, comment_id) -> Result<Option<CommentRow>>` — `SELECT * FROM comments WHERE id = $1`
- `update_comment_body(pool, comment_id, body) -> Result<CommentRow>` — `UPDATE comments SET body = $1, updated_at = NOW() WHERE id = $2 RETURNING *`
- `update_comment_status(pool, comment_id, status) -> Result<CommentRow>` — `UPDATE comments SET status = $1, updated_at = NOW() WHERE id = $2 RETURNING *`

### 4. Implement comment REST handlers

- [ ] In `packages/server/src/documents/api.rs`, replace the stub `list_comments` and `create_comment` handlers with real implementations and add the new handlers:

**`list_comments`** — extract `document_id` from path, optional `status` query param, check Read permission, call `db::list_comments`, map to `CommentResponse` vec.

**`create_comment`** — extract `document_id` from path, parse `CreateCommentRequest` body, check Comment permission, validate body is non-empty, call `db::create_comment` (with NULL anchors for now), return created comment.

**`reply_to_comment`** — extract `document_id` and `comment_id` from path, parse `CreateReplyRequest`, check Comment permission, verify parent exists and belongs to same document, verify parent is top-level (`parent_id IS NULL`), call `db::create_reply`, return created reply.

**`edit_comment`** — extract `comment_id`, parse `EditCommentRequest`, check Comment permission, verify comment exists, verify caller is the author (`comment.author_id == user.id`), validate body non-empty, call `db::update_comment_body`.

**`resolve_comment`** — extract `comment_id`, check Comment permission, verify comment exists and is top-level, call `db::update_comment_status(id, "resolved")`.

**`unresolve_comment`** — same as resolve but sets status to `"open"`.

### 5. Update the router

- [ ] In `packages/server/src/lib.rs`, replace the existing stub comment routes with:

```rust
.route("/api/docs/{id}/comments", get(list_comments).post(create_comment))
.route("/api/docs/{id}/comments/{comment_id}", put(edit_comment))
.route("/api/docs/{id}/comments/{comment_id}/replies", post(reply_to_comment))
.route("/api/docs/{id}/comments/{comment_id}/resolve", post(resolve_comment))
.route("/api/docs/{id}/comments/{comment_id}/unresolve", post(unresolve_comment))
```

### 6. Add frontend API functions

- [ ] Add to `packages/web/src/api.ts`:

```typescript
export interface Comment {
  id: string;
  document_id: string;
  author: { id: string; display_name: string; email: string };
  parent_id: string | null;
  body: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export async function listComments(
  docId: string,
  status?: 'open' | 'resolved' | 'all',
): Promise<Comment[]> {
  const params = status ? `?status=${status}` : '';
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments${params}`,
  );
  if (!res.ok) throw new Error('Failed to list comments');
  return res.json();
}

export async function createComment(
  docId: string,
  body: string,
  anchorFrom?: number,
  anchorTo?: number,
): Promise<Comment> {
  const res = await authFetch(`${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      body,
      anchor_from: anchorFrom,
      anchor_to: anchorTo,
    }),
  });
  if (!res.ok) throw new Error('Failed to create comment');
  return res.json();
}

export async function replyToComment(
  docId: string,
  commentId: string,
  body: string,
): Promise<Comment> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments/${encodeURIComponent(commentId)}/replies`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ body }),
    },
  );
  if (!res.ok) throw new Error('Failed to reply to comment');
  return res.json();
}

export async function editComment(
  docId: string,
  commentId: string,
  body: string,
): Promise<Comment> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments/${encodeURIComponent(commentId)}`,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ body }),
    },
  );
  if (!res.ok) throw new Error('Failed to edit comment');
  return res.json();
}

export async function resolveComment(docId: string, commentId: string): Promise<Comment> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments/${encodeURIComponent(commentId)}/resolve`,
    { method: 'POST' },
  );
  if (!res.ok) throw new Error('Failed to resolve comment');
  return res.json();
}

export async function unresolveComment(docId: string, commentId: string): Promise<Comment> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments/${encodeURIComponent(commentId)}/unresolve`,
    { method: 'POST' },
  );
  if (!res.ok) throw new Error('Failed to unresolve comment');
  return res.json();
}
```

### 7. Test the endpoints

- [ ] Start the full dev stack: `pnpm dev`
- [ ] Register two users and create a document.
- [ ] Test with curl or the API directly:

```bash
# Create a comment
curl -X POST http://localhost:8080/api/docs/{id}/comments \
  -H 'Authorization: Bearer <token>' \
  -H 'Content-Type: application/json' \
  -d '{"body": "This needs clarification"}'

# List comments
curl http://localhost:8080/api/docs/{id}/comments \
  -H 'Authorization: Bearer <token>'

# Reply
curl -X POST http://localhost:8080/api/docs/{id}/comments/{cid}/replies \
  -H 'Authorization: Bearer <token>' \
  -H 'Content-Type: application/json' \
  -d '{"body": "Good point, will fix"}'

# Resolve
curl -X POST http://localhost:8080/api/docs/{id}/comments/{cid}/resolve \
  -H 'Authorization: Bearer <token>'

# Edit (must be author)
curl -X PUT http://localhost:8080/api/docs/{id}/comments/{cid} \
  -H 'Authorization: Bearer <token>' \
  -H 'Content-Type: application/json' \
  -d '{"body": "Updated comment text"}'
```

- [ ] Verify permission enforcement: a Read-role user gets 403 on create/reply/resolve.
- [ ] Verify author enforcement: editing someone else's comment returns 403.
- [ ] Verify threading rules: replying to a reply returns 400.

### 8. Build and format check

- [ ] Run `cargo build` in `packages/server/` — compiles without errors.
- [ ] Run `cargo test` in `packages/server/` — all tests pass.
- [ ] Run `pnpm -F @cadmus/web build` — TypeScript compiles (new API types are valid).
- [ ] Run `pnpm run format:check` — no formatting issues.

## Verification

- [ ] Migration creates the `comments` table with correct schema
- [ ] `GET /api/docs/{id}/comments` returns comments with author info
- [ ] `GET /api/docs/{id}/comments?status=open` filters correctly
- [ ] `POST /api/docs/{id}/comments` creates a top-level comment
- [ ] `POST /api/docs/{id}/comments/{cid}/replies` creates a threaded reply
- [ ] Replying to a reply returns 400
- [ ] `PUT /api/docs/{id}/comments/{cid}` updates body (author only)
- [ ] Editing another user's comment returns 403
- [ ] `POST /api/docs/{id}/comments/{cid}/resolve` sets status to resolved
- [ ] `POST /api/docs/{id}/comments/{cid}/unresolve` sets status back to open
- [ ] Resolve/unresolve on a reply returns 400
- [ ] Read-role users can list but not create/reply/resolve
- [ ] Comment/Edit-role users can create, reply, resolve
- [ ] Empty body returns 400

## Files Modified

| File                                                     | Change                                         |
| -------------------------------------------------------- | ---------------------------------------------- |
| `packages/server/migrations/20260312000003_comments.sql` | New: comments table migration                  |
| `packages/server/src/documents/comments.rs`              | New: comment data structures and request types |
| `packages/server/src/documents/mod.rs`                   | Add `pub mod comments`                         |
| `packages/server/src/db.rs`                              | Add comment query methods                      |
| `packages/server/src/documents/api.rs`                   | Replace stubs with real comment handlers       |
| `packages/server/src/lib.rs`                             | Update comment routes                          |
| `packages/web/src/api.ts`                                | Add comment API functions and types            |
