# PR 1: Users & Auth Endpoints

## Purpose

Establish the user model and authentication endpoints. After this PR, users can register with email/password, log in to receive JWTs, refresh expiring tokens, and obtain short-lived WebSocket tokens. No endpoints are _protected_ yet — that comes in PR 2. This PR builds the auth infrastructure that everything else depends on.

## Database Schema

### `users` Table

| Column          | Type                   | Notes                                                       |
| --------------- | ---------------------- | ----------------------------------------------------------- |
| `id`            | `UUID PRIMARY KEY`     | Generated server-side (UUIDv4)                              |
| `email`         | `TEXT NOT NULL UNIQUE` | Lowercase, trimmed. Used for login and sharing invitations. |
| `display_name`  | `TEXT NOT NULL`        | Shown in awareness/cursors and sharing UI                   |
| `password_hash` | `TEXT NOT NULL`        | Argon2id hash                                               |
| `created_at`    | `TIMESTAMPTZ NOT NULL` | `DEFAULT NOW()`                                             |
| `updated_at`    | `TIMESTAMPTZ NOT NULL` | `DEFAULT NOW()`                                             |

### Migration: `20260312000002_users.sql`

1. Create the `users` table.
2. Add a FK constraint from `document_permissions.user_id` to `users(id)`. The column already exists (created in M2's initial migration as a plain UUID); this migration adds the constraint.
3. Add `created_by UUID REFERENCES users(id)` column to the `documents` table. Nullable for backward compatibility with documents created before auth existed.

### Why Argon2id

Argon2id is the current OWASP recommendation for password hashing. It's memory-hard (resistant to GPU attacks) and available via the `argon2` Rust crate. We use default parameters from the crate, which target ~1 second hashing time on modern hardware.

## Auth Endpoints

### `POST /api/auth/register`

Creates a new user account.

Request:

```json
{
  "email": "alice@example.com",
  "display_name": "Alice",
  "password": "securepassword123"
}
```

Response (201):

```json
{
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "alice@example.com",
    "display_name": "Alice"
  },
  "access_token": "eyJhbG...",
  "refresh_token": "eyJhbG...",
  "expires_in": 900
}
```

Validation:

- Email must be non-empty, contain `@`, lowercased and trimmed.
- Display name must be non-empty (trimmed, 1–100 chars).
- Password must be at least 8 characters.
- Duplicate email → 409 Conflict.

Registration immediately returns tokens so the user doesn't need a separate login step.

### `POST /api/auth/login`

Authenticates an existing user.

Request:

```json
{
  "email": "alice@example.com",
  "password": "securepassword123"
}
```

Response (200):

```json
{
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "alice@example.com",
    "display_name": "Alice"
  },
  "access_token": "eyJhbG...",
  "refresh_token": "eyJhbG...",
  "expires_in": 900
}
```

Invalid credentials → 401 with generic "Invalid email or password" message (don't reveal whether the email exists).

### `POST /api/auth/refresh`

Exchanges a valid refresh token for a new access token.

Request:

```json
{
  "refresh_token": "eyJhbG..."
}
```

Response (200):

```json
{
  "access_token": "eyJhbG...",
  "expires_in": 900
}
```

Invalid or expired refresh token → 401.

### `POST /api/auth/ws-token`

Issues a short-lived, single-use JWT for WebSocket authentication. Requires a valid access token in the `Authorization` header.

Response (200):

```json
{
  "ws_token": "eyJhbG...",
  "expires_in": 30
}
```

The ws-token includes the user's ID and is valid for 30 seconds. It's passed as a query parameter on the WebSocket URL: `wss://host/api/docs/{id}/ws?token={ws_token}`.

### `GET /api/auth/me`

Returns the current user's profile. Requires a valid access token.

Response (200):

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "email": "alice@example.com",
  "display_name": "Alice"
}
```

This endpoint is used by the frontend to validate a stored token on page load.

## JWT Structure

### Access Token Claims

```json
{
  "sub": "user-uuid",
  "email": "alice@example.com",
  "name": "Alice",
  "type": "access",
  "iat": 1710000000,
  "exp": 1710000900
}
```

- Signed with HS256 using `JWT_SECRET` (already in config).
- 15-minute expiry.

### Refresh Token Claims

```json
{
  "sub": "user-uuid",
  "type": "refresh",
  "iat": 1710000000,
  "exp": 1710604800
}
```

- 7-day expiry.
- Minimal claims (no email/name — these are looked up fresh on refresh).

### WebSocket Token Claims

```json
{
  "sub": "user-uuid",
  "type": "ws",
  "iat": 1710000000,
  "exp": 1710000030
}
```

- 30-second expiry.
- Validated by the WebSocket upgrade handler (PR 3).

## Password Hashing Module

A thin wrapper around the `argon2` crate:

```rust
pub fn hash_password(password: &str) -> Result<String, AppError>;
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError>;
```

Uses `Argon2::default()` which provides Argon2id with OWASP-recommended parameters.

## Response Types

```rust
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

## Error Handling

| Scenario               | Status Code | Response Body                                           |
| ---------------------- | ----------- | ------------------------------------------------------- |
| Duplicate email        | 409         | `{ "error": "Email already registered" }`               |
| Invalid credentials    | 401         | `{ "error": "Invalid email or password" }`              |
| Missing/invalid fields | 400         | `{ "error": "..." }` (specific field message)           |
| Invalid refresh token  | 401         | `{ "error": "Invalid or expired token" }`               |
| Password too short     | 400         | `{ "error": "Password must be at least 8 characters" }` |

## What's Not Included

- **Route protection** — auth endpoints work, but existing document endpoints remain unprotected. PR 2 adds middleware.
- **Permission enforcement** — deferred to PR 3.
- **Frontend login UI** — deferred to PR 4.
- **OAuth/SSO** — deferred to M7. The JWT layer is designed to support it (OAuth would just be a different way to create the initial tokens).
- **Password reset** — deferred. Can be added without schema changes.
- **Email verification** — deferred. Can be added with a `verified_at` column later.

## Dependencies

### New Rust Crates

| Crate    | Purpose                                      |
| -------- | -------------------------------------------- |
| `argon2` | Argon2id password hashing                    |
| `rand`   | Salt generation (if not pulled transitively) |

The `jsonwebtoken` crate is already in `Cargo.toml`. `uuid`, `chrono`, `serde` are already present.
