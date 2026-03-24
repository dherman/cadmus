# Milestone: Comments

## Goal

Anchored comments with threading, at rough parity with Google Docs. Users can select text, leave comments, reply in threads, and resolve/unresolve conversations — all in real time across connected clients.

Milestone 4 proved the sidecar pipeline and gave us programmatic document access. But collaboration is more than concurrent editing — reviewers need a way to annotate, discuss, and sign off on specific parts of a document without modifying the content itself. This milestone adds the full comment system: a `comments` table with Yjs RelativePosition anchors, REST endpoints for the complete comment lifecycle, WebSocket broadcasting for real-time visibility, and a frontend comment sidebar with threaded discussions and resolve/unresolve controls.

## Success Criteria

- Users with Comment or Edit permission can select text and create a comment anchored to that range.
- Comment anchors survive concurrent edits — a comment on paragraph 3 stays on paragraph 3 even if someone inserts content above it.
- Threaded replies work: any Comment/Edit user can reply to an existing comment thread.
- Only a comment's author can edit its body. Any Comment/Edit user can resolve or unresolve a thread.
- Read-role users can see all comments but cannot create, reply, resolve, or edit.
- Comment creation, replies, resolution, and edits are broadcast in real time to all connected clients via WebSocket custom messages — no page refresh required.
- The comment sidebar shows open threads by default, with a toggle to show resolved threads.
- Anchored text ranges are highlighted in the editor with a visual indicator linking them to the sidebar.
- Clicking a comment in the sidebar scrolls to the anchored text; clicking highlighted text opens the corresponding comment.
- The REST API supports all comment operations: list, create, reply, edit, resolve, unresolve.

## Scope Boundaries

**In scope:** Comments migration, comment CRUD REST endpoints, Yjs RelativePosition anchor conversion (character offset → RelativePosition on create), WebSocket custom message broadcasting for comment events, frontend comment sidebar, text selection → comment creation flow, anchor range highlighting, threaded replies, resolve/unresolve, permission enforcement.

**Out of scope:** Comment deletion (resolve is sufficient for the prototype — deletion adds complexity around orphaned replies), inline comment editing in the sidebar (edit via a modal or re-type — not inline contentEditable), comment notifications outside of WebSocket (email, push — deferred), comment mentions/tagging, agent comment creation via REST (the endpoints support it, but agent-specific flows are M6), offline comment creation (requires client-side queue — deferred).

## PR Breakdown

The milestone is divided into five PRs, ordered by dependency. Each PR is independently mergeable and produces a testable artifact.

| PR  | Title                                                                  | Depends On                 | Estimated Effort |
| --- | ---------------------------------------------------------------------- | -------------------------- | ---------------- |
| 1   | [Comments Table & CRUD API](prs/01-comments-table-and-crud-api/)       | —                          | 2–3 days         |
| 2   | [Comment Anchoring](prs/02-comment-anchoring/)                         | PR 1 (comments table)      | 2–3 days         |
| 3   | [WebSocket Comment Events](prs/03-websocket-comment-events/)           | PR 1 (comment mutations)   | 1–2 days         |
| 4   | [Comment Sidebar UI](prs/04-comment-sidebar-ui/)                       | PR 1 + PR 3 (API + events) | 2–3 days         |
| 5   | [Comment Threads & Resolution](prs/05-comment-threads-and-resolution/) | PR 4 (sidebar exists)      | 2–3 days         |

PRs 2 and 3 can be developed in parallel since PR 2 is about anchor persistence/resolution (server-side Yrs work) and PR 3 is about WebSocket event broadcasting (protocol-layer work). Both depend only on PR 1. PR 4 needs both PR 1 (to call the REST API) and PR 3 (to receive real-time events). PR 5 builds on the sidebar from PR 4 to add threading and resolution UI.

```
PR 1 (Comments Table & CRUD API)
 ├── PR 2 (Comment Anchoring) ────── can run in parallel
 └── PR 3 (WebSocket Comment Events) ── can run in parallel
      └── PR 4 (Comment Sidebar UI) ← also depends on PR 1
           └── PR 5 (Comment Threads & Resolution)
```

