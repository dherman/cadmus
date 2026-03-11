# CLI & Local Tools

## Design Decision: Checkout/Push Model (Option A)

We evaluated three approaches for local tool integration:

- **Option A: CLI with snapshot checkout/merge.** Simple, stateless, familiar to developers.
- **Option B: Local sync daemon.** Filesystem watcher + bidirectional CRDT sync. Elegant but operationally complex.
- **Option C: Local WebSocket proxy.** Small local server proxying CRDT protocol. Flexible but similar complexity to B.

We chose Option A. It's the simplest, works for both human CLI users and agents, and doesn't require a persistent local process. The checkout/push model is familiar (analogous to git) and can be enhanced later.

## Workflow

```bash
# Authenticate (stores credentials in ~/.config/cadmus/credentials)
cadmus auth login

# List accessible documents
cadmus docs list

# Checkout: fetches current markdown + records version
cadmus checkout doc_xyz -o ./design-spec.md

# Edit locally with any tool (IDE, vim, agent, script)

# Preview changes before pushing
cadmus push doc_xyz ./design-spec.md --dry-run

# Push: submits changes, server merges via CRDT
cadmus push doc_xyz ./design-spec.md

# Leave a comment by line range
cadmus comment doc_xyz --lines 45-52 "This needs clarification"
```

## Checkout Metadata

On checkout, the CLI creates a `.cadmus/` directory alongside the output file:

```
.cadmus/
  doc_xyz.json    # { "doc_id": "...", "version": "v_abc", "checked_out_at": "...", "file": "./design-spec.md" }
```

This records the `base_version` needed for the push merge step. The CLI reads this automatically on push.

## Push Merge Strategy

1. CLI reads the local markdown file and the `.cadmus/` metadata.
2. CLI calls `POST /api/docs/{id}/content` with `base_version` and the markdown content.
3. Server sends the markdown to the sidecar for parsing → `new_doc` (ProseMirror JSON).
4. Server loads the ProseMirror JSON at `base_version` → `old_doc`.
5. Server sends `old_doc` + `new_doc` to sidecar's `/diff` → ProseMirror Steps.
6. Server translates Steps into Yrs operations and applies to the current live document.

The CRDT handles concurrent edits gracefully in most cases. If both the CLI user and a web user edited different paragraphs, the merge is clean. Structural conflicts (both edited the same paragraph, or one moved a section the other modified) produce a best-effort CRDT merge that should be reviewed.

The `--dry-run` flag returns the diff and any detected conflicts without applying changes.

## On Git Compatibility

We decided against a git-compatible mirror. The impedance mismatch between continuous CRDT edits and discrete git commits is significant — batching CRDT updates into commits is an unsolved design problem, and bidirectional sync (git push → CRDT) would be a project unto itself. The checkout/push API gives 80% of the value. One-way export (document history → git repo) could be added later without bidirectional complexity.

## Agent Usage

Agents use the same REST API the CLI uses. A typical agent workflow:

```python
# 1. Read the document
doc = api.get("/docs/{id}/content?format=markdown")

# 2. Process/modify the content
updated = agent.process(doc["content"])

# 3. Preview changes
preview = api.post("/docs/{id}/content?dry_run=true", {
    "base_version": doc["version"],
    "format": "markdown",
    "content": updated
})

# 4. Apply if satisfied
result = api.post("/docs/{id}/content", {
    "base_version": doc["version"],
    "format": "markdown",
    "content": updated
})
```
