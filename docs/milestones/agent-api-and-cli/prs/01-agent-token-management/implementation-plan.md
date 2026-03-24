# PR 1: Agent Token Management — Implementation Plan

## Prerequisites

- [x] Milestone 5 (Comments) is merged

## Steps

### 1. Create the agent_tokens migration

- [x] Create `packages/server/migrations/20260324000001_agent_tokens.sql`:

```sql
CREATE TABLE agent_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            VARCHAR(255) NOT NULL,
    token_hash      VARCHAR(255) NOT NULL,
    scopes          TEXT[] NOT NULL DEFAULT '{}',
    document_ids    UUID[],
    expires_at      TIMESTAMPTZ NOT NULL,
    revoked_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_tokens_user_id ON agent_tokens(user_id);
CREATE INDEX idx_agent_tokens_token_hash ON agent_tokens(token_hash);
```

- [x] Start the dev database (`pnpm dev:infra`) and verify the migration runs on server startup.

### 2. Add agent token data structures

- [x] Create `packages/server/src/auth/tokens.rs` with:
  - `AgentTokenRow` (sqlx::FromRow) — maps to the database row.
  - `AgentTokenResponse` (Serialize) — returned by list endpoint (no hash, no secret).
  - `AgentTokenCreatedResponse` (Serialize) — returned by create endpoint (includes secret).
  - `CreateAgentTokenRequest` (Deserialize) — request body for create.
  - Token generation: `generate_agent_token() -> (String, String)` — returns `(raw_secret, sha256_hash)`. Raw secret format: `cadmus_<32-bytes-hex>`.
  - Duration parsing: `parse_expires_in(s: &str) -> Result<Duration>` — parses "7d", "30d", etc.
  - Scope validation: `validate_scopes(scopes: &[String]) -> Result<()>` — checks against allowed set.

- [x] Add `pub mod tokens;` to `packages/server/src/auth/mod.rs`.

### 3. Add database query methods

- [x] Add to `packages/server/src/db.rs`:
  - `create_agent_token(pool, user_id, name, token_hash, scopes, document_ids, expires_at) -> Result<AgentTokenRow>`
  - `list_agent_tokens(pool, user_id) -> Result<Vec<AgentTokenRow>>` — returns non-revoked, non-expired tokens for the user.
  - `get_agent_token_by_hash(pool, token_hash) -> Result<Option<AgentTokenRow>>` — for auth lookup.
  - `revoke_agent_token(pool, token_id, user_id) -> Result<bool>` — sets `revoked_at`, returns whether row was found.

### 4. Implement token REST handlers

- [x] Add to `packages/server/src/auth/handlers.rs`:

**`create_token`** — extract `AuthUser`, parse `CreateAgentTokenRequest`, validate scopes and expires_in, validate document_ids (if provided, check user has access to each), generate token, hash it, store in DB, return `AgentTokenCreatedResponse` with the raw secret.

**`list_tokens`** — extract `AuthUser`, call `db::list_agent_tokens`, map to `AgentTokenResponse` vec.

**`revoke_token`** — extract `AuthUser` and `token_id` from path, call `db::revoke_agent_token` (checks user_id ownership), return 204.

### 5. Update the router

- [x] In `packages/server/src/lib.rs`, add token routes:

```rust
.route("/api/tokens", post(auth::handlers::create_token))
.route("/api/tokens", get(auth::handlers::list_tokens))
.route("/api/tokens/{token_id}", delete(auth::handlers::revoke_token))
```

### 6. Extend auth middleware for dual auth

- [x] In `packages/server/src/auth/middleware.rs`:

Update `AuthUser` struct:

```rust
pub struct AuthUser {
    pub user_id: Uuid,
    pub is_agent: bool,
    pub agent_name: Option<String>,
    pub token_scopes: Option<Vec<String>>,
}
```

Update `FromRequestParts` implementation:

1. Read `Authorization: Bearer <value>`.
2. If value starts with `cadmus_`: hash it, look up via `db::get_agent_token_by_hash`, verify not revoked and not expired, construct `AuthUser` with `is_agent: true`.
3. Otherwise: validate as JWT (existing logic), construct `AuthUser` with `is_agent: false`.

- [x] The middleware needs access to the database pool. Update `AppState` if needed — the `AuthUser` extractor currently only needs `Config` (for JWT secret). Now it also needs `Database` for agent token lookups. The extractor already has access to `AppState` via `State`.

### 7. Add scope enforcement helper

