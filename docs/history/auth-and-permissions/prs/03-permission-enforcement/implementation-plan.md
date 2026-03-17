# PR 3: Permission Enforcement — Implementation Plan

## Prerequisites

- [x] PR 2 (JWT Middleware & Auth Extractors) merged
- [x] All document endpoints require `AuthUser` extractor

## Steps

### Step 1: Define the Permission enum

- [x] Create `packages/server/src/documents/permissions.rs`:

```rust
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Read,
    Comment,
    Edit,
}

impl Permission {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "read" => Some(Permission::Read),
            "comment" => Some(Permission::Comment),
            "edit" => Some(Permission::Edit),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::Read => "read",
            Permission::Comment => "comment",
            Permission::Edit => "edit",
        }
    }
}

impl PartialOrd for Permission {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Permission {
    fn cmp(&self, other: &Self) -> Ordering {
        let rank = |p: &Permission| match p {
            Permission::Read => 0,
            Permission::Comment => 1,
            Permission::Edit => 2,
        };
        rank(self).cmp(&rank(other))
    }
}
```

- [x] Add `pub mod permissions;` to `packages/server/src/documents/mod.rs`

### Step 2: Add permission database methods

- [x] Add to `packages/server/src/db.rs`:

```rust
pub async fn get_user_permission(
    &self,
    document_id: Uuid,
    user_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT role FROM document_permissions WHERE document_id = $1 AND user_id = $2"
    )
    .bind(document_id)
    .bind(user_id)
    .fetch_optional(&self.pool)
    .await?;
    Ok(row.map(|r| r.0))
}

pub async fn list_permissions_with_users(
    &self,
    document_id: Uuid,
) -> Result<Vec<PermissionWithUser>, sqlx::Error> {
    sqlx::query_as::<_, PermissionWithUser>(
        r#"SELECT dp.user_id, u.email, u.display_name, dp.role
           FROM document_permissions dp
           INNER JOIN users u ON u.id = dp.user_id
           WHERE dp.document_id = $1
           ORDER BY dp.created_at ASC"#
    )
    .bind(document_id)
    .fetch_all(&self.pool)
    .await
}

pub async fn update_permission(
    &self,
    document_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE document_permissions SET role = $3 WHERE document_id = $1 AND user_id = $2"
    )
    .bind(document_id)
    .bind(user_id)
    .bind(role)
    .execute(&self.pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_permission(
    &self,
    document_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM document_permissions WHERE document_id = $1 AND user_id = $2"
    )
    .bind(document_id)
    .bind(user_id)
    .execute(&self.pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_accessible_documents(
    &self,
    user_id: Uuid,
) -> Result<Vec<DocumentRow>, sqlx::Error> {
    sqlx::query_as::<_, DocumentRow>(
        r#"SELECT d.id, d.title, d.schema_version, d.snapshot_key, d.created_at, d.updated_at
           FROM documents d
           INNER JOIN document_permissions dp ON dp.document_id = d.id
           WHERE dp.user_id = $1
           ORDER BY d.updated_at DESC"#
    )
    .bind(user_id)
    .fetch_all(&self.pool)
    .await
}
```

- [x] Add `PermissionWithUser` struct:

```rust
#[derive(Debug, sqlx::FromRow)]
pub struct PermissionWithUser {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
}
```

### Step 3: Add the require_permission helper

- [x] In `packages/server/src/documents/permissions.rs`, add:

```rust
use crate::db::Database;
use crate::errors::AppError;
use uuid::Uuid;

pub async fn require_permission(
    db: &Database,
    user_id: Uuid,
    document_id: Uuid,
    required: Permission,
) -> Result<Permission, AppError> {
    let role_str = db.get_user_permission(document_id, user_id).await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Forbidden("You don't have access to this document".into()))?;

    let permission = Permission::from_str(&role_str)
        .ok_or_else(|| AppError::Internal(format!("Invalid role in database: {}", role_str)))?;

    if permission < required {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }

    Ok(permission)
}

pub async fn require_owner(
    db: &Database,
    user_id: Uuid,
    document_id: Uuid,
) -> Result<(), AppError> {
    let doc = db.get_document(document_id).await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

    if doc.created_by != Some(user_id) {
        return Err(AppError::Forbidden("Only the document owner can perform this action".into()));
    }

    Ok(())
}
```

