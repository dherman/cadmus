# PR 2: Content Push Endpoint — Implementation Plan

## Prerequisites

- [x] PR 1 (Agent Token Management) is merged

## Steps

### 1. Add version tracking to document storage

- [x] In `packages/server/src/documents/storage.rs`, ensure that each flush to S3 records a version identifier. The snapshot key (UUID) serves as the version ID.

- [x] Add a `current_version` field to `DocumentSession` in `packages/server/src/documents/mod.rs`:

```rust
pub struct DocumentSession {
    // ... existing fields ...
    pub current_version: RwLock<Option<String>>,
}
```

- [x] Update the flush logic to set `current_version` after each successful S3 write.

- [x] Add a method `load_version_snapshot(version: &str) -> Result<Vec<u8>>` to `SnapshotStorage` that loads a specific S3 snapshot by key.

### 2. Update the content-read endpoint to return version

- [x] In `packages/server/src/documents/api.rs`, update `get_content` to include the current version in its response:

```rust
// Response for GET /api/docs/{id}/content
{
    "version": "v_abc123",
    "format": "markdown",
    "content": "...",
    "updated_at": "..."
}
```

- [x] The version comes from `session.current_version`. If the session was just loaded from S3, the version is the snapshot key. If no flush has occurred yet (new document), generate an initial version on first read.

### 3. Add Yrs JSON extraction utility

- [x] In `packages/server/src/documents/yrs_json.rs`, ensure there are functions for:
  - `extract_prosemirror_json(doc: &Doc) -> Result<serde_json::Value>` — reads the Yrs XML fragment and converts to ProseMirror JSON.
  - `replace_yrs_content(doc: &Doc, pm_json: &serde_json::Value) -> Result<()>` — replaces the entire Yrs document content with new ProseMirror JSON (interim strategy for PR 2, upgraded to Step-based in PR 3).

- [x] The extraction function already exists from M4. Verify it handles all schema node types. The replacement function is new — it creates a Yrs transaction, clears the XML fragment, and rebuilds it from the ProseMirror JSON.

### 4. Implement the push_content handler

- [x] In `packages/server/src/documents/api.rs`, replace the stub `push_content` with the full implementation:

```rust
pub async fn push_content(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(params): Query<PushContentQuery>,
    Json(body): Json<PushContentRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // 1. Permission + scope check
    require_permission(&state.db, auth.user_id, id, Permission::Edit).await?;
    require_scope(&auth, "docs:write")?;

    // 2. Load the document session
    let session = state.document_sessions.get_or_load(id, &state.db, &state.storage).await?;

    // 3. Load base version ProseMirror JSON
    let base_snapshot = state.storage.load_version_snapshot(&body.base_version).await?;
    let base_doc = yrs_json::extract_from_snapshot(&base_snapshot)?;

    // 4. Parse pushed markdown via sidecar
    let new_doc = state.sidecar.parse(&body.content, 1).await?;

    // 5. Compute diff via sidecar
    let steps = state.sidecar.diff(&base_doc, &new_doc.doc).await?;

    // 6. Build change summary
    let summary = compute_change_summary(&steps);

    // 7. If dry run, generate markdown diff and return preview
    if params.dry_run.unwrap_or(false) {
        let base_markdown = state.sidecar.serialize(&base_doc, 1).await?;
        let diff = generate_unified_diff(&base_markdown, &body.content);
        return Ok(Json(json!({
            "status": "preview",
            "diff": diff,
            "changes_summary": summary,
        })));
    }

    // 8. Apply changes to the live document
    //    (PR 2: full replace; PR 3 upgrades to Step translation)
    yrs_json::replace_yrs_content(&session.doc, &new_doc.doc)?;

    // 9. Trigger flush and get new version
    let new_version = session.trigger_flush().await?;

    Ok(Json(json!({
        "version": new_version,
        "status": "applied",
        "changes_summary": summary,
    })))
}
```

### 5. Add query parameter support for dry_run

- [x] Add a query parameter struct:

```rust
#[derive(Deserialize)]
pub struct PushContentQuery {
    pub dry_run: Option<bool>,
}
```

### 6. Implement unified diff generation

- [x] Add a utility function `generate_unified_diff(old: &str, new: &str) -> String` that produces a standard unified diff between two markdown strings. Use the `similar` crate (already common in Rust projects) or a simple line-by-line diff.

