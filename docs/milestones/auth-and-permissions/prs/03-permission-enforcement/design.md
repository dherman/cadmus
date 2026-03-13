# PR 3: Permission Enforcement

## Purpose

Enforce per-document permissions on both REST endpoints and WebSocket connections. After this PR, users can only access documents they have permission for, and their access level (Read/Comment/Edit) determines what operations they can perform. This is the core security PR of the milestone.

## Permission Model

### Roles

| Role        | REST: View doc | REST: Edit metadata | REST: Delete doc | REST: Push content | WS: Receive updates | WS: Send updates (edit) | Comments (M5) |
| ----------- | -------------- | ------------------- | ---------------- | ------------------ | ------------------- | ----------------------- | ------------- |
| **Read**    | ✓              | ✗                   | ✗                | ✗                  | ✓                   | ✗                       | View only     |
| **Comment** | ✓              | ✗                   | ✗                | ✗                  | ✓                   | ✗                       | Create/reply  |
| **Edit**    | ✓              | ✓                   | Owner only       | ✓                  | ✓                   | ✓                       | Create/reply  |

### Owner Semantics

The document creator (`documents.created_by`) has additional privileges beyond the Edit role:

- Can delete the document
- Can invite other users (manage permissions)
- Can change other users' roles
- Can remove other users' access

Any Edit-role user can edit the document content and metadata (title), but only the owner can manage access and delete.

### Permission Resolution

For a given (user, document) pair:

1. Look up `document_permissions` for a row matching both `user_id` and `document_id`.
2. If no row exists → the user has no access (403 for authenticated users).
3. If a row exists → the `role` column determines the access level.

Future milestones will add workspace-level and org-level defaults that feed into this resolution. For now, it's strictly per-document.

## REST Permission Enforcement

### Approach: Per-Handler Permission Checks

Rather than middleware, permission checks are done inside each handler. This is more explicit and allows different endpoints to require different permission levels.

A helper function centralizes the check:

```rust
pub async fn require_permission(
    db: &Database,
    user_id: Uuid,
    document_id: Uuid,
    required_role: Permission,
) -> Result<Permission, AppError> {
    let permission = db.get_user_permission(document_id, user_id).await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Forbidden("You don't have access to this document".into()))?;

    if permission < required_role {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }

    Ok(permission)
}
```

Where `Permission` is an enum with ordering: `Read < Comment < Edit`.

### Endpoint Permission Requirements

| Endpoint                       | Minimum Role | Additional Check                  |
| ------------------------------ | ------------ | --------------------------------- |
| `GET /api/docs`                | (special)    | Filter to accessible docs only    |
| `POST /api/docs`               | (none)       | Any authenticated user can create |
| `GET /api/docs/{id}`           | Read         |                                   |
| `PATCH /api/docs/{id}`         | Edit         |                                   |
| `DELETE /api/docs/{id}`        | Edit         | Must be owner                     |
| `GET /api/docs/{id}/content`   | Read         |                                   |
| `POST /api/docs/{id}/content`  | Edit         |                                   |
| `GET /api/docs/{id}/comments`  | Read         |                                   |
| `POST /api/docs/{id}/comments` | Comment      |                                   |

### Document Listing Change

`GET /api/docs` changes from "return all documents" to "return documents the authenticated user has access to":

```sql
SELECT d.id, d.title, d.created_at, d.updated_at
FROM documents d
INNER JOIN document_permissions dp ON dp.document_id = d.id
WHERE dp.user_id = $1
ORDER BY d.updated_at DESC
```

## WebSocket Permission Enforcement

### Token Validation on Upgrade

The WebSocket upgrade handler changes to require a valid ws-token:

```rust
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Path(doc_id): Path<Uuid>,
    Query(params): Query<WsQueryParams>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Validate ws-token from query parameter
    let claims = jwt::validate_token(&params.token, "ws", &state.config.jwt_secret)?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Invalid token".into()))?;

    // 2. Check document permission
    let permission = require_permission(&state.db, user_id, doc_id, Permission::Read).await?;

    // 3. Load document session
    let session = state.document_sessions
        .get_or_load(doc_id, &state.db, &state.storage).await?;

    // 4. Upgrade with permission context
    session.add_connection();
    Ok(ws.on_upgrade(move |socket| {
        handle_ws(socket, session, permission, /* ... */)
    }))
}
```

### PermissionedProtocol

A custom y-sync `Protocol` implementation that gates incoming messages by role:

