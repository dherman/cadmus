# PR 2: Document Persistence Lifecycle

## Purpose

Implement the load/flush/unload lifecycle for document sessions so that CRDT state survives server restarts. This is the core persistence machinery — when a document is first opened, its state is loaded from S3 + the update log. While clients are editing, updates are periodically flushed. When all clients disconnect, the document is compacted and unloaded from memory.

After this PR, documents are durable. The existing WebSocket collaboration continues to work exactly as before, but edits are now persisted behind the scenes.

## Persistence Architecture

The architecture doc ([WebSocket Sync Protocol](../../../../architecture/websocket-protocol.md)) defines the persistence model. This PR implements it:

```
Client edits → Yrs Doc (in-memory) → broadcast to other clients (immediate)
                                    → append to update_log (async, per-update)
                                    → compact snapshot to S3 (periodic)
```

### Write Path

Every Yrs update that arrives via WebSocket is:

1. Applied to the in-memory `Doc` and broadcast to other clients (this already works from M1)
2. Appended to the `update_log` table in Postgres as a raw binary blob

Periodically (on flush), the full document state is:

3. Encoded as a Yrs state vector (compact binary snapshot)
4. Written to S3 as `snapshots/{doc_id}/latest.yrs`
5. The `update_log` entries for this document are cleared (they're now redundant)
6. The `documents.snapshot_key` is updated

### Read Path (Document Load)

When the first client connects to a document:

1. Fetch the document row from Postgres to get `snapshot_key`
2. If `snapshot_key` exists, download the snapshot from S3
3. Load the snapshot into a new Yrs `Doc` using `yrs::Update::decode`
4. Fetch any `update_log` entries created after the snapshot (crash recovery)
5. Apply each update to the `Doc`
6. The document is now fully reconstructed in memory

### Flush Triggers

A flush is triggered by either condition:

- **Inactivity timer:** No updates received for 5 seconds
- **Update count:** 100 updates received since last flush

The flush is debounced — if a new update arrives during the 5s window, the timer resets. This avoids unnecessary writes during active editing while ensuring reasonable durability.

### Unload

When the last client disconnects from a document:

1. Start a 60-second grace timer
2. If a new client connects within 60s, cancel the timer
3. If the timer expires, perform a final flush and remove the session from the `DashMap`

This grace period handles tab refreshes and brief network interruptions without the cost of a full load cycle.

## S3 Storage Module

A new `storage.rs` module wraps the AWS S3 SDK for snapshot operations:

- `upload_snapshot(doc_id, data)` — PUT object to `snapshots/{doc_id}/latest.yrs`
- `download_snapshot(key)` → `Option<Vec<u8>>` — GET object, returns None if not found
- `delete_snapshot(doc_id)` — DELETE object (used when deleting a document)

The S3 client is configured with a custom endpoint URL when `S3_ENDPOINT` is set (for LocalStack in dev). In production, it uses the default AWS credential chain.

## Yrs Update Observation

To capture updates for the write path, we use Yrs' `Doc::observe_update_v1()` callback. This fires every time a remote update is applied to the doc. The callback:

1. Appends the raw update bytes to the `update_log` table
2. Increments the update counter
3. Resets the inactivity timer

This runs asynchronously — it doesn't block the WebSocket handler or the broadcast path.

## Error Handling

- **S3 upload failure:** Log the error and retry on the next flush cycle. The update log provides durability — as long as Postgres is up, data isn't lost even if S3 is temporarily unavailable.
- **S3 download failure on load:** Return an error to the connecting client (503). The document can't be loaded without its snapshot.
- **Postgres insert failure (update log):** Log the error. Individual update log failures are tolerable — the next snapshot compaction will capture the full state. A sustained failure should be alerted on.
- **Load failure for nonexistent document:** If the document ID isn't in the `documents` table, return 404 to the WebSocket upgrade. This is a change from M1 where any UUID would create an empty doc.

## Key Design Decisions

**Why append individual updates to the update log, not batch them?** Batching adds complexity (buffering, flush coordination) for minimal benefit. Individual inserts to an append-only table are fast in Postgres, and the update log is cleared on every snapshot compaction, so it stays small.

**Why `latest.yrs` instead of versioned keys?** For this milestone, we only need the latest snapshot. Version history (M8) will introduce versioned snapshots later. Using a fixed key simplifies the read path and avoids S3 object enumeration.

**Why not use the `update_log` as the sole persistence mechanism?** Replaying thousands of small updates on load would be slow. The snapshot provides a compact baseline that loads in a single read. The update log is only for crash recovery between compactions.
