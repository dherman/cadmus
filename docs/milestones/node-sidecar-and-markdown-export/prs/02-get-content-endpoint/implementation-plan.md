# PR 2: Content Read Endpoints — Implementation Plan

## Prerequisites

- [x] Milestone 3 (Auth and Permissions) is merged
- [x] Sidecar dev server runs (`pnpm dev:sidecar`) with working `/serialize` endpoint

## Risk Note

**`yrs_json.rs` is the highest-uncertainty piece of this PR.** The Yrs XML structure that y-prosemirror writes is not formally documented — you'll need to inspect it empirically (e.g., `println!` the raw XML after a few edits via WebSocket) to confirm the tag names, attribute encoding, and mark representation before writing the extractor. Do this spike in Step 1 before touching the handler. If the extraction turns out to be significantly more complex than anticipated, consider opening a separate PR for `yrs_json.rs` alone so the content endpoint can be unblocked independently.

## Steps

### 1. Create the Yrs XML → ProseMirror JSON extraction module

- [x] Create `packages/server/src/documents/yrs_json.rs`:

```rust
use serde_json::{json, Value};
use yrs::{Doc, ReadTxn, Transact, XmlFragmentRef, XmlNode, XmlTextRef, XmlElementRef};

/// Extract ProseMirror JSON from a Yrs document.
///
/// y-prosemirror stores document content in an XmlFragment named "prosemirror".
/// This function walks the Yrs XML tree and produces the equivalent ProseMirror
/// JSON representation.
pub fn extract_prosemirror_json(doc: &Doc) -> Result<Value, String> {
    let txn = doc.transact();

    // y-prosemirror uses "prosemirror" as the fragment name (this is the
    // default in y-prosemirror's ySyncPlugin configuration)
    let fragment = txn
        .get_xml_fragment("prosemirror")
        .ok_or_else(|| "No prosemirror fragment found".to_string())?;

    let children = extract_children(&txn, fragment.children(&txn));

    if children.is_empty() {
        // Return a default empty doc
        return Ok(json!({
            "type": "doc",
            "content": [{ "type": "paragraph" }]
        }));
    }

    Ok(json!({
        "type": "doc",
        "content": children
    }))
}

/// Default empty document JSON for documents with no CRDT content.
pub fn empty_doc_json() -> Value {
    json!({
        "type": "doc",
        "content": [{ "type": "paragraph" }]
    })
}
```

The full implementation of `extract_children` will need to handle `XmlElement` (block/inline nodes), `XmlText` (text with marks), and their attributes. The Yrs XML model maps cleanly to ProseMirror's node tree — each `XmlElement` tag corresponds to a ProseMirror node type, and attributes map to node attrs.

- [x] Register the module in `packages/server/src/documents/mod.rs`:

```rust
pub mod yrs_json;
```

### 2. Implement the `get_content` handler

- [x] Update `packages/server/src/documents/api.rs` — replace the TODO stub:

```rust
pub async fn get_content(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(query): Query<ContentQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_permission(&state.db, auth.user_id, id, Permission::Read).await?;

    let format = query.format.as_deref().unwrap_or("json");

    // Validate format parameter
    if format != "json" && format != "markdown" {
        return Err(AppError::BadRequest(
            "Invalid format: must be 'json' or 'markdown'".to_string(),
        ));
    }

    // Load or get the document session
    let session = state
        .document_sessions
        .get_or_load(id, &state.db, &state.storage)
        .await?;

    // Extract ProseMirror JSON from the Yrs document
    let doc_json = {
        let awareness = session.awareness.read().await;
        let yrs_doc = awareness.doc();
        yrs_json::extract_prosemirror_json(yrs_doc)
            .unwrap_or_else(|_| yrs_json::empty_doc_json())
    };

    match format {
        "markdown" => {
            // Send through sidecar for markdown conversion
            let markdown = state
                .sidecar
                .serialize(doc_json, 1) // schema_version = 1
                .await
                .map_err(|e| AppError::BadGateway(
                    format!("Markdown conversion service error: {}", e),
                ))?;

            Ok(Json(serde_json::json!({
                "format": "markdown",
                "content": markdown
            })))
        }
        _ => {
            Ok(Json(serde_json::json!({
                "format": "json",
                "content": doc_json
            })))
        }
    }
}
```

### 3. Add `BadGateway` error variant

- [x] Add a `BadGateway` variant to `AppError` in `packages/server/src/errors.rs`:

```rust
BadGateway(String),  // 502 — sidecar or upstream service error
```

- [x] Add the match arm in the `IntoResponse` impl:

```rust
AppError::BadGateway(msg) => {
    (StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))).into_response()
}
```

### 4. Verify the `get_content` endpoint works

- [x] Start the full dev environment: `pnpm dev`
- [x] Register a user and create a document via the UI.
- [x] Edit the document with some content (headings, bold text, etc.).
- [x] Test JSON format:

```bash
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/docs/<doc-id>/content
```

- [x] Test markdown format:

```bash
curl -H "Authorization: Bearer <token>" \
  "http://localhost:8080/api/docs/<doc-id>/content?format=markdown"
```

- [x] Verify 403 for a user without access.
- [x] Verify 400 for `?format=invalid`.

### 5. Write Rust tests for the content endpoint

- [x] Add content endpoint tests to `packages/server/tests/api_test.rs` (or a new `content_test.rs`):

Test cases:

- `get_content` returns JSON by default
- `get_content?format=json` returns ProseMirror JSON
- `get_content?format=markdown` returns markdown string
- `get_content` returns 403 for unauthorized user
- `get_content` returns 400 for invalid format parameter
- `get_content` returns default empty doc for newly created document

### 6. Run tests and verify

- [x] Run `cargo test` — all tests pass.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [x] `GET /api/docs/{id}/content` returns ProseMirror JSON by default
- [x] `GET /api/docs/{id}/content?format=json` returns ProseMirror JSON
- [x] `GET /api/docs/{id}/content?format=markdown` returns canonical markdown
- [x] Returns 401 without auth token
- [x] Returns 403 for users without Read permission
- [x] Returns 400 for invalid format values
- [x] Returns default empty doc for a newly created document with no edits
- [x] Markdown output matches what the frontend editor would produce (same schema)
- [x] Endpoint works for documents actively being edited (live session in memory)
- [x] Endpoint works for documents not in memory (loads from S3 + update log)

## Files Modified

| File                                        | Change                                       |
| ------------------------------------------- | -------------------------------------------- |
| `packages/server/src/documents/yrs_json.rs` | New: Yrs XML → ProseMirror JSON extraction   |
| `packages/server/src/documents/mod.rs`      | Register `yrs_json` module                   |
| `packages/server/src/documents/api.rs`      | Implement `get_content` handler              |
| `packages/server/src/errors.rs`             | Add `BadGateway` variant                     |
| `packages/server/tests/api_test.rs`         | Add content endpoint tests                   |
| `packages/server/tests/content_test.rs`     | New: edited-doc + flush/reload content tests |
