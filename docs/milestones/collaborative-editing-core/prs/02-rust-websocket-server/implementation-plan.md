# PR 2: Rust WebSocket Server — Implementation Plan

## Prerequisites

- [ ] Rust toolchain (stable, >= 1.75) installed
- [ ] PR 1 merged (repo structure exists, `server/` directory is created)

## Steps

### Step 1: Set Up Cargo Project

- [ ] Update `server/Cargo.toml` with proper metadata and dependencies:
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
- [ ] Verify `cargo check` passes

### Step 2: Application State and Document Session

- [ ] Create `server/src/state.rs`
- [ ] Define `AppState`:
  ```rust
  pub struct AppState {
      pub documents: DashMap<String, Arc<BroadcastGroup>>,
  }
  ```
- [ ] Implement `AppState::new()` that creates one default document:
  - Instantiate a `yrs::Doc` with a default client ID
  - Create an `Awareness` wrapping the doc
  - Create a `BroadcastGroup` from the awareness
  - Insert into the map with key `"default"`
- [ ] Implement `AppState::get_document(&self, id: &str) -> Option<Arc<BroadcastGroup>>`

### Step 3: WebSocket Handler

- [ ] Create `server/src/websocket.rs`
- [ ] Define the WebSocket upgrade handler:
  ```rust
  pub async fn ws_handler(
      Path(doc_id): Path<String>,
      ws: WebSocketUpgrade,
      State(state): State<Arc<AppState>>,
  ) -> impl IntoResponse
  ```
- [ ] Look up the document in `state.documents`. Return 404 if not found
- [ ] Upgrade the connection and subscribe to the `BroadcastGroup`. The `yrs-axum` `BroadcastGroup::subscribe` method returns a `Subscription` that handles the y-sync protocol. Wire the subscription's `sink` and `stream` to the WebSocket

### Step 4: Health Endpoint

- [ ] Create `server/src/health.rs`
- [ ] Implement `GET /health` handler returning `Json({"status": "ok"})`

### Step 5: Main Entrypoint and Router

- [ ] Update `server/src/main.rs`
- [ ] Initialize `tracing_subscriber` with env filter
- [ ] Create `AppState` with the default document
- [ ] Build the Axum router:
  ```rust
  let app = Router::new()
      .route("/health", get(health_handler))
      .route("/ws/:doc_id", get(ws_handler))
      .layer(cors_layer())
      .with_state(Arc::new(state));
  ```
- [ ] Configure CORS to allow `http://localhost:5173` (Vite dev server) and `http://localhost:3000`
- [ ] Bind to `{HOST}:{PORT}` and serve
- [ ] Log the listening address on startup

### Step 6: Write Tests

- [ ] Create `server/tests/websocket_test.rs` (integration test) with:
  - [ ] **"health endpoint returns 200"** — Start the server on a random port, `GET /health`, assert 200 and JSON body
  - [ ] **"WebSocket connects to default document"** — Connect a `tokio-tungstenite` client to `ws://localhost:{port}/ws/default`. Assert the connection upgrades successfully
  - [ ] **"WebSocket returns 404 for unknown document"** — Attempt to connect to `/ws/nonexistent`. Assert HTTP 404 (before upgrade)
  - [ ] **"two clients sync edits"** — Connect two `tokio-tungstenite` clients. Have Client A apply a Yrs update (insert text). Verify Client B receives the update via the broadcast

### Step 7: Docker Configuration (Optional but Recommended)

- [ ] Create `server/Dockerfile` (multi-stage build: `rust:slim` for building, `debian:slim` for runtime)
- [ ] Add `server` service to a root-level `docker-compose.yml`:
  ```yaml
  services:
    server:
      build: ./server
      ports:
        - "8080:8080"
      environment:
        - RUST_LOG=info
  ```

## Verification

- [ ] `cargo build` succeeds
- [ ] `cargo test` passes all integration tests, including the two-client sync test
- [ ] Start the server (`cargo run`), open two browser tabs to a test page (or use `websocat`), and confirm sync works manually
- [ ] `curl http://localhost:8080/health` returns `{"status": "ok"}`

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