```rust
pub struct PermissionedProtocol {
    permission: Permission,
}

impl Protocol for PermissionedProtocol {
    fn handle_sync_step1(/* ... */) -> Result<Option<Message>, Error> {
        // All roles: allowed (needed for initial document sync)
        DefaultProtocol.handle_sync_step1(/* ... */)
    }

    fn handle_sync_step2(/* ... */) -> Result<Option<Message>, Error> {
        // All roles: allowed (needed for initial document sync)
        DefaultProtocol.handle_sync_step2(/* ... */)
    }

    fn handle_update(/* ... */) -> Result<Option<Message>, Error> {
        match self.permission {
            Permission::Edit => DefaultProtocol.handle_update(/* ... */),
            _ => {
                // Read/Comment users cannot send document updates
                tracing::warn!("Rejecting update from non-edit user");
                Err(Error::PermissionDenied)
            }
        }
    }

    fn handle_auth(/* ... */) -> Result<Option<Message>, Error> {
        DefaultProtocol.handle_auth(/* ... */)
    }
}
```

The key constraint: the `BroadcastGroup::subscribe` call needs to accept a custom protocol. The `yrs-axum` crate's `BroadcastGroup` supports this — we pass our `PermissionedProtocol` instance when subscribing a new connection.

### Awareness

All permission levels can send and receive Awareness messages (presence/cursors). This is important — Read-only users should still appear in the collaborator list.

The awareness state now includes real user information from the JWT rather than random generated names:

```json
{
  "user": {
    "id": "user-123",
    "name": "Alice",
    "color": "#e06c75"
  },
  "cursor": { "anchor": 142, "head": 142 }
}
```

## Sharing Endpoints

These endpoints manage the `document_permissions` table:

### `GET /api/docs/{id}/permissions`

Returns the permission list for a document. Requires Edit role (or owner).

```json
[
  {
    "user_id": "...",
    "email": "alice@example.com",
    "display_name": "Alice",
    "role": "edit",
    "is_owner": true
  },
  {
    "user_id": "...",
    "email": "bob@example.com",
    "display_name": "Bob",
    "role": "comment",
    "is_owner": false
  }
]
```

### `POST /api/docs/{id}/permissions`

Invites a user by email. Requires owner.

```json
{
  "email": "bob@example.com",
  "role": "comment"
}
```

If the email doesn't match an existing user → 404 "User not found". (No invitation system in the prototype; the user must register first.)

### `PATCH /api/docs/{id}/permissions/{user_id}`

Changes a user's role. Requires owner. Cannot change own role.

```json
{
  "role": "edit"
}
```

### `DELETE /api/docs/{id}/permissions/{user_id}`

Removes a user's access. Requires owner. Cannot remove own access.

## Database Methods

New methods on `Database`:

```rust
pub async fn get_user_permission(&self, document_id: Uuid, user_id: Uuid) -> Result<Option<String>, sqlx::Error>;
pub async fn list_permissions(&self, document_id: Uuid) -> Result<Vec<PermissionRow>, sqlx::Error>;
pub async fn update_permission(&self, document_id: Uuid, user_id: Uuid, role: &str) -> Result<bool, sqlx::Error>;
pub async fn delete_permission(&self, document_id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error>;
```

The `create_permission` method was added in PR 2.

## WebSocket Reconnection on Token Expiry

When the server detects a token has expired during a long session, it closes the WebSocket with close code `4401`. The frontend handles this by:

1. Detecting close code `4401`.
2. Calling `POST /api/auth/ws-token` to get a fresh token.
3. Reconnecting the WebSocket with the new token URL.

This is transparent to the user — the y-websocket provider handles reconnection automatically, and the sync protocol replays any missed updates.

## Error Responses

| Scenario                          | Status | Body                                                            |
| --------------------------------- | ------ | --------------------------------------------------------------- |
| No permission record for user+doc | 403    | `{ "error": "You don't have access to this document" }`         |
| Insufficient role                 | 403    | `{ "error": "Insufficient permissions" }`                       |
| Only owner can delete             | 403    | `{ "error": "Only the document owner can delete" }`             |
| Only owner can manage permissions | 403    | `{ "error": "Only the document owner can manage permissions" }` |
| WS upgrade without token          | 401    | `{ "error": "Missing WebSocket token" }`                        |
| WS upgrade with invalid token     | 401    | `{ "error": "Invalid or expired token" }`                       |
| WS upgrade with no doc permission | 403    | `{ "error": "You don't have access to this document" }`         |

## What's Not Included

- **Frontend sharing UI** — PR 5 builds the sharing dialog that calls these endpoints.
- **Frontend permission-aware controls** — PR 4 handles login UI; PR 5 adds permission-aware editor state (disable toolbar for read-only users).
- **Workspace/org-level permissions** — M7 Enterprise.
