# PR 2: JWT Middleware & Auth Extractors

## Purpose

Add Axum middleware that validates JWT access tokens and makes the authenticated user available to handlers. After this PR, any handler can require authentication by adding an `AuthUser` extractor to its signature. Existing document endpoints are updated to require auth but don't yet enforce permissions — that's PR 3.

This PR is the bridge between "auth endpoints exist" (PR 1) and "permissions are enforced" (PR 3).

## Auth Extractor

The primary interface is an Axum extractor that handlers use to require authentication:

```rust
/// Authenticated user extracted from a valid JWT.
pub struct AuthUser {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
}
```

Usage in a handler:

```rust
pub async fn list_documents(
    auth: AuthUser,  // ← request rejected with 401 if no valid token
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DocumentSummary>>, AppError> {
    // auth.user_id is available
}
```

### Implementation

`AuthUser` implements Axum's `FromRequestParts` trait:

1. Extract the `Authorization` header.
2. Verify it starts with `Bearer `.
3. Decode and validate the JWT using `jsonwebtoken::decode` with the `JWT_SECRET` from `AppState.config`.
4. Verify the `type` claim is `"access"` (reject refresh/ws tokens used as access tokens).
5. Return `AuthUser` with the claims, or `AppError::Unauthorized` on any failure.

### Optional Auth

Some future endpoints may need optional auth (e.g., public document viewing). For this, we provide:

```rust
pub struct OptionalAuthUser(pub Option<AuthUser>);
```

This extractor never rejects — it returns `None` if no valid token is present. Not used in this milestone but included for forward compatibility.

## Applying Auth to Existing Routes

All document endpoints now require authentication. The handler signatures change to include `AuthUser`:

| Endpoint                       | Before (M2) | After (M3 PR 2)                        |
| ------------------------------ | ----------- | -------------------------------------- |
| `GET /api/docs`                | No auth     | Requires `AuthUser`                    |
| `POST /api/docs`               | No auth     | Requires `AuthUser`, sets `created_by` |
| `GET /api/docs/{id}`           | No auth     | Requires `AuthUser`                    |
| `PATCH /api/docs/{id}`         | No auth     | Requires `AuthUser`                    |
| `DELETE /api/docs/{id}`        | No auth     | Requires `AuthUser`                    |
| `GET /api/docs/{id}/content`   | No auth     | Requires `AuthUser`                    |
| `POST /api/docs/{id}/content`  | No auth     | Requires `AuthUser`                    |
| `GET /api/docs/{id}/comments`  | No auth     | Requires `AuthUser`                    |
| `POST /api/docs/{id}/comments` | No auth     | Requires `AuthUser`                    |

**Important:** In this PR, auth is required but permissions are not checked. Any authenticated user can access any document. Permission checks (is this user allowed to access _this_ document?) come in PR 3.

### Create Document Change

`POST /api/docs` now sets the `created_by` column (added in PR 1's migration) to the authenticated user's ID:

```rust
let row = state.db.create_document(id, body.title.trim(), auth.user_id).await?;
```

The `create_document` DB method gains an `owner_id` parameter.

## Route Organization

Auth endpoints from PR 1 are excluded from the auth middleware (they're the endpoints that _issue_ tokens). The router is organized:

```rust
Router::new()
    // Public routes (no auth required)
    .route("/health", get(health))
    .route("/api/auth/register", post(auth::handlers::register))
    .route("/api/auth/login", post(auth::handlers::login))
    .route("/api/auth/refresh", post(auth::handlers::refresh))
    // Protected routes (auth required via AuthUser extractor)
    .route("/api/auth/ws-token", post(auth::handlers::ws_token))
    .route("/api/auth/me", get(auth::handlers::me))
    .route("/api/docs", get(documents::api::list_documents))
    // ... etc
```

Note: We don't use a layer-based middleware approach. The `AuthUser` extractor is per-handler — handlers that need auth include it, handlers that don't (register, login, refresh, health) simply omit it. This is simpler and more explicit than a middleware layer with route exclusions.

## Error Responses

| Scenario                                        | Status | Body                                          |
| ----------------------------------------------- | ------ | --------------------------------------------- |
| No Authorization header                         | 401    | `{ "error": "Missing authorization header" }` |
| Malformed header                                | 401    | `{ "error": "Invalid authorization header" }` |
| Invalid/expired JWT                             | 401    | `{ "error": "Invalid or expired token" }`     |
| Wrong token type (e.g., refresh used as access) | 401    | `{ "error": "Invalid or expired token" }`     |

All auth failures return the same generic 401 message (after the initial parse) to avoid leaking information about why specifically the token was rejected.

## What's Not Included

- **Permission checks** — authenticated users can access any document. PR 3 adds per-document permission enforcement.
- **WebSocket auth** — the WS upgrade handler still accepts unauthenticated connections. PR 3 adds token validation on upgrade.
- **Frontend changes** — the frontend doesn't send auth headers yet. PR 4 adds the auth UI and token management.