### Step 4: Add permission checks to document REST handlers

- [x] Update each handler in `packages/server/src/documents/api.rs`:

**list_documents** — replace `db.list_documents()` with `db.list_accessible_documents(auth.user_id)`:

```rust
pub async fn list_documents(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DocumentSummary>>, AppError> {
    let rows = state.db.list_accessible_documents(auth.user_id).await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}
```

**get_document** — require Read:

```rust
require_permission(&state.db, auth.user_id, id, Permission::Read).await?;
```

**update_document** — require Edit:

```rust
require_permission(&state.db, auth.user_id, id, Permission::Edit).await?;
```

**delete_document** — require owner:

```rust
require_owner(&state.db, auth.user_id, id).await?;
```

**get_content** — require Read:

```rust
require_permission(&state.db, auth.user_id, id, Permission::Read).await?;
```

**push_content** — require Edit:

```rust
require_permission(&state.db, auth.user_id, id, Permission::Edit).await?;
```

**list_comments** — require Read:

```rust
require_permission(&state.db, auth.user_id, id, Permission::Read).await?;
```

**create_comment** — require Comment:

```rust
require_permission(&state.db, auth.user_id, id, Permission::Comment).await?;
```

### Step 5: Add sharing endpoint handlers

- [x] Add sharing handlers to `packages/server/src/documents/permissions.rs` (or a new file `sharing.rs`):

**list_permissions:**

```rust
pub async fn list_permissions(
    auth: AuthUser,
    Path(doc_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PermissionEntry>>, AppError> {
    require_permission(&state.db, auth.user_id, doc_id, Permission::Edit).await?;
    let doc = state.db.get_document(doc_id).await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

    let perms = state.db.list_permissions_with_users(doc_id).await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let entries: Vec<PermissionEntry> = perms.into_iter().map(|p| PermissionEntry {
        user_id: p.user_id,
        email: p.email,
        display_name: p.display_name,
        role: p.role,
        is_owner: doc.created_by == Some(p.user_id),
    }).collect();

    Ok(Json(entries))
}
```

**add_permission** (invite by email):

```rust
pub async fn add_permission(
    auth: AuthUser,
    Path(doc_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddPermissionRequest>,
) -> Result<StatusCode, AppError> {
    require_owner(&state.db, auth.user_id, doc_id).await?;

    let user = state.db.get_user_by_email(&body.email.to_lowercase().trim()).await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    // Validate role
    Permission::from_str(&body.role)
        .ok_or_else(|| AppError::BadRequest("Invalid role".into()))?;

    // Check if permission already exists
    if state.db.get_user_permission(doc_id, user.id).await
        .map_err(|e| AppError::Internal(e.to_string()))?.is_some() {
        return Err(AppError::Conflict("User already has access".into()));
    }

    state.db.create_permission(doc_id, user.id, &body.role).await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(StatusCode::CREATED)
}
```

**update_permission** and **delete_permission** — similar pattern, require owner, prevent self-modification.

### Step 6: Register sharing routes

- [x] In `packages/server/src/lib.rs`, add:

```rust
.route("/api/docs/{id}/permissions", get(documents::permissions::list_permissions))
.route("/api/docs/{id}/permissions", post(documents::permissions::add_permission))
.route("/api/docs/{id}/permissions/{user_id}", patch(documents::permissions::update_permission_handler))
.route("/api/docs/{id}/permissions/{user_id}", delete(documents::permissions::delete_permission_handler))
```

### Step 7: Add WebSocket token validation

- [x] Update `packages/server/src/websocket/handler.rs`:

```rust
#[derive(Deserialize)]
pub struct WsQueryParams {
    pub token: String,
}

pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Path(doc_id): Path<Uuid>,
    Query(params): Query<WsQueryParams>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    // Validate ws-token
    let claims = crate::auth::jwt::validate_token(
        &params.token, "ws", &state.config.jwt_secret
    )?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Invalid token".into()))?;

    // Check document permission
    let permission = crate::documents::permissions::require_permission(
        &state.db, user_id, doc_id, Permission::Read
    ).await?;

    // Load session
    let session = state.document_sessions
        .get_or_load(doc_id, &state.db, &state.storage).await?;

    session.add_connection();

    let document_sessions = state.document_sessions.clone();
    let db = state.db.clone();
    let storage = state.storage.clone();

    Ok(ws.on_upgrade(move |socket| {
        handle_ws(socket, session, permission, document_sessions, db, storage)
    }))
}
```

