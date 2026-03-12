# PR 1: Database Schema & Migrations — Implementation Plan

## Prerequisites

- [ ] Milestone 1 merged and working
- [ ] PostgreSQL 16 running locally (via Docker Compose or native)
- [ ] SQLx CLI installed (`cargo install sqlx-cli --no-default-features --features postgres`)

## Steps

### Step 1: Create the SQLx migration

- [ ] Create directory `packages/server/migrations/`
- [ ] Create `packages/server/migrations/20260312000001_initial.sql`:

```sql
-- Documents table: metadata for each document
CREATE TABLE documents (
    id              UUID PRIMARY KEY,
    title           TEXT NOT NULL,
    schema_version  INTEGER NOT NULL DEFAULT 1,
    snapshot_key    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Document permissions: created now, enforced in Milestone 3
CREATE TABLE document_permissions (
    id              UUID PRIMARY KEY,
    document_id     UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL,
    role            TEXT NOT NULL CHECK (role IN ('read', 'comment', 'edit')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_document_permissions_document_id ON document_permissions(document_id);
CREATE INDEX idx_document_permissions_user_id ON document_permissions(user_id);

-- Update log: append-only log of Yrs updates between snapshot compactions
CREATE TABLE update_log (
    id              BIGSERIAL PRIMARY KEY,
    document_id     UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    data            BYTEA NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_update_log_document_id ON update_log(document_id, id);
```

### Step 2: Add migration runner to server startup

- [ ] In `packages/server/src/main.rs`, add migration execution after database connection:

```rust
sqlx::migrate!().run(&database.pool).await
    .expect("Failed to run database migrations");
```

- [ ] The `migrate!()` macro looks for `migrations/` relative to `CARGO_MANIFEST_DIR`, which is `packages/server/`

### Step 3: Add database helper methods

- [ ] Expand `packages/server/src/db.rs` with query methods for the documents table:

```rust
impl Database {
    // ... existing connect() ...

    pub async fn create_document(&self, id: Uuid, title: &str) -> Result<DocumentRow, sqlx::Error> {
        sqlx::query_as!(
            DocumentRow,
            r#"INSERT INTO documents (id, title) VALUES ($1, $2)
               RETURNING id, title, schema_version, snapshot_key, created_at, updated_at"#,
            id, title
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_document(&self, id: Uuid) -> Result<Option<DocumentRow>, sqlx::Error> {
        sqlx::query_as!(
            DocumentRow,
            "SELECT id, title, schema_version, snapshot_key, created_at, updated_at FROM documents WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_documents(&self) -> Result<Vec<DocumentRow>, sqlx::Error> {
        sqlx::query_as!(
            DocumentRow,
            "SELECT id, title, schema_version, snapshot_key, created_at, updated_at FROM documents ORDER BY updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete_document(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM documents WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_document_snapshot(&self, id: Uuid, snapshot_key: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE documents SET snapshot_key = $2, updated_at = NOW() WHERE id = $1",
            id, snapshot_key
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn append_update_log(&self, document_id: Uuid, data: &[u8]) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "INSERT INTO update_log (document_id, data) VALUES ($1, $2)",
            document_id, data
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_update_log(&self, document_id: Uuid) -> Result<Vec<Vec<u8>>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT data FROM update_log WHERE document_id = $1 ORDER BY id ASC",
            document_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.data).collect())
    }

    pub async fn clear_update_log(&self, document_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM update_log WHERE document_id = $1", document_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
```

- [ ] Add the `DocumentRow` struct:

```rust
pub struct DocumentRow {
    pub id: Uuid,
    pub title: String,
    pub schema_version: i32,
    pub snapshot_key: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

### Step 4: Add LocalStack to Docker Compose

- [ ] Add a `localstack` service to `docker-compose.yml`:

```yaml
  localstack:
    image: localstack/localstack:3
    ports:
      - '4566:4566'
    environment:
      - SERVICES=s3
      - DEFAULT_REGION=us-east-1
```

- [ ] Add `S3_ENDPOINT` to the server service environment:

```yaml
  server:
    environment:
      - S3_ENDPOINT=http://localstack:4566
```

- [ ] Create `scripts/init-localstack.sh` to create the S3 bucket:

```bash
#!/usr/bin/env bash
# Wait for LocalStack to be ready, then create the S3 bucket
set -e
echo "Waiting for LocalStack..."
until aws --endpoint-url=http://localhost:4566 s3 ls 2>/dev/null; do
  sleep 1
done
aws --endpoint-url=http://localhost:4566 s3 mb s3://cadmus-documents 2>/dev/null || true
echo "LocalStack ready, bucket created."
```

- [ ] Make the script executable: `chmod +x scripts/init-localstack.sh`

### Step 5: Update Config for S3 endpoint

- [ ] Add `s3_endpoint` to `Config` in `packages/server/src/config.rs`:

```rust
pub struct Config {
    // ... existing fields ...
    pub s3_endpoint: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            // ... existing fields ...
            s3_endpoint: std::env::var("S3_ENDPOINT").ok(),
        }
    }
}
```

### Step 6: Make database connection required

- [ ] In `packages/server/src/lib.rs`, change `db` from `Option<Database>` to `Database`:

```rust
pub struct AppState {
    pub db: Database,
    // ... rest unchanged
}
```

- [ ] Update `main.rs` accordingly (remove the `Some()` wrapper)

### Step 7: Generate SQLx offline metadata

- [ ] Run the migration against the local database: `cd packages/server && sqlx migrate run`
- [ ] Generate offline query metadata: `cargo sqlx prepare`
- [ ] Commit the generated `.sqlx/` directory (contains cached query metadata for CI builds)

## Verification

- [ ] `docker compose up -d db localstack` starts PostgreSQL and LocalStack
- [ ] `cd packages/server && sqlx migrate run` applies the migration without errors
- [ ] `psql` into the database and verify all three tables exist with correct columns
- [ ] `scripts/init-localstack.sh` creates the S3 bucket
- [ ] `aws --endpoint-url=http://localhost:4566 s3 ls` shows `cadmus-documents`
- [ ] `cargo build` succeeds with the new query macros
- [ ] `cargo test` passes (existing tests unaffected)
- [ ] Server starts and runs migrations automatically on startup

## Files Created/Modified

- `packages/server/migrations/20260312000001_initial.sql` (new)
- `packages/server/src/main.rs` (modified — add migration runner)
- `packages/server/src/db.rs` (modified — add DocumentRow, query methods)
- `packages/server/src/lib.rs` (modified — make db non-optional)
- `packages/server/src/config.rs` (modified — add s3_endpoint)
- `packages/server/.sqlx/` (new — offline query metadata)
- `docker-compose.yml` (modified — add localstack service, S3_ENDPOINT env)
- `scripts/init-localstack.sh` (new)
