# PR 1: Agent Token Management

## Purpose

Create the `agent_tokens` table, implement token CRUD endpoints (create, list, revoke), and extend the auth middleware to accept agent tokens alongside user JWTs. After this PR, non-browser clients can authenticate with a bearer token and access all existing REST and WebSocket endpoints — they just can't push content yet (that's PR 2).

## Database Schema

```sql
CREATE TABLE agent_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            VARCHAR(255) NOT NULL,
    token_hash      VARCHAR(255) NOT NULL,
    scopes          TEXT[] NOT NULL DEFAULT '{}',
    document_ids    UUID[],                         -- NULL = all documents
    expires_at      TIMESTAMPTZ NOT NULL,
    revoked_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_tokens_user_id ON agent_tokens(user_id);
CREATE INDEX idx_agent_tokens_token_hash ON agent_tokens(token_hash);
```

- `token_hash` stores a SHA-256 hash of the token secret. The raw secret is returned once at creation time and never stored.
- `scopes` is an array of permission strings: `docs:read`, `docs:write`, `comments:read`, `comments:write`.
- `document_ids` is NULL for tokens that can access all documents the user has permissions on. When set, restricts the token to only those document IDs.
- `revoked_at` is NULL for active tokens. Setting it revokes the token without deleting the row (preserves audit trail).
- `expires_at` is always set. The creation endpoint accepts an `expires_in` string (e.g., `"7d"`, `"30d"`, `"90d"`) and computes the absolute timestamp.

## Token Format

Agent tokens use the format `cadmus_<random-32-bytes-hex>` (e.g., `cadmus_a1b2c3d4...`). This prefix makes tokens visually distinguishable from JWTs and enables the auth middleware to route them correctly without parsing.

## Data Structures

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AgentTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub token_hash: String,
    pub scopes: Vec<String>,
    pub document_ids: Option<Vec<Uuid>>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AgentTokenResponse {
    pub id: Uuid,
    pub name: String,
    pub scopes: Vec<String>,
    pub document_ids: Option<Vec<Uuid>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AgentTokenCreatedResponse {
    pub token_id: Uuid,
    pub secret: String,      // shown once, never again
    pub name: String,
    pub scopes: Vec<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentTokenRequest {
    pub name: String,
    pub scopes: Vec<String>,
    pub document_ids: Option<Vec<Uuid>>,
    pub expires_in: String,  // "7d", "30d", "90d"
}
```

## REST Endpoints

### Create token

```
POST /api/tokens
{
    "name": "my-coding-agent",
    "scopes": ["docs:read", "docs:write", "comments:write"],
    "document_ids": null,
    "expires_in": "30d"
}
→ {
    "token_id": "...",
    "secret": "cadmus_...",
    "name": "my-coding-agent",
    "scopes": ["docs:read", "docs:write", "comments:write"],
    "expires_at": "..."
}
```

Generates a random token secret, hashes it, stores the hash, returns the raw secret once.

**Permission:** Authenticated user (any user can create tokens for themselves).

**Validation:**

- `name` must be non-empty and ≤255 characters.
- `scopes` must be a non-empty subset of valid scopes (`docs:read`, `docs:write`, `comments:read`, `comments:write`).
- `expires_in` must parse to a valid duration (supported: `"1d"` through `"365d"`).
- If `document_ids` is provided, each ID must reference a document the user currently has access to.

### List tokens

```
GET /api/tokens
→ [
    {
        "id": "...",
        "name": "my-coding-agent",
        "scopes": ["docs:read", "docs:write"],
        "document_ids": null,
        "expires_at": "...",
        "created_at": "..."
    }
]
```

Returns all non-revoked, non-expired tokens for the authenticated user. Does not return revoked tokens.

**Permission:** Authenticated user.

### Revoke token

```
DELETE /api/tokens/{token_id}
```

Sets `revoked_at` to NOW(). Returns 204 No Content.

**Permission:** Authenticated user, must own the token.

## Auth Middleware Changes

The `AuthUser` extractor in `middleware.rs` currently:

1. Reads the `Authorization: Bearer <token>` header.
2. Validates it as a JWT access token.

After this PR, it also handles agent tokens:

1. Read the `Authorization: Bearer <value>` header.
2. If `<value>` starts with `cadmus_`, treat it as an agent token:
   a. Hash the token value.
   b. Look up the hash in `agent_tokens`.
   c. Verify: not revoked, not expired.
   d. Construct `AuthUser` with `user_id` from the token row, `is_agent: true`, `agent_name` from the token's `name` field, and `token_scopes` for scope enforcement.
3. Otherwise, validate as a JWT (existing behavior).

The `AuthUser` struct gains optional agent fields:

```rust
pub struct AuthUser {
    pub user_id: Uuid,
    pub is_agent: bool,
    pub agent_name: Option<String>,
    pub token_scopes: Option<Vec<String>>,
}
```

Scope enforcement is checked at the handler level — handlers that require `docs:write` verify the token includes that scope (user JWTs have all scopes implicitly).

### WebSocket token support

For WebSocket connections, agent tokens are passed as `?token=cadmus_...` (same query parameter as ws-tokens). The `ws_upgrade` handler already reads this parameter — it just needs to route to the agent token path when the value starts with `cadmus_`.

## Scope Enforcement

Scopes restrict what an agent token can do, layered on top of the user's existing document permissions:

| Scope            | Allows                                      |
| ---------------- | ------------------------------------------- |
| `docs:read`      | GET document metadata, GET document content |
| `docs:write`     | POST/PATCH documents, POST content (push)   |
| `comments:read`  | GET comments                                |
| `comments:write` | POST/PUT comments, resolve/unresolve        |

A token with `docs:read` but not `docs:write` can read documents but not push changes, even if the user has Edit permission on the document.

## Document ID Restrictions

When `document_ids` is set on a token, every document-scoped request checks that the target document ID is in the allowlist. This check happens in addition to the normal permission check.

## Error Cases

| Scenario                                        | Response         |
| ----------------------------------------------- | ---------------- |
| Invalid scope string                            | 400 Bad Request  |
| Invalid expires_in format                       | 400 Bad Request  |
| Empty name                                      | 400 Bad Request  |
| Document ID in list user doesn't have access to | 403 Forbidden    |
| Token expired                                   | 401 Unauthorized |
| Token revoked                                   | 401 Unauthorized |
| Token's document_ids doesn't include target doc | 403 Forbidden    |
| Token missing required scope                    | 403 Forbidden    |
| Revoking someone else's token                   | 404 Not Found    |

## What's Not Included

- Rate limiting per token (M7)
- Organization-level token policies (M7)
- Audit logging of token usage (M7)
- Token refresh/rotation (not needed — create a new token and revoke the old one)
