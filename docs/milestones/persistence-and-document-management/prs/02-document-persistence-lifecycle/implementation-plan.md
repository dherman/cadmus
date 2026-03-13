# PR 2: Document Persistence Lifecycle — Implementation Plan

## Prerequisites

- [x] PR 1 (Database Schema & Migrations) merged
- [x] PostgreSQL running with migrations applied
- [x] LocalStack running with `cadmus-documents` bucket created

## Steps

### Step 1: Create the S3 storage module

- [x] Create `packages/server/src/documents/storage.rs`
- [x] Implement the S3 client wrapper:

```rust
use aws_sdk_s3::Client as S3Client;

pub struct SnapshotStorage {
    client: S3Client,
    bucket: String,
}

impl SnapshotStorage {
    pub async fn new(bucket: &str, endpoint: Option<&str>) -> Self {
        let mut config_loader = aws_config::from_env();
        if let Some(endpoint) = endpoint {
            config_loader = config_loader.endpoint_url(endpoint);
        }
        let config = config_loader.load().await;
        let client = S3Client::new(&config);
        Self { client, bucket: bucket.to_string() }
    }

    pub async fn upload_snapshot(&self, doc_id: Uuid, data: &[u8]) -> Result<String, ...> {
        let key = format!("snapshots/{}/latest.yrs", doc_id);
        self.client.put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(data.to_vec().into())
            .send()
            .await?;
        Ok(key)
    }

    pub async fn download_snapshot(&self, key: &str) -> Result<Option<Vec<u8>>, ...> {
        // GET object, return None on NoSuchKey
    }

    pub async fn delete_snapshot(&self, doc_id: Uuid) -> Result<(), ...> {
        let key = format!("snapshots/{}/latest.yrs", doc_id);
        self.client.delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await?;
        Ok(())
    }
}
```

- [x] Add `pub mod storage;` to `packages/server/src/documents/mod.rs`

### Step 2: Add SnapshotStorage to AppState

- [x] In `packages/server/src/lib.rs`, add storage to `AppState`:

```rust
pub struct AppState {
    pub db: Database,
    pub document_sessions: SessionManager,
    pub storage: documents::storage::SnapshotStorage,
    pub sidecar: SidecarClient,
    pub config: Config,
}
```

- [x] In `main.rs`, initialize `SnapshotStorage` with config values:

```rust
let storage = SnapshotStorage::new(
    &cfg.s3_bucket,
    cfg.s3_endpoint.as_deref(),
).await;
```

### Step 3: Implement document loading from persistent state

- [x] Update `SessionManager` to accept `Database` and `SnapshotStorage` references
- [x] Rewrite `get_or_load()` to load from storage when a session doesn't exist in memory:

```rust
pub async fn get_or_load(
    &self,
    doc_id: Uuid,
    db: &Database,
    storage: &SnapshotStorage,
) -> Result<Arc<DocumentSession>, AppError> {
    // 1. Check in-memory cache
    if let Some(session) = self.sessions.get(&doc_id) {
        return Ok(session.clone());
    }

    // 2. Load document metadata from Postgres
    let doc_row = db.get_document(doc_id).await?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

    // 3. Create a new Yrs Doc
    let doc = Doc::new();

    // 4. If a snapshot exists, load it
    if let Some(ref key) = doc_row.snapshot_key {
        if let Some(snapshot_data) = storage.download_snapshot(key).await? {
            let update = yrs::Update::decode_v1(&snapshot_data)?;
            doc.transact_mut().apply_update(update);
        }
    }

    // 5. Replay any update log entries (crash recovery)
    let updates = db.get_update_log(doc_id).await?;
    for update_data in updates {
        let update = yrs::Update::decode_v1(&update_data)?;
        doc.transact_mut().apply_update(update);
    }

    // 6. Create session and insert into cache
    let session = DocumentSession::new_with_doc(doc_id, doc).await;
    self.sessions.insert(doc_id, session.clone());
    Ok(session)
}
```

- [x] Add `DocumentSession::new_with_doc()` that accepts an existing `Doc` instead of creating an empty one

### Step 4: Implement the update observation and logging

- [x] Add an update observer to `DocumentSession` that captures Yrs updates:

```rust
impl DocumentSession {
    pub fn start_update_logging(&self, db: Database) {
        let doc_id = self.doc_id;
        let awareness = self.awareness.clone();

        // observe_update_v1 fires on every remote update applied to the doc
        let awareness_read = awareness.blocking_read();
        awareness_read.doc().observe_update_v1(move |_txn, event| {
            let update_data = event.update.clone();
            let db = db.clone();
            tokio::spawn(async move {
                if let Err(e) = db.append_update_log(doc_id, &update_data).await {
                    tracing::error!("Failed to log update for doc {}: {}", doc_id, e);
                }
            });
        });
    }
}
```

- [x] Wire up update logging when a session is created in `get_or_load()`

### Step 5: Implement the flush mechanism

- [x] Add flush state to `DocumentSession`:

