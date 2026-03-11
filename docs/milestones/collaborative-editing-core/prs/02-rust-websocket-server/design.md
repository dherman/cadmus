# PR 2: Rust WebSocket Server — Design

## Purpose

Build the Axum-based Rust server that hosts Yrs documents and manages WebSocket connections. This is the collaboration hub — clients connect here, and the server merges their edits via the Yrs CRDT. For this milestone, the server runs a single in-memory document with no auth and no persistence.

## Architecture

```
                     WebSocket (wss://host/ws/{doc_id})
Browser Tab A ──────────────────────┐
                                    ▼
                            ┌───────────────┐
                            │  Axum Server  │
                            │               │
Browser Tab B ─────────────►│  DocSession   │
                            │  ┌─────────┐  │
                            │  │ Yrs Doc │  │
                            │  │ BGroup  │  │
                            │  └─────────┘  │
                            └───────────────┘
```

The server is minimal for this milestone: a single binary that listens on a port, upgrades WebSocket connections, and manages one or more document sessions in memory.

## Key Design Decisions

### yrs-axum for WebSocket Handling

The `yrs-axum` crate provides `BroadcastGroup` — a high-level abstraction that manages Yrs document sync over WebSocket connections. It handles the y-sync protocol (SyncStep1/SyncStep2/Update), Awareness propagation, and client connection lifecycle. Using this crate means we don't need to manually implement the sync protocol.

The `BroadcastGroup` is initialized with a `yrs::Doc` and an `Awareness` instance. When a WebSocket connection is accepted, it's added to the group, and the group handles the initial sync handshake and ongoing update broadcasting automatically.

### Document Session Map

Documents are stored in a `DashMap<String, Arc<BroadcastGroup>>` (using `dashmap` for concurrent access). For this milestone, we pre-seed a single document on startup. The map structure is forward-compatible with Milestone 2's multi-document support.

### DefaultProtocol (No Permission Enforcement)

The `yrs-axum` crate supports custom `Protocol` implementations for permission enforcement. For this milestone, we use `DefaultProtocol`, which allows all clients to send all message types. The `PermissionedProtocol` will be introduced in Milestone 3.

### CORS Configuration

The server needs to accept WebSocket connections from the frontend dev server (likely `localhost:5173` for Vite). CORS is configured via `tower-http`'s CORS middleware to allow the dev origin. In production, this would be locked down to the actual domain.

### Health Endpoint

A `GET /health` endpoint returns `200 OK` with `{ "status": "ok" }`. This is useful for Docker health checks and for the frontend to verify the server is running before attempting a WebSocket connection.

## Connection Lifecycle (This Milestone)

1. Client opens `ws://localhost:8080/ws/{doc_id}`.
2. Axum handler looks up the document in the `DashMap`. If not found, returns 404.
3. Handler upgrades the HTTP connection to WebSocket.
4. The WebSocket is added to the document's `BroadcastGroup`.
5. `BroadcastGroup` runs the y-sync handshake (SyncStep1/SyncStep2) and then enters steady-state update/awareness broadcasting.
6. On disconnect, the client is automatically removed from the group.

No authentication, no token validation, no permission checks. These are added in Milestone 3.

## Error Handling

- Unknown `doc_id` in WebSocket path: return HTTP 404 before upgrade.
- WebSocket connection drops: `yrs-axum` handles cleanup automatically.
- Server panic in a document handler: each document session runs in a separate Tokio task, so a panic is isolated. The `catch_unwind` boundary logs the error.

## Configuration

Environment variables with sensible defaults:

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `8080` | Listen port |
| `RUST_LOG` | `info` | Log level (via `tracing`) |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `axum` | HTTP/WebSocket framework |
| `tokio` | Async runtime |
| `yrs` | Yrs CRDT library |
| `yrs-axum` | WebSocket sync integration |
| `dashmap` | Concurrent document map |
| `tower-http` | CORS middleware |
| `tracing` / `tracing-subscriber` | Structured logging |
| `serde` / `serde_json` | JSON serialization for health endpoint |
