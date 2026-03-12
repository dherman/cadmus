# Milestone: Collaborative Editing Core

## Goal

Two browser tabs collaboratively editing the same document in real time.

This is the foundational milestone for Cadmus. It proves out the core technology stack — Yjs/Yrs CRDT interop over WebSocket, Tiptap's schema bound to a collaborative document, and real-time presence rendering — without the complexity of persistence, auth, or the sidecar. Everything here runs in-memory with anonymous access.

## Success Criteria

- Two browser tabs connect to the same document via WebSocket and see each other's edits within ~100ms.
- The Tiptap editor renders the full launch schema (headings, lists, code blocks, blockquotes, images, horizontal rules, all marks).
- Toolbar buttons apply formatting correctly and stay in sync across tabs.
- Colored cursors with user names appear for all connected clients.
- No data loss or corruption during concurrent editing sessions.

## Scope Boundaries

**In scope:** schema package, Rust WebSocket server, React editor, toolbar, awareness/cursors.

**Out of scope:** persistence (documents live only in memory), authentication (anonymous access), the Node sidecar, comments, REST API beyond health checks.

## PR Breakdown

The milestone is divided into six PRs, ordered by dependency. Each PR is independently mergeable and produces a testable artifact.

| PR  | Title                                                                                 | Depends On            | Estimated Effort |
| --- | ------------------------------------------------------------------------------------- | --------------------- | ---------------- |
| 1   | [Project Scaffolding & Shared Schema Package](prs/01-project-scaffolding-and-schema/) | —                     | 1–2 days         |
| 2   | [Rust WebSocket Server](prs/02-rust-websocket-server/)                                | PR 1 (schema types)   | 2–3 days         |
| 3   | [React Editor Foundation](prs/03-react-editor-foundation/)                            | PR 1 (schema package) | 1–2 days         |
| 4   | [Collaborative Sync Integration](prs/04-collaborative-sync/)                          | PR 2 + PR 3           | 1–2 days         |
| 5   | [Editor Toolbar](prs/05-editor-toolbar/)                                              | PR 3                  | 1 day            |
| 6   | [Awareness & Cursors](prs/06-awareness-and-cursors/)                                  | PR 4                  | 1 day            |

PRs 3 and 5 can be developed in parallel with PR 2. PR 6 depends on the sync integration from PR 4.

```
PR 1 (Scaffolding + Schema)
 ├── PR 2 (Rust Server)──┐
 └── PR 3 (React Editor)─┼── PR 4 (Sync Integration) ── PR 6 (Awareness)
      └── PR 5 (Toolbar)─┘
```

## Architecture Context

Refer to the main architecture docs for full context:

- [Architecture Overview](../../architecture/overview.md) — system diagram and technology choices
- [WebSocket Sync Protocol](../../architecture/websocket-protocol.md) — y-sync protocol layers, session manager design
- [Schema Design](../../architecture/schema-design.md) — launch schema definition, versioning strategy

## Key Technical Decisions for This Milestone

**In-memory only.** The Document Session Manager described in the WebSocket protocol doc includes persistence (S3 snapshots, update log). For this milestone, we implement only the in-memory portion: the `DashMap<DocId, Arc<DocumentSession>>` with `BroadcastGroup`, but without load/flush/unload lifecycle hooks. A single hardcoded document ID is sufficient.

**No auth layer.** The `PermissionedProtocol` described in the protocol doc is deferred. All connections get Edit-level access. The WebSocket handler uses `DefaultProtocol` from `yrs-axum`.

**Anonymous users with random identities.** Each browser tab generates a random user name and color on load, stored in `localStorage`. This feeds into awareness state for cursor rendering.

## Repository Structure (End State of This Milestone)

```
packages/
  doc-schema/               # PR 1: shared Tiptap extension configuration
    src/
      extensions.ts
      index.ts
    package.json
    tsconfig.json
server/                     # PR 2: Rust Axum server
  src/
    main.rs
    websocket.rs
  Cargo.toml
web/                        # PR 3–6: React frontend
  src/
    App.tsx
    Editor.tsx              # PR 3: Tiptap editor component
    Toolbar.tsx             # PR 5: formatting toolbar
    Cursors.tsx             # PR 6: awareness/cursor rendering
    collaboration.ts        # PR 4: y-websocket provider setup
  package.json
  vite.config.ts
package.json                # PR 1: workspace root
pnpm-workspace.yaml         # PR 1: pnpm workspace config
```