### Step 8: Implement PermissionedProtocol

- [x] Create `packages/server/src/websocket/protocol.rs`:

```rust
use yrs::sync::{DefaultProtocol, Error, Message, MessageReader, Protocol, SyncMessage};
use yrs::updates::decoder::Decode;
use crate::documents::permissions::Permission;

pub struct PermissionedProtocol {
    pub permission: Permission,
}

impl Protocol for PermissionedProtocol {
    // Override handle_update to reject edits from non-Edit users
    // Delegate all other methods to DefaultProtocol
}
```

- [x] Add `pub mod protocol;` to `packages/server/src/websocket/mod.rs`

- [x] Update `handle_ws` to use `PermissionedProtocol` when subscribing to the BroadcastGroup:

```rust
async fn handle_ws(
    socket: axum::extract::ws::WebSocket,
    session: Arc<DocumentSession>,
    permission: Permission,
    /* ... */
) {
    // ... existing setup ...
    let protocol = PermissionedProtocol { permission };
    let sub = session.broadcast_group.subscribe_with(sink, binary_stream, protocol);
    // ... rest unchanged ...
}
```

### Step 9: Update frontend collaboration.ts

- [x] Update `packages/web/src/collaboration.ts` to pass token as query parameter:

```typescript
export function createCollaborationProvider(docId: string, wsToken: string) {
  const ydoc = new Y.Doc();
  // Pass token as query parameter
  const provider = new WebsocketProvider(
    WS_BASE_URL,
    `${docId}/ws?token=${encodeURIComponent(wsToken)}`,
    ydoc,
  );
  return { ydoc, provider };
}
```

- [x] This requires the frontend auth context (PR 4) to obtain ws-tokens. For now, update the function signature; the actual token passing is wired in PR 4/5.

### Step 10: Tests

- [x] Write permission enforcement tests in `packages/server/tests/permissions.rs`:
  - User with no permission on a document → 403 on GET
  - User with Read permission → can GET, cannot PATCH/DELETE/POST content
  - User with Comment permission → can GET, can POST comments, cannot POST content
  - User with Edit permission → can GET, PATCH, POST content, cannot DELETE (non-owner)
  - Owner can DELETE
  - Owner can invite, change roles, remove users
  - Non-owner with Edit cannot manage permissions
  - WebSocket upgrade without token → 401
  - WebSocket upgrade with valid token + Read permission → connects, cannot send updates
  - WebSocket upgrade with valid token + Edit permission → connects, can send updates
  - Document listing only returns accessible documents

## Verification

- [x] `cargo build` succeeds
- [x] User with no permission gets 403 on all document endpoints
- [x] User with Read permission can view but not edit
- [x] User with Edit permission can edit but only owner can delete
- [x] WebSocket connections require a valid ws-token
- [x] Read-only WebSocket clients cannot push edits (updates rejected server-side)
- [x] Document listing is scoped to the authenticated user's accessible documents
- [x] Sharing endpoints work (invite by email, change role, remove access)
- [x] `cargo test` passes all tests

## Files Created/Modified

- `packages/server/src/documents/permissions.rs` (new)
- `packages/server/src/documents/mod.rs` (modified — add permissions module)
- `packages/server/src/documents/api.rs` (modified — add permission checks)
- `packages/server/src/websocket/handler.rs` (modified — token validation, permission passing)
- `packages/server/src/websocket/protocol.rs` (new — PermissionedProtocol)
- `packages/server/src/websocket/mod.rs` (modified — add protocol module)
- `packages/server/src/db.rs` (modified — permission queries)
- `packages/server/src/lib.rs` (modified — sharing routes)
- `packages/web/src/collaboration.ts` (modified — token parameter)
- `packages/server/tests/permissions.rs` (new)
