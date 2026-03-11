# PR 2: Rust WebSocket Server — Implementation Plan

## Prerequisites

- [x] Rust toolchain (stable, >= 1.75) installed
- [x] PR 1 merged (repo structure exists, `server/` directory is created)

## Steps

### Step 1: Set Up Cargo Project

- [x] Update `server/Cargo.toml` with proper metadata and dependencies:

  ```toml
  [package]
  name = "cadmus-server"
  version = "0.1.0"
  edition = "2021"

  [dependencies]
  axum = { version = "0.8", features = ["ws"] }
  tokio = { version = "1", features = ["full"] }
  yrs = "0.21"
  yrs-axum = "0.2"
  dashmap = "6"
  tower-http = { version = "0.6", features = ["cors"] }
  tracing = "0.1"
  tracing-subscriber = { version = "0.3", features = ["env-filter"] }
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  ```

  (Pin exact versions after verifying compatibility; the versions above are indicative.)

- [x] Verify `cargo check` passes

### Step 2: Application State and Document Session

- [x] Create `server/src/state.rs`
- [x] Define `AppState`:
  ```rust
  pub struct AppState {
      pub documents: DashMap<String, Arc<BroadcastGroup>>,
  }
  ```
- [x] Implement `AppState::new()` that creates one default document:
  - Instantiate a `yrs::Doc` with a default client ID
  - Create an `Awareness` wrapping the doc
  - Create a `BroadcastGroup` from the awareness
  - Insert into the map with key `"default"`
- [x] Implement `AppState::get_document(&self, id: &str) -> Option<Arc<BroadcastGroup>>`

### Step 3: WebSocket Handler

- [x] Create `server/src/websocket.rs`
- [x] Define the WebSocket upgrade handler:
  ```rust
  pub async fn ws_handler(
      Path(doc_id): Path<String>,
      ws: WebSocketUpgrade,
      State(state): State<Arc<AppState>>,
  ) -> impl IntoResponse
  ```
- [x] Look up the document in `state.documents`. Return 404 if not found
- [x] Upgrade the connection and subscribe to the `BroadcastGroup`. The `yrs-axum` `BroadcastGroup::subscribe` method returns a `Subscription` that handles the y-sync protocol. Wire the subscription's `sink` and `stream` to the WebSocket

### Step 4: Health Endpoint

- [x] Create `server/src/health.rs`
- [x] Implement `GET /health` handler returning `Json({"status": "ok"})`

### Step 5: Main Entrypoint and Router

- [x] Update `server/src/main.rs`
- [x] Initialize `tracing_subscriber` with env filter
- [x] Create `AppState` with the default document
- [x] Build the Axum router:
  ```rust
  let app = Router::new()
      .route("/health", get(health_handler))
      .route("/ws/:doc_id", get(ws_handler))
      .layer(cors_layer())
      .with_state(Arc::new(state));
  ```
- [x] Configure CORS to allow `http://localhost:5173` (Vite dev server) and `http://localhost:3000`
- [x] Bind to `{HOST}:{PORT}` and serve
- [x] Log the listening address on startup

### Step 6: Write Tests

- [x] Create `server/tests/websocket_test.rs` (integration test) with:
  - [x] **"health endpoint returns 200"** — Start the server on a random port, `GET /health`, assert 200 and JSON body
  - [x] **"WebSocket connects to default document"** — Connect a `tokio-tungstenite` client to `ws://localhost:{port}/ws/default`. Assert the connection upgrades successfully
  - [x] **"WebSocket returns 404 for unknown document"** — Attempt to connect to `/ws/nonexistent`. Assert HTTP 404 (before upgrade)
  - [x] **"two clients sync edits"** — Connect two `tokio-tungstenite` clients. Have Client A apply a Yrs update (insert text). Verify Client B receives the update via the broadcast

### Step 7: Docker Configuration (Optional but Recommended)

- [x] Create `server/Dockerfile` (multi-stage build: `rust:slim` for building, `debian:slim` for runtime)
- [x] Add `server` service to a root-level `docker-compose.yml`:
  ```yaml
  services:
    server:
      build: ./server
      ports:
        - '8080:8080'
      environment:
        - RUST_LOG=info
  ```

## Verification

- [x] `cargo build` succeeds
- [x] `cargo test` passes all integration tests, including the two-client sync test
- [ ] Start the server (`cargo run`), open two browser tabs to a test page (or use `websocat`), and confirm sync works manually
- [x] `curl http://localhost:8080/health` returns `{"status": "ok"}`

## Files Created/Modified

```
server/Cargo.toml                   (modified)
server/src/main.rs                  (modified)
server/src/state.rs                 (new)
server/src/websocket.rs             (new)
server/src/health.rs                (new)
server/tests/websocket_test.rs      (new)
server/Dockerfile                   (new, optional)
docker-compose.yml                  (new, optional)
```
