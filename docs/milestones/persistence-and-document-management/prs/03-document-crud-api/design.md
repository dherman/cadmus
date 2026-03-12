# PR 3: Document CRUD API

## Purpose

Implement the REST endpoints for creating, listing, retrieving, and deleting documents. These replace the stub handlers from Milestone 1 with real database-backed operations.

After this PR, clients can create new documents, browse existing ones, and delete documents they no longer need — all via the REST API. The frontend dashboard (PR 4) consumes these endpoints.

## Endpoints

### `GET /api/docs` — List documents

Returns all documents, ordered by most recently updated. No pagination for now (added with auth in M3 when per-user filtering makes pagination necessary).

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "Design Spec",
    "created_at": "2026-03-12T10:00:00Z",
    "updated_at": "2026-03-12T14:30:00Z"
  }
]
```

### `POST /api/docs` — Create document

Creates a new document. Returns the created document metadata. Optionally accepts initial content (plain text treated as a document title — full markdown import comes with the sidecar in M4).

Request:
```json
{
  "title": "My New Document"
}
```

Response (201):
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "My New Document",
  "created_at": "2026-03-12T10:00:00Z",
  "updated_at": "2026-03-12T10:00:00Z"
}
```

The document ID is generated server-side (UUIDv4). The document starts with no snapshot — the first WebSocket connection creates an empty Yrs doc, and the first flush persists it.

### `GET /api/docs/{id}` — Get document metadata

Returns a single document's metadata. 404 if not found.

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Design Spec",
  "created_at": "2026-03-12T10:00:00Z",
  "updated_at": "2026-03-12T14:30:00Z"
}
```

### `DELETE /api/docs/{id}` — Delete document

Deletes the document, its update log entries (via CASCADE), and its S3 snapshot. Returns 204 on success, 404 if not found.

This also unloads the document session from memory if it's currently active. Active WebSocket connections to the document are closed.

### `PATCH /api/docs/{id}` — Update document metadata

Updates document title. Returns the updated document.

Request:
```json
{
  "title": "Updated Title"
}
```

Response (200):
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Updated Title",
  "created_at": "2026-03-12T10:00:00Z",
  "updated_at": "2026-03-12T14:31:00Z"
}
```

## WebSocket Integration

Currently, the WebSocket handler creates an empty in-memory doc for any UUID. With this PR, the behavior changes:

- The WebSocket upgrade handler checks that the document exists in the database before proceeding
- If the document doesn't exist, the upgrade is rejected with a 404 response
- If the document exists, `get_or_load` proceeds as before (loading from S3 if persistent state exists)

This prevents orphaned in-memory sessions for nonexistent documents.

## Response Types

The API uses a consistent `DocumentSummary` response type. The existing struct in `api.rs` is updated to derive from the database row:

```rust
#[derive(Serialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

We intentionally omit `schema_version` and `snapshot_key` from the API response — these are internal implementation details.

## Error Handling

| Scenario                    | Status Code | Response Body                          |
| --------------------------- | ----------- | -------------------------------------- |
| Document not found          | 404         | `{ "error": "Document not found" }`   |
| Invalid UUID in path        | 400         | `{ "error": "Invalid document ID" }`  |
| Missing title in create     | 400         | `{ "error": "Title is required" }`    |
| Database error              | 500         | `{ "error": "Internal server error" }` |
| Delete with active sessions | 204         | (sessions closed, document deleted)    |

## What's Not Included

- **Pagination/filtering** — all documents are returned. With no auth, there's no per-user filtering to make this necessary. Added in M3.
- **Content read/write endpoints** (`GET/POST /api/docs/{id}/content`) — these require the sidecar for markdown conversion. They remain stubs until M4.
- **Permissions enforcement** — no auth, no permissions. All operations are open to anyone.