```rust
pub struct DocumentSession {
    pub doc_id: Uuid,
    pub awareness: AwarenessRef,
    pub broadcast_group: Arc<BroadcastGroup>,
    flush_notify: tokio::sync::Notify,
    update_count: AtomicU64,
}
```

- [x] Implement the `flush()` method:

```rust
pub async fn flush(
    &self,
    db: &Database,
    storage: &SnapshotStorage,
) -> Result<(), AppError> {
    // 1. Encode the current doc state as a Yrs state vector
    let awareness = self.awareness.read().await;
    let doc = awareness.doc();
    let txn = doc.transact();
    let state = txn.encode_state_as_update_v1(&yrs::StateVector::default());
    drop(txn);
    drop(awareness);

    // 2. Upload snapshot to S3
    let key = storage.upload_snapshot(self.doc_id, &state).await?;

    // 3. Update the document row with the new snapshot key
    db.update_document_snapshot(self.doc_id, &key).await?;

    // 4. Clear the update log (now redundant)
    db.clear_update_log(self.doc_id).await?;

    // 5. Reset update counter
    self.update_count.store(0, Ordering::Relaxed);

    tracing::info!("Flushed document {} to S3", self.doc_id);
    Ok(())
}
```

- [x] Spawn a background flush task per session that waits for flush triggers:

```rust
async fn flush_loop(
    session: Arc<DocumentSession>,
    db: Database,
    storage: SnapshotStorage,
) {
    loop {
        // Wait for either: 5s of inactivity, or 100 updates
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                // Inactivity timeout — flush if there are pending updates
                if session.update_count.load(Ordering::Relaxed) > 0 {
                    session.flush(&db, &storage).await;
                }
            }
            _ = session.flush_notify.notified() => {
                // Update count threshold reached
                session.flush(&db, &storage).await;
            }
        }
    }
}
```

### Step 6: Implement the unload mechanism

- [x] Add connection tracking to `DocumentSession`:
  - Increment on WebSocket connect, decrement on disconnect
  - When count reaches 0, start 60s grace timer

- [x] Implement the grace period logic:

```rust
pub async fn start_unload_timer(
    session_manager: Arc<SessionManager>,
    session: Arc<DocumentSession>,
    db: &Database,
    storage: &SnapshotStorage,
) {
    tokio::time::sleep(Duration::from_secs(60)).await;

    // Check if anyone reconnected during the grace period
    if session.connection_count.load(Ordering::Relaxed) == 0 {
        // Final flush
        session.flush(db, storage).await;
        // Remove from memory
        session_manager.unload(session.doc_id).await;
        tracing::info!("Unloaded document {} from memory", session.doc_id);
    }
}
```

- [x] Update the WebSocket handler to track connects/disconnects:
  - Increment counter before subscribing to broadcast group
  - Decrement counter after subscription completes (client disconnected)
  - Start unload timer when counter reaches 0

### Step 7: Wire everything together

- [x] Update the WebSocket handler (`websocket/handler.rs`) to pass `db` and `storage` to `get_or_load()`
- [x] Ensure the update observer is started when sessions are created
- [x] Ensure the flush loop is spawned when sessions are created
- [x] Test the full lifecycle: create doc → edit → wait for flush → restart server → reconnect → verify state

### Step 8: Integration tests

- [x] Write tests in `packages/server/tests/`:

```rust
#[tokio::test]
async fn test_document_persists_across_restart() {
    // 1. Start server, connect WebSocket, send updates
    // 2. Wait for flush
    // 3. Simulate server restart (drop all sessions, recreate)
    // 4. Reconnect, verify document state matches
}

#[tokio::test]
async fn test_update_log_crash_recovery() {
    // 1. Create session, apply updates
    // 2. Don't flush (simulate crash before compaction)
    // 3. Load from snapshot + update log
    // 4. Verify all updates are present
}

#[tokio::test]
async fn test_unload_after_grace_period() {
    // 1. Create session, connect
    // 2. Disconnect, wait > 60s
    // 3. Verify session is removed from memory
}
```

## Verification

- [x] Start server with Postgres and LocalStack running
- [ ] Open two browser tabs, edit a document collaboratively
- [ ] Wait 5+ seconds with no edits — check S3 for a snapshot file
- [ ] Restart the server (`Ctrl+C` and `cargo run` again)
- [ ] Reconnect — verify the document content is fully restored
- [ ] Disconnect all tabs — verify the session unloads after 60s (check server logs)
- [x] `cargo test` passes all new integration tests

## Files Created/Modified

- `packages/server/src/documents/storage.rs` (new)
- `packages/server/src/documents/mod.rs` (modified — add storage module, persistence lifecycle)
- `packages/server/src/lib.rs` (modified — add SnapshotStorage to AppState)
- `packages/server/src/main.rs` (modified — initialize storage)
- `packages/server/src/websocket/handler.rs` (modified — connection tracking, pass db/storage)
- `packages/server/tests/persistence.rs` (new)