## Architecture Context

Refer to the main architecture docs for full context:

- [Architecture Overview](../../architecture/overview.md) — system diagram, comment placement in the stack
- [Comments](../../architecture/comments.md) — data model, anchoring strategy, REST + WebSocket notification pattern, permission model
- [WebSocket Sync Protocol](../../architecture/websocket-protocol.md) — Layer 5 (comment notifications), custom message support

## Key Technical Decisions for This Milestone

**REST + WebSocket notifications, not CRDT.** Comments are managed via REST API, not replicated through the Yjs CRDT. Each comment has a single author — there's no concurrent editing of comment bodies. Real-time visibility is achieved by broadcasting comment events as y-sync custom messages to all connected WebSocket clients. This is simpler than maintaining a second CRDT document and sufficient for comment semantics (see [Comments architecture doc](../../architecture/comments.md)).

**Yjs RelativePositions for anchoring.** Comment anchors are stored as binary Yjs `RelativePosition`s, which track a position through concurrent edits. The REST API accepts integer character offsets (what the frontend naturally has from the ProseMirror selection), and the server converts these to `RelativePosition`s at creation time using the live Yrs document state. A `base_version` field in the creation request ensures correct position mapping even if the document has changed slightly between the user selecting text and submitting the comment.

**Anchor resolution at read time.** When listing comments, the server resolves stored `RelativePosition`s back to absolute character offsets using the current Yrs document state. This means the frontend always receives concrete positions it can use to highlight ranges, without needing to understand Yjs internals. If the anchored content has been deleted, the positions resolve to the nearest valid location (or the comment is marked as orphaned).

**Comment events via y-sync custom messages.** The `yrs-axum` `BroadcastGroup` supports custom message types. When a comment mutation occurs via REST, the handler broadcasts a `Custom(COMMENT_EVENT_TAG, payload)` to all connected clients on that document. Connected clients update their local comment state from these events. Initial comment state is fetched via REST on page load.

**No comment deletion in the prototype.** Resolved threads are sufficient for managing comment lifecycle. Deletion introduces complexity around orphaned child replies (cascade delete? re-parent? leave tombstones?) that isn't worth solving for the prototype. Users resolve threads to dismiss them.

**Stub endpoints already exist.** The router in `lib.rs` already has `GET /api/docs/{id}/comments` and `POST /api/docs/{id}/comments` wired up, returning 501 Not Implemented. PR 1 replaces these stubs with real implementations and adds the additional endpoints for replies, editing, and resolution.

## Repository Changes (End State of This Milestone)

```
packages/
  server/
    migrations/
      20260312000001_initial.sql            # (unchanged)
      20260312000002_users.sql              # (unchanged)
      20260312000003_comments.sql           # PR 1: comments table
    src/
      main.rs                               # (unchanged)
      lib.rs                                # PR 1: add comment routes (replace stubs)
      db.rs                                 # PR 1: comment query methods
      documents/
        api.rs                              # PR 1: comment CRUD handlers
        permissions.rs                      # (unchanged — Comment permission already exists)
        comments.rs                         # PR 1: comment data structures, PR 2: anchor logic
      websocket/
        handler.rs                          # (unchanged)
        protocol.rs                         # PR 3: add custom message broadcasting
        events.rs                           # PR 3: comment event types and serialization
  web/
    src/
      api.ts                                # PR 1: comment API functions
      EditorPage.tsx                        # PR 4: integrate comment sidebar
      Editor.tsx                            # PR 4: anchor highlight marks, PR 5: selection → comment
      CommentSidebar.tsx                    # PR 4: comment list and creation form
      CommentThread.tsx                     # PR 5: threaded reply view
      useComments.ts                        # PR 4: comment state management + WS event handling
      editor.css                            # PR 4+5: comment sidebar and highlight styles
```