- [x] Create a helper function `require_scope(auth: &AuthUser, scope: &str) -> Result<()>`:
  - If `auth.token_scopes` is None (JWT user), return Ok (users have all scopes).
  - If `auth.token_scopes` contains the required scope, return Ok.
  - Otherwise, return 403.

- [x] Add `require_scope` calls to existing handlers where needed:
  - Document read handlers: require `docs:read`.
  - Document write handlers: require `docs:write`.
  - Comment read handlers: require `comments:read`.
  - Comment write handlers: require `comments:write`.

### 8. Add document ID restriction enforcement

- [x] Create a helper function `check_document_restriction(auth: &AuthUser, token_row: &AgentTokenRow, doc_id: Uuid) -> Result<()>`:
  - If the token has no `document_ids` restriction (NULL), return Ok.
  - If `document_ids` contains `doc_id`, return Ok.
  - Otherwise, return 403.

- [x] Integrate this check into the document permission check flow — it should run alongside `require_permission` for agent-authenticated requests.

### 9. Update WebSocket handler for agent tokens

- [x] In `packages/server/src/websocket/handler.rs`:

The `ws_upgrade` handler reads `token` from the query string and validates it as a ws-token JWT. Extend this to also handle agent tokens:

1. If the token starts with `cadmus_`: validate as agent token (same logic as the middleware).
2. Otherwise: validate as ws-token JWT (existing logic).

Set awareness state with `is_agent` and `agent_name` fields from the auth result.

### 10. Test the endpoints

- [x] Start the full dev stack: `pnpm dev`
- [x] Register a user and log in to get an access token.
- [x] Test token CRUD:

```bash
# Create a token
curl -X POST http://localhost:8080/api/tokens \
  -H 'Authorization: Bearer <jwt>' \
  -H 'Content-Type: application/json' \
  -d '{"name": "test-agent", "scopes": ["docs:read", "docs:write"], "expires_in": "30d"}'

# List tokens
curl http://localhost:8080/api/tokens \
  -H 'Authorization: Bearer <jwt>'

# Use the agent token to access docs
curl http://localhost:8080/api/docs \
  -H 'Authorization: Bearer cadmus_...'

# Revoke the token
curl -X DELETE http://localhost:8080/api/tokens/{token_id} \
  -H 'Authorization: Bearer <jwt>'

# Verify revoked token is rejected
curl http://localhost:8080/api/docs \
  -H 'Authorization: Bearer cadmus_...'
# Should return 401
```

- [x] Verify scope enforcement: a token with only `docs:read` gets 403 on write operations.
- [x] Verify document_ids restriction: a token restricted to doc A gets 403 when accessing doc B.
- [x] Verify expired tokens are rejected.

### 11. Build and format check

- [x] Run `cargo build` in `packages/server/` — compiles without errors.
- [x] Run `cargo test` in `packages/server/` — all tests pass.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [x] Migration creates the `agent_tokens` table
- [x] `POST /api/tokens` returns token_id and secret
- [x] Secret starts with `cadmus_` prefix
- [x] `GET /api/tokens` lists non-revoked, non-expired tokens
- [x] `DELETE /api/tokens/{id}` revokes the token (204)
- [x] Revoked tokens are rejected with 401
- [x] Expired tokens are rejected with 401
- [x] Agent tokens work as Bearer tokens on all REST endpoints
- [x] Agent tokens work as query parameter on WebSocket upgrade
- [x] Scope enforcement: `docs:read`-only token can't push content
- [x] Document ID restriction: restricted token can't access unrestricted docs
- [x] Existing JWT auth continues to work unchanged
- [x] Invalid scope strings are rejected at creation (400)
- [x] Invalid expires_in formats are rejected (400)

## Files Modified

| File                                                         | Change                                         |
| ------------------------------------------------------------ | ---------------------------------------------- |
| `packages/server/migrations/20260324000001_agent_tokens.sql` | New: agent_tokens table migration              |
| `packages/server/src/auth/tokens.rs`                         | New: token data structures and generation      |
| `packages/server/src/auth/mod.rs`                            | Add `pub mod tokens`                           |
| `packages/server/src/auth/middleware.rs`                     | Extend AuthUser, dual auth (JWT + agent token) |
| `packages/server/src/auth/handlers.rs`                       | Add token CRUD handlers                        |
| `packages/server/src/db.rs`                                  | Add token query methods                        |
| `packages/server/src/lib.rs`                                 | Add token routes                               |
| `packages/server/src/websocket/handler.rs`                   | Accept agent tokens on WS upgrade              |
| `packages/server/src/documents/api.rs`                       | Add scope enforcement to handlers              |
