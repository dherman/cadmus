# Agent API

## Design Philosophy

Agents are API clients. They authenticate with scoped tokens, inherit user permissions, and interact via the same REST endpoints and WebSocket protocol as browser clients. The system doesn't call out to agents — agents call in. This avoids the security complexity of the server making requests to arbitrary user-specified endpoints.

## Authentication

### Token model

A user creates an agent token via the web UI or REST API. The token encodes:

- The user ID it acts on behalf of
- Optional restriction to specific document IDs or workspace scopes
- A permission ceiling (can never exceed the user's own permissions, but can be further restricted)
- An expiration

```
POST /api/tokens
{
  "name": "my-coding-agent",
  "scopes": ["docs:read", "docs:write", "comments:write"],
  "document_ids": null,
  "expires_in": "30d"
}
→ {
  "token_id": "tok_abc123",
  "secret": "sk-...",
  "expires_at": "..."
}
```

The `secret` is shown once at creation time. It's used as a Bearer token for REST calls and as the `token` query parameter for WebSocket connections.

### Enterprise controls

Organization admins can: define allowlists of approved token scopes, restrict which users can create agent tokens, set maximum token lifetimes, provide default agent integrations for all users, disable BYO agents entirely. These are policy checks at token creation time.

## REST Endpoints

### Document discovery

```
GET /api/docs?workspace={id}&cursor={...}&limit=50
GET /api/docs/{id}
```

Standard paginated listing. Agents discover documents the same way humans do.

### Document content — read

```
GET /api/docs/{id}/content?format=markdown|json&version={version_id}
→ {
  "version": "v_abc",
  "format": "markdown",
  "content": "# Design Spec\n\nThis document...",
  "updated_at": "..."
}
```

When `format=markdown`, the server calls the sidecar to serialize. When `format=json`, returns ProseMirror JSON directly. The optional `version` parameter reads historical state.

### Document content — write (Pattern 1: full markdown push)

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
  "changes_summary": { "paragraphs_added": 2, "paragraphs_modified": 1 }
}
```

Server diffs pushed markdown against the base version using ProseMirror Steps (via the sidecar), then applies to the current Yrs document. The `base_version` is critical — it enables three-way merge.

### Document content — write (Pattern 2: targeted edits, deferred)

```
POST /api/docs/{id}/edits
{
  "base_version": "v_abc",
  "operations": [
    { "type": "replace_section", "heading": "## Implementation", "content": "..." },
    { "type": "insert_after", "heading": "## Implementation", "content": "## Timeline\n..." },
    { "type": "append", "content": "\n## References\n..." }
  ]
}
```

Higher-level API for surgical edits. Build Pattern 1 first; add this once agent usage patterns are clear.

### Dry-run mode

Any write endpoint accepts `?dry_run=true`:

```
POST /api/docs/{id}/content?dry_run=true
→ {
  "status": "preview",
  "diff": "unified diff of what would change",
  "conflicts": [{ "region": "lines 45-52", "reason": "modified since base_version" }]
}
```

### Comments

```
GET  /api/docs/{id}/comments?status=open|resolved|all
POST /api/docs/{id}/comments
     { "base_version": "v_abc", "anchor": { "from": 142, "to": 189 }, "body": "..." }
POST /api/docs/{id}/comments/{cid}/replies
     { "body": "..." }
PUT  /api/docs/{id}/comments/{cid}
     { "body": "updated" }
POST /api/docs/{id}/comments/{cid}/resolve
POST /api/docs/{id}/comments/{cid}/unresolve
```

Comment anchors are provided as integer character offsets (relative to the `base_version` markdown). The server converts these to Yjs `RelativePosition`s at creation time, so they survive concurrent edits.

### History

```
GET /api/docs/{id}/versions?cursor=...&limit=20
GET /api/docs/{id}/diff?from={v1}&to={v2}&format=markdown
```

### WebSocket (Pattern 3: real-time agent)

An agent can connect via WebSocket identically to a browser client. It participates in the Yjs sync protocol, sees live updates, and can push edits as CRDT operations. The agent's awareness state can include status information (`"analyzing section 3"`) that the UI renders as a presence indicator.

This is the most powerful integration but requires the agent to understand Yjs data structures. Most batch-oriented agents will prefer Patterns 1 or 2.

## Rate Limiting and Guardrails

- Per-token rate limits on write operations (e.g., 60 writes/minute). Clear 429 responses.
- Change size limits. Warn on large diffs, reject above a hard threshold unless `force: true`.
- Audit logging: every agent action logged with token ID, agent name, operation type, document ID, change summary.