- [x] Add `similar` to `Cargo.toml` if not already present.

### 7. Implement change summary computation

- [x] Add `compute_change_summary(steps: &[serde_json::Value]) -> serde_json::Value`:

```rust
fn compute_change_summary(steps: &[serde_json::Value]) -> serde_json::Value {
    let mut added = 0;
    let mut removed = 0;
    let mut modified = 0;

    for step in steps {
        match step["stepType"].as_str() {
            Some("replace") => {
                let has_content = step.get("slice").is_some();
                let from = step["from"].as_u64().unwrap_or(0);
                let to = step["to"].as_u64().unwrap_or(0);
                if from == to && has_content { added += 1; }
                else if !has_content { removed += 1; }
                else { modified += 1; }
            }
            Some("addMark") | Some("removeMark") => { modified += 1; }
            _ => {}
        }
    }

    json!({
        "steps_applied": steps.len(),
        "nodes_added": added,
        "nodes_removed": removed,
        "nodes_modified": modified,
    })
}
```

### 8. Implement yrs_json::replace_yrs_content

- [x] In `packages/server/src/documents/yrs_json.rs`, implement the full document replace:

```rust
pub fn replace_yrs_content(doc: &Doc, pm_json: &serde_json::Value) -> Result<()> {
    let mut txn = doc.transact_mut();
    let fragment = txn.get_or_insert_xml_fragment("prosemirror");

    // Clear existing content
    let len = fragment.len(&txn);
    for i in (0..len).rev() {
        fragment.remove_range(&mut txn, i, 1);
    }

    // Rebuild from ProseMirror JSON
    build_yrs_content(&mut txn, &fragment, pm_json)?;

    Ok(())
}
```

This is the interim strategy. It works correctly but doesn't preserve concurrent CRDT state. PR 3 replaces this with Step-based translation.

### 9. Test the push endpoint

- [x] Start the full dev stack: `pnpm dev`
- [x] Create a document and add some content via the browser editor.
- [x] Get the document content and version:

```bash
curl http://localhost:8080/api/docs/{id}/content?format=markdown \
  -H 'Authorization: Bearer <token>'
```

- [x] Test dry-run:

```bash
curl -X POST 'http://localhost:8080/api/docs/{id}/content?dry_run=true' \
  -H 'Authorization: Bearer <token>' \
  -H 'Content-Type: application/json' \
  -d '{
    "base_version": "v_abc",
    "format": "markdown",
    "content": "# Updated Title\n\nNew content here."
  }'
```

- [x] Test actual push:

```bash
curl -X POST http://localhost:8080/api/docs/{id}/content \
  -H 'Authorization: Bearer <token>' \
  -H 'Content-Type: application/json' \
  -d '{
    "base_version": "v_abc",
    "format": "markdown",
    "content": "# Updated Title\n\nNew content here."
  }'
```

- [x] Verify the document content changed by reading it again.
- [x] Verify a browser client connected to the document sees the change in real time.

### 10. Build and format check

- [x] Run `cargo build` in `packages/server/` — compiles without errors.
- [x] Run `cargo test` in `packages/server/` — all tests pass.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [ ] `POST /api/docs/{id}/content` accepts markdown with base_version
- [ ] Returns new version ID and change summary on success
- [ ] `?dry_run=true` returns unified diff and summary without applying
- [ ] Invalid base_version returns 404
- [ ] Missing required fields return 400
- [ ] Permission enforcement works (Edit required)
- [ ] Scope enforcement works (docs:write required for agent tokens)
- [ ] Sidecar errors (parse failure) return 422
- [ ] Document content is updated and visible to browser clients
- [ ] get_content returns the updated version after push
- [ ] Sequential pushes work (push, read new version, push again)

## Files Modified

| File                                        | Change                                            |
| ------------------------------------------- | ------------------------------------------------- |
| `packages/server/src/documents/api.rs`      | Implement push_content handler, add dry_run query |
| `packages/server/src/documents/yrs_json.rs` | Add replace_yrs_content function                  |
| `packages/server/src/documents/mod.rs`      | Add current_version to DocumentSession            |
| `packages/server/src/documents/storage.rs`  | Add load_version_snapshot, version tracking       |
| `packages/server/Cargo.toml`                | Add `similar` crate for diff generation           |
