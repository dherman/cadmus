# PR 2: Content Read Endpoints

## Purpose

Implement `GET /api/docs/{id}/content` to return document content as either ProseMirror JSON or canonical markdown. This is the first REST endpoint that bridges the Yrs CRDT state with the sidecar — it extracts ProseMirror JSON from the live Yrs document and optionally converts it to markdown via the sidecar's `/serialize` endpoint. The endpoint enables M4's export feature and lays groundwork for M6's agent read path.

## API Endpoint

### `GET /api/docs/{id}/content`

**Query parameters:**

| Parameter | Type   | Default  | Description              |
| --------- | ------ | -------- | ------------------------ |
| `format`  | string | `"json"` | `"json"` or `"markdown"` |

**Response (format=json):**

```json
{
  "format": "json",
  "content": {
    "type": "doc",
    "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Hello world" }] }]
  }
}
```

**Response (format=markdown):**

```json
{
  "format": "markdown",
  "content": "Hello world\n"
}
```

**Status codes:**

| Status | Condition                                  |
| ------ | ------------------------------------------ |
| 200    | Success                                    |
| 400    | Invalid `format` value                     |
| 401    | Missing or invalid auth token              |
| 403    | User lacks Read permission on the document |
| 404    | Document not found                         |
| 502    | Sidecar unavailable or returned an error   |

### Authentication and Permissions

The endpoint requires authentication (JWT Bearer token) and at minimum Read permission on the document. The existing `require_permission(&state.db, auth.user_id, id, Permission::Read)` check in the handler stub already enforces this.

## Yrs XML → ProseMirror JSON Extraction

The Yrs document stores content in an `XmlFragment` that mirrors ProseMirror's node tree. To extract ProseMirror JSON, we need a `yrs_json` module that walks the Yrs XML structure and produces the equivalent JSON.

### Yrs-to-JSON Mapping

| Yrs Type        | ProseMirror JSON                                        |
| --------------- | ------------------------------------------------------- |
| `XmlFragment`   | `{ "type": "doc", "content": [...] }`                   |
| `XmlElement`    | `{ "type": "<tag>", "attrs": {...}, "content": [...] }` |
| `XmlText`       | `{ "type": "text", "text": "...", "marks": [...] }`     |
| XML attributes  | Node `attrs` object                                     |
| Text formatting | `marks` array on text nodes                             |

The `XmlFragment` in a Yrs document created by y-prosemirror uses the tag names and attribute conventions from ProseMirror's schema. The extraction function reads the Yrs transaction, walks the XML tree, and produces the JSON representation.

### Core Extraction Function

```rust
// packages/server/src/documents/yrs_json.rs

use yrs::{Doc, ReadTxn, Transact, XmlFragmentRef, XmlNode};
use serde_json::{json, Value};

/// Extract ProseMirror JSON from a Yrs document's default XmlFragment.
pub fn extract_prosemirror_json(doc: &Doc) -> Result<Value, String> {
    let txn = doc.transact();
    let fragment = txn
        .get_xml_fragment("prosemirror")
        .ok_or_else(|| "No prosemirror fragment found in document".to_string())?;

    let content = extract_fragment_children(&txn, &fragment);

    Ok(json!({
        "type": "doc",
        "content": content
    }))
}
```

### Handling Empty Documents

A newly created document that has never been edited through the WebSocket has no Yrs content. The handler should return a default empty document:

```json
{
  "type": "doc",
  "content": [{ "type": "paragraph" }]
}
```

## Sidecar Integration

For `format=markdown`, the handler:

1. Extracts ProseMirror JSON from the Yrs document (as above).
2. Sends it to the sidecar's `POST /serialize` endpoint via the existing `SidecarClient`.
3. Returns the markdown string in the response.

### Error Handling for Sidecar Calls

The sidecar is a localhost HTTP call with sub-millisecond overhead in production. Failures should be rare but handled:

- **Sidecar unreachable:** Return 502 with `{ "error": "Markdown conversion service unavailable" }`.
- **Sidecar returns error:** Return 502 with the sidecar's error message.
- **Schema version mismatch:** The sidecar returns 400 if schema versions differ. Surface this as 502 since it indicates a deployment issue, not a client error.

## What's Not Included

- The `version` query parameter for fetching historical content (deferred to M8 History UI).
- The `POST /api/docs/{id}/content` push endpoint (deferred to M6 Agent API).
- Caching of serialized content — each request re-extracts and re-serializes. Acceptable for prototype.

## Dependencies

**New Rust module:**

- `documents/yrs_json.rs` — Yrs XML → ProseMirror JSON extraction

**Existing (no new crates):**

- `SidecarClient` in `sidecar.rs` — already has `serialize()` method
- `SessionManager` in `documents/mod.rs` — already has `get_or_load()` method
