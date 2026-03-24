# PR 2: Content Push Endpoint

## Purpose

Implement the `POST /api/docs/{id}/content` endpoint that accepts markdown from agents and CLI clients, diffs it against the base version using the sidecar, and prepares for applying changes to the live CRDT document. This PR implements the full push flow except the ProseMirror Step → Yrs translation (PR 3) — in this PR, the endpoint calls the sidecar to compute Steps but applies them by replacing the full Yrs document content rather than doing surgical CRDT updates. PR 3 upgrades this to proper Step-based translation.

## Push Flow

```
Agent/CLI                    Server                          Sidecar
    |                           |                               |
    |-- POST /content --------->|                               |
    |   { base_version,         |                               |
    |     format: "markdown",   |                               |
    |     content: "..." }      |                               |
    |                           |-- Load base_version JSON ---->|
    |                           |   (from S3/memory)            |
    |                           |                               |
    |                           |-- POST /parse --------------->|
    |                           |   { markdown: "..." }         |
    |                           |<-- { doc: new_doc } ----------|
    |                           |                               |
    |                           |-- POST /diff ---------------->|
    |                           |   { old_doc, new_doc }        |
    |                           |<-- { steps: [...] } ----------|
    |                           |                               |
    |                           |-- Apply to Yrs doc            |
    |                           |   (full replace in PR 2,      |
    |                           |    Step translation in PR 3)  |
    |                           |                               |
    |<-- { version, summary } --|                               |
```

## Request/Response

### Push content

```
POST /api/docs/{id}/content
{
    "base_version": "v_abc",
    "format": "markdown",
    "content": "# Design Spec\n\nUpdated content..."
}
→ {
    "version": "v_def",
    "status": "applied",
    "changes_summary": {
        "steps_applied": 5,
        "nodes_added": 2,
        "nodes_removed": 0,
        "nodes_modified": 1
    }
}
```

### Dry-run mode

```
POST /api/docs/{id}/content?dry_run=true
{
    "base_version": "v_abc",
    "format": "markdown",
    "content": "# Design Spec\n\nUpdated content..."
}
→ {
    "status": "preview",
    "diff": "--- base\n+++ pushed\n@@ ... @@\n ...",
    "changes_summary": {
        "steps_applied": 5,
        "nodes_added": 2,
        "nodes_removed": 0,
        "nodes_modified": 1
    }
}
```

The diff is a unified diff of the markdown representation (base version markdown vs pushed markdown), generated server-side by serializing the base version via the sidecar and diffing the two markdown strings.

## Version Tracking

The server needs to resolve `base_version` to a ProseMirror JSON document. This requires storing version snapshots:

- Each flush to S3 records a version entry in the `documents` table (or a new `document_versions` table) with the S3 key and a version ID.
- The content-read endpoint (`GET /api/docs/{id}/content`) already returns a `version` field — this PR ensures that version ID is meaningful and resolvable.
- For the prototype, version IDs are the S3 snapshot keys (which are UUIDs). The `base_version` in a push request maps to a specific S3 snapshot.

### Version resolution strategy

1. If the document is currently loaded in memory, check if `base_version` matches the latest known version.
2. If not, load the snapshot from S3 at that version key.
3. Extract ProseMirror JSON from the Yrs document state at that version.
4. This becomes `old_doc` for the sidecar's `/diff` call.

## Change Summary

The change summary is computed from the ProseMirror Steps returned by the sidecar:

- `steps_applied`: total number of Steps
- `nodes_added`: count of ReplaceSteps that insert content
- `nodes_removed`: count of ReplaceSteps that delete content
- `nodes_modified`: count of ReplaceSteps that replace content, plus AddMarkStep/RemoveMarkStep count

This is a best-effort summary — exact categorization of Steps into add/remove/modify is imprecise but useful for agent feedback.

## Applying Changes (Interim Strategy)

In this PR, before the Step → Yrs translation layer exists (PR 3), changes are applied by:

1. Parsing the pushed markdown → `new_doc` (ProseMirror JSON) via the sidecar.
2. Computing Steps via `/diff` (for the change summary and dry-run diff).
3. Converting `new_doc` back to a Yrs XML fragment and replacing the document's content.

This is a full-document replace, not a surgical CRDT merge. It works correctly for single-writer scenarios (one agent pushing at a time) but doesn't preserve concurrent browser edits that happened after `base_version`. PR 3 upgrades this to proper Step-based application, which enables true three-way merge.

## Scope Enforcement

The handler checks:

1. The user (or agent token's user) has Edit permission on the document.
2. If the request comes via an agent token, the token includes `docs:write` scope.

## Error Cases

| Scenario                               | Response                 |
| -------------------------------------- | ------------------------ |
| Missing or empty `base_version`        | 400 Bad Request          |
| Missing or empty `content`             | 400 Bad Request          |
| Unsupported `format` (only "markdown") | 400 Bad Request          |
| `base_version` not found               | 404 Not Found            |
| User lacks Edit permission             | 403 Forbidden            |
| Agent token lacks `docs:write` scope   | 403 Forbidden            |
| Sidecar unavailable                    | 503 Service Unavailable  |
| Sidecar parse error (invalid markdown) | 422 Unprocessable Entity |
| Document not found                     | 404 Not Found            |

## What's Not Included

- ProseMirror Step → Yrs surgical translation (PR 3)
- Rate limiting on push (M7)
- Change size limits (M7)
- Version history listing (M8)
