# PR 2: JWT Middleware & Auth Extractors — Implementation Plan

## Prerequisites

- [x] PR 1 (Users & Auth Endpoints) merged
- [x] Auth endpoints working (register, login, refresh)

## Steps

### Step 1: Create the auth middleware module

- [x] Create `packages/server/src/auth/middleware.rs`
- [x] Add `pub mod middleware;` to `packages/server/src/auth/mod.rs`

### Step 2: Implement the AuthUser extractor

- [x] Define `AuthUser` struct:

```rust
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::errors::AppError;
use crate::AppState;
use super::jwt;

/// Authenticated user extracted from a valid JWT access token.
/// Add this to any handler's parameters to require authentication.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
}
```

- [x] Implement `FromRequestParts<Arc<AppState>>` for `AuthUser`:

```rust
#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        // 1. Get Authorization header
        let auth_header = parts.headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing authorization header".into()))?;

        // 2. Extract Bearer token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Invalid authorization header".into()))?;

        // 3. Validate JWT (must be "access" type)
        let claims = jwt::validate_token(token, "access", &state.config.jwt_secret)?;

        // 4. Build AuthUser from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AppError::Unauthorized("Invalid token subject".into()))?;

        Ok(AuthUser {
            user_id,
            email: claims.email.unwrap_or_default(),
            display_name: claims.name.unwrap_or_default(),
        })
    }
}
```

### Step 3: Implement OptionalAuthUser extractor

- [x] Define and implement `OptionalAuthUser`:

```rust
pub struct OptionalAuthUser(pub Option<AuthUser>);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for OptionalAuthUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        Ok(OptionalAuthUser(AuthUser::from_request_parts(parts, state).await.ok()))
    }
}
```

### Step 4: Update document list endpoint

- [x] Modify `list_documents` in `packages/server/src/documents/api.rs`:

```rust
pub async fn list_documents(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DocumentSummary>>, AppError> {
    // For now, still returns all documents (no permission filtering yet — PR 3)
    let rows = state.db.list_documents().await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let docs: Vec<DocumentSummary> = rows.into_iter().map(Into::into).collect();
    Ok(Json(docs))
}
```

### Step 5: Update document create endpoint

- [x] Modify `create_document` to use `AuthUser` and set `created_by`:

```rust
pub async fn create_document(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateDocumentRequest>,
) -> Result<(StatusCode, Json<DocumentSummary>), AppError> {
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("Title is required".to_string()));
    }

    let id = Uuid::new_v4();
    let row = state.db.create_document(id, body.title.trim(), Some(auth.user_id)).await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Auto-grant Edit permission to the creator
    state.db.create_permission(id, auth.user_id, "edit").await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(row.into())))
}
```

### Step 6: Update remaining document endpoints

- [x] Add `auth: AuthUser` parameter to all remaining document handlers:
  - `get_document`
  - `update_document`
  - `delete_document`
  - `get_content`
  - `push_content`
  - `list_comments`
  - `create_comment`

- [x] No permission logic yet — just require the token is present and valid

### Step 7: Update auth handlers to use the extractor

- [x] Refactor `ws_token` and `me` handlers in `auth/handlers.rs` to use `AuthUser` extractor instead of manually parsing the Authorization header:

```rust
pub async fn ws_token(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<WsTokenResponse>, AppError> {
    let token = jwt::create_ws_token(auth.user_id, &state.config.jwt_secret)?;
    Ok(Json(WsTokenResponse { ws_token: token, expires_in: 30 }))
}

pub async fn me(auth: AuthUser) -> Json<UserProfile> {
    Json(UserProfile {
        id: auth.user_id,
        email: auth.email,
        display_name: auth.display_name,
    })
}
```

### Step 8: Add permission helper to database

- [x] Add `create_permission` method to `Database` in `db.rs`:

```rust
pub async fn create_permission(
    &self,
    document_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO document_permissions (id, document_id, user_id, role) VALUES ($1, $2, $3, $4)"
    )
    .bind(Uuid::new_v4())
    .bind(document_id)
    .bind(user_id)
    .bind(role)
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

### Step 9: Update existing tests

- [x] Update `packages/server/tests/api.rs` — all document API tests now need to:
  1. Register a user first
  2. Include `Authorization: Bearer {token}` header on all requests
  - Add a test helper function to register and get tokens

- [x] Add test: unauthenticated request to `GET /api/docs` → 401
- [x] Add test: request with expired token → 401
- [x] Add test: request with refresh token (wrong type) → 401

## Verification

- [x] `cargo build` succeeds
- [x] All document endpoints return 401 without a valid token
- [x] All document endpoints work with a valid access token
- [x] `POST /api/docs` creates a document with `created_by` set to the authenticated user
- [x] `POST /api/docs` creates an `edit` permission entry for the creator
- [x] Auth endpoints (register, login, refresh) still work without tokens
- [x] Health endpoint works without tokens
- [x] `cargo test` passes all tests (existing tests updated)

## Files Created/Modified

- `packages/server/src/auth/mod.rs` (modified — add middleware module)
- `packages/server/src/auth/middleware.rs` (new)
- `packages/server/src/auth/handlers.rs` (modified — use AuthUser extractor)
- `packages/server/src/documents/api.rs` (modified — add AuthUser to all handlers)
- `packages/server/src/db.rs` (modified — add create_permission, update create_document)
- `packages/server/tests/api.rs` (modified — add auth to all tests)
