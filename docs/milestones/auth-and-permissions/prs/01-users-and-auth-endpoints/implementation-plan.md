# PR 1: Users & Auth Endpoints — Implementation Plan

## Prerequisites

- [x] Milestone 2 merged and working
- [ ] PostgreSQL running with M2 migrations applied
- [ ] `argon2` crate available on crates.io

## Steps

### Step 1: Add new dependencies to Cargo.toml

- [ ] Add `argon2 = "0.5"` to `packages/server/Cargo.toml`
- [ ] Add `rand = "0.8"` if not already a transitive dependency (needed for salt generation)
- [ ] Verify `jsonwebtoken = "9"` is already present (it is)

### Step 2: Create the users migration

- [ ] Create `packages/server/migrations/20260312000002_users.sql`:

```sql
-- Users table
CREATE TABLE users (
    id              UUID PRIMARY KEY,
    email           TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    password_hash   TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);

-- Add owner tracking to documents
ALTER TABLE documents ADD COLUMN created_by UUID REFERENCES users(id);

-- Add FK constraint to document_permissions (column exists but had no FK)
ALTER TABLE document_permissions
    ADD CONSTRAINT fk_document_permissions_user_id
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
```

### Step 3: Add user database methods

- [ ] Add `UserRow` struct to `packages/server/src/db.rs`:

```rust
#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] Add user query methods to `Database`:

```rust
pub async fn create_user(&self, id: Uuid, email: &str, display_name: &str, password_hash: &str) -> Result<UserRow, sqlx::Error>;
pub async fn get_user_by_id(&self, id: Uuid) -> Result<Option<UserRow>, sqlx::Error>;
pub async fn get_user_by_email(&self, email: &str) -> Result<Option<UserRow>, sqlx::Error>;
```

- [ ] Update `create_document` to accept an optional `created_by: Option<Uuid>` parameter

### Step 4: Create the password hashing module

- [ ] Create `packages/server/src/auth/mod.rs` with `pub mod password; pub mod jwt; pub mod handlers;`
- [ ] Create `packages/server/src/auth/password.rs`:

```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::rngs::OsRng;

use crate::errors::AppError;

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("Password hashing failed: {}", e)))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(format!("Invalid password hash: {}", e)))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok())
}
```

### Step 5: Create the JWT module

- [ ] Create `packages/server/src/auth/jwt.rs`:

```rust
use chrono::Utc;
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,         // user ID
    pub email: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub token_type: String,  // "access", "refresh", "ws"
    pub iat: i64,
    pub exp: i64,
}

const ACCESS_TOKEN_EXPIRY_SECS: i64 = 900;      // 15 minutes
const REFRESH_TOKEN_EXPIRY_SECS: i64 = 604800;  // 7 days
const WS_TOKEN_EXPIRY_SECS: i64 = 30;           // 30 seconds

pub fn create_access_token(user_id: Uuid, email: &str, name: &str, secret: &str) -> Result<String, AppError>;
pub fn create_refresh_token(user_id: Uuid, secret: &str) -> Result<String, AppError>;
pub fn create_ws_token(user_id: Uuid, secret: &str) -> Result<String, AppError>;
pub fn validate_token(token: &str, expected_type: &str, secret: &str) -> Result<Claims, AppError>;
```

- [ ] Implement all four functions using `jsonwebtoken::encode` / `decode`
- [ ] `validate_token` checks both signature validity and the `type` claim

### Step 6: Create auth request/response types

- [ ] In `packages/server/src/auth/handlers.rs`, define:

```rust
#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user: UserProfile,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Serialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
}

#[derive(Serialize)]
pub struct WsTokenResponse {
    pub ws_token: String,
    pub expires_in: u64,
}
```

### Step 7: Implement auth handlers

- [ ] Implement `register` handler:
  - Validate email (contains `@`, trim, lowercase), display_name (non-empty), password (≥8 chars)
  - Check for existing user with same email → 409 Conflict
  - Hash password with `auth::password::hash_password`
  - Insert user via `db.create_user`
  - Issue access + refresh tokens
  - Return `AuthResponse` with 201 status

- [ ] Implement `login` handler:
  - Look up user by email (lowercased, trimmed)
  - If not found → 401 "Invalid email or password"
  - Verify password → if mismatch, 401 "Invalid email or password"
  - Issue access + refresh tokens
  - Return `AuthResponse`

- [ ] Implement `refresh` handler:
  - Validate refresh token (type must be "refresh")
  - Look up user by ID from claims (ensures user still exists)
  - Issue new access token only (refresh token stays the same)
  - Return `TokenResponse`

- [ ] Implement `ws_token` handler:
  - This handler requires an access token (will be enforced via extractor in PR 2; for now, manually parse the Authorization header)
  - Issue a ws-token for the authenticated user
  - Return `WsTokenResponse`

- [ ] Implement `me` handler:
  - Parse access token from Authorization header
  - Look up user, return `UserProfile`

### Step 8: Register auth routes

- [ ] In `packages/server/src/lib.rs`:
  - Add `pub mod auth;`
  - Add routes:

```rust
.route("/api/auth/register", post(auth::handlers::register))
.route("/api/auth/login", post(auth::handlers::login))
.route("/api/auth/refresh", post(auth::handlers::refresh))
.route("/api/auth/ws-token", post(auth::handlers::ws_token))
.route("/api/auth/me", get(auth::handlers::me))
```

### Step 9: Tests

- [ ] Write unit tests for password hashing in `auth/password.rs`:
  - Hash and verify a valid password
  - Verify returns false for wrong password
  - Hash produces different output for same input (salt is random)

- [ ] Write unit tests for JWT in `auth/jwt.rs`:
  - Create and validate an access token
  - Create and validate a refresh token
  - Reject an expired token
  - Reject a token with wrong type (use refresh token as access)

- [ ] Write integration tests in `packages/server/tests/auth.rs`:
  - Register a new user → 201 with valid tokens
  - Register with duplicate email → 409
  - Register with invalid fields (missing email, short password) → 400
  - Login with valid credentials → 200 with tokens
  - Login with wrong password → 401
  - Login with nonexistent email → 401
  - Refresh with valid refresh token → 200 with new access token
  - Refresh with invalid token → 401
  - `GET /api/auth/me` with valid token → 200 with user profile
  - `GET /api/auth/me` without token → 401

## Verification

- [ ] `cargo build` succeeds with new dependencies
- [ ] `sqlx migrate run` applies the users migration
- [ ] `psql` shows `users` table with correct columns
- [ ] `psql` shows FK constraint on `document_permissions.user_id`
- [ ] `psql` shows `created_by` column on `documents` table
- [ ] Registration creates a user in the database with a hashed password
- [ ] Login returns valid JWTs
- [ ] Refresh extends session without re-authentication
- [ ] `cargo test` passes all new and existing tests
- [ ] Existing document endpoints still work (no auth enforced yet)

## Files Created/Modified

- `packages/server/Cargo.toml` (modified — add `argon2`, `rand`)
- `packages/server/migrations/20260312000002_users.sql` (new)
- `packages/server/src/auth/mod.rs` (new)
- `packages/server/src/auth/password.rs` (new)
- `packages/server/src/auth/jwt.rs` (new)
- `packages/server/src/auth/handlers.rs` (new)
- `packages/server/src/db.rs` (modified — add UserRow, user queries, update create_document)
- `packages/server/src/lib.rs` (modified — add auth module, auth routes)
- `packages/server/tests/auth.rs` (new)
