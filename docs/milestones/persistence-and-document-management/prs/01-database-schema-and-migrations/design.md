# PR 1: Database Schema & Migrations

## Purpose

Establish the PostgreSQL schema that Cadmus will use for document metadata, permissions, and the update log. Set up SQLx migrations so the schema is version-controlled and applied automatically on server startup. Extend Docker Compose with LocalStack for local S3 emulation.

This PR creates the foundation that PRs 2 and 3 both build on. It's purely infrastructure — no application logic changes.

## Database Schema

### `documents` Table

Stores document metadata. One row per document.

| Column           | Type                   | Notes                                                                           |
| ---------------- | ---------------------- | ------------------------------------------------------------------------------- |
| `id`             | `UUID PRIMARY KEY`     | Generated client-side (UUIDv4)                                                  |
| `title`          | `TEXT NOT NULL`        | Display name, user-editable                                                     |
| `schema_version` | `INTEGER NOT NULL`     | ProseMirror schema version (currently 1)                                        |
| `snapshot_key`   | `TEXT`                 | S3 object key for the latest compacted Yrs snapshot, nullable until first flush |
| `created_at`     | `TIMESTAMPTZ NOT NULL` | `DEFAULT NOW()`                                                                 |
| `updated_at`     | `TIMESTAMPTZ NOT NULL` | `DEFAULT NOW()`, updated on every flush                                         |

### `document_permissions` Table

Created now but not enforced until Milestone 3 (Auth). Having the table in place means the schema migration is already done when we add auth.

| Column        | Type                   | Notes                                                                     |
| ------------- | ---------------------- | ------------------------------------------------------------------------- |
| `id`          | `UUID PRIMARY KEY`     | Permission row ID                                                         |
| `document_id` | `UUID NOT NULL`        | FK → `documents(id) ON DELETE CASCADE`                                    |
| `user_id`     | `UUID NOT NULL`        | FK → `users(id)` (deferred — no users table yet, so stored as plain UUID) |
| `role`        | `TEXT NOT NULL`        | `'read'`, `'comment'`, or `'edit'`                                        |
| `created_at`  | `TIMESTAMPTZ NOT NULL` | `DEFAULT NOW()`                                                           |

The `user_id` column references a `users` table that doesn't exist yet. We'll create it as a plain UUID column with no FK constraint for now and add the constraint in Milestone 3.

### `update_log` Table

Append-only log of Yrs updates between snapshot compactions. Used for crash recovery — replay these on top of the latest snapshot to reconstruct the full document state.

| Column        | Type                    | Notes                                  |
| ------------- | ----------------------- | -------------------------------------- |
| `id`          | `BIGSERIAL PRIMARY KEY` | Auto-incrementing for ordering         |
| `document_id` | `UUID NOT NULL`         | FK → `documents(id) ON DELETE CASCADE` |
| `data`        | `BYTEA NOT NULL`        | Raw Yrs update binary                  |
| `created_at`  | `TIMESTAMPTZ NOT NULL`  | `DEFAULT NOW()`                        |

Index: `(document_id, id)` for efficient range queries during document load.

### Why No `users` Table Yet

Milestone 3 introduces authentication and the users table. Adding it now would create dead infrastructure that we'd need to seed/manage without any benefit. The `document_permissions.user_id` is typed as UUID without a FK constraint — it's a placeholder column.

## SQLx Migration Setup

We use SQLx's built-in migration system:

- Migration files live in `packages/server/migrations/`
- File naming: `YYYYMMDDHHMMSS_description.sql` (SQLx convention)
- Migrations run automatically on startup via `sqlx::migrate!().run(&pool).await`
- For CI, we use SQLx's offline mode with `sqlx-data.json` (compile-time query checking against cached metadata)

The initial migration creates all three tables in a single file. This is fine because they have no independent lifecycle — they're all part of the same schema version.

## LocalStack for S3

Development needs an S3-compatible object store without requiring AWS credentials. LocalStack provides this:

- Added as a service in `docker-compose.yml`
- Exposes S3 on `localhost:4566`
- The server's S3 client is configured with a custom endpoint URL when `S3_ENDPOINT` env var is set (LocalStack in dev, omitted in production to use real AWS)
- A startup script creates the `cadmus-documents` bucket in LocalStack

## Environment Variables

| Variable       | Default                       | Notes                                         |
| -------------- | ----------------------------- | --------------------------------------------- |
| `DATABASE_URL` | `postgres://localhost/cadmus` | Existing, unchanged                           |
| `S3_ENDPOINT`  | (none)                        | Set to `http://localhost:4566` for LocalStack |
| `S3_BUCKET`    | `cadmus-documents`            | Existing, unchanged                           |

## Error Handling

- Migration failures on startup cause the server to exit with a clear error message. This is intentional — a schema mismatch is not recoverable at runtime.
- If the LocalStack container isn't running, the server still starts (S3 operations will fail lazily when persistence is attempted in PR 2).
