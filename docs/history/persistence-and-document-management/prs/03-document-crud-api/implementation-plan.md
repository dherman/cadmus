# PR 3: Document CRUD API — Implementation Plan

## Prerequisites

- [x] PR 1 (Database Schema & Migrations) merged
- [x] PostgreSQL running with migrations applied

## Steps

### Step 1: Update the DocumentSummary response type

- [x] In `packages/server/src/documents/api.rs`, update `DocumentSummary` to use proper types:

```rust
use chrono::{DateTime, Utc};

#[derive(Serialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DocumentRow> for DocumentSummary {
    fn from(row: DocumentRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
```

### Step 2: Implement the list endpoint

- [x] Replace the `list_documents` stub:

```rust
pub async fn list_documents(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DocumentSummary>>, AppError> {
    let rows = state.db.list_documents().await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let docs: Vec<DocumentSummary> = rows.into_iter().map(Into::into).collect();
    Ok(Json(docs))
}
```

### Step 3: Implement the create endpoint

- [x] Replace the `create_document` stub:

```rust
pub async fn create_document(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateDocumentRequest>,
) -> Result<(StatusCode, Json<DocumentSummary>), AppError> {
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("Title is required".to_string()));
    }

    let id = Uuid::new_v4();
    let row = state.db.create_document(id, body.title.trim()).await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(row.into())))
}
```

- [x] Import `StatusCode` from axum
- [x] Update the return type in the route to accept the tuple

### Step 4: Implement the get endpoint

- [x] Replace the `get_document` stub:

```rust
pub async fn get_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<DocumentSummary>, AppError> {
    let row = state.db.get_document(id).await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    Ok(Json(row.into()))
}
```

### Step 5: Implement the delete endpoint

- [x] Add a `delete_document` handler:

```rust
pub async fn delete_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    // 1. Check if the document exists
    let doc = state.db.get_document(id).await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    // 2. Unload from memory if active
    state.document_sessions.unload(id).await;

    // 3. Delete S3 snapshot if exists
    if doc.snapshot_key.is_some() {
        state.storage.delete_snapshot(id).await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    // 4. Delete from database (cascades to update_log and permissions)
    state.db.delete_document(id).await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
```

### Step 6: Implement the rename endpoint

- [x] Add `update_document_title` to `db.rs`:

```rust
pub async fn update_document_title(&self, id: Uuid, title: &str) -> Result<Option<DocumentRow>, sqlx::Error> {
    sqlx::query_as!(
        DocumentRow,
        r#"UPDATE documents SET title = $2, updated_at = NOW() WHERE id = $1
           RETURNING id, title, schema_version, snapshot_key, created_at, updated_at"#,
        id, title
    )
    .fetch_optional(&self.pool)
    .await
}
```

- [x] Add the handler:

```rust
#[derive(Deserialize)]
pub struct UpdateDocumentRequest {
    pub title: Option<String>,
}

pub async fn update_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateDocumentRequest>,
) -> Result<Json<DocumentSummary>, AppError> {
    let title = body.title
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("Title is required".to_string()))?;

    let row = state.db.update_document_title(id, title.trim()).await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    Ok(Json(row.into()))
}
```

### Step 7: Register routes

- [x] In `packages/server/src/lib.rs`, add the new routes:

```rust
use axum::routing::{delete, patch};

.route("/api/docs/{id}", delete(documents::api::delete_document))
.route("/api/docs/{id}", patch(documents::api::update_document))
```

### Step 8: Guard WebSocket connections

- [x] In `websocket/handler.rs`, verify the document exists before upgrading:

```rust
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Path(doc_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    // Verify document exists in database
    state.db.get_document(doc_id).await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    let session = state.document_sessions.get_or_load(doc_id).await;
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, session)))
}
```

### Step 9: Tests

- [x] Write API tests in `packages/server/tests/api.rs`:

```rust
#[tokio::test]
async fn test_create_and_list_documents() {
    // POST /api/docs → 201, verify response
    // GET /api/docs → list includes the created doc
}

#[tokio::test]
async fn test_get_document() {
    // Create a doc, GET /api/docs/{id} → 200
    // GET /api/docs/{nonexistent} → 404
}

#[tokio::test]
async fn test_delete_document() {
    // Create a doc, DELETE /api/docs/{id} → 204
    // GET /api/docs/{id} → 404
}

#[tokio::test]
async fn test_update_document_title() {
    // Create a doc, PATCH /api/docs/{id} → 200 with new title
}

#[tokio::test]
async fn test_websocket_rejects_nonexistent_document() {
    // Attempt WebSocket upgrade for nonexistent doc → 404
}
```

## Verification

- [x] `curl -X POST localhost:8080/api/docs -H 'Content-Type: application/json' -d '{"title":"Test"}'` → 201 with document JSON
- [x] `curl localhost:8080/api/docs` → list includes the created document
- [x] `curl localhost:8080/api/docs/{id}` → 200 with document details
- [x] `curl -X PATCH localhost:8080/api/docs/{id} -H 'Content-Type: application/json' -d '{"title":"Renamed"}'` → 200 with updated title
- [x] `curl -X DELETE localhost:8080/api/docs/{id}` → 204
- [x] `curl localhost:8080/api/docs/{id}` → 404 after deletion
- [x] WebSocket connection to a nonexistent document ID fails
- [x] WebSocket connection to a valid document ID succeeds
- [x] `cargo test` passes all new tests

## Files Created/Modified

- `packages/server/src/documents/api.rs` (modified — implement all handlers)
- `packages/server/src/db.rs` (modified — add update_document_title)
- `packages/server/src/lib.rs` (modified — add delete and patch routes)
- `packages/server/src/websocket/handler.rs` (modified — document existence check)
- `packages/server/tests/api.rs` (new)
