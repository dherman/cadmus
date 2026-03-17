# Implementation Roadmap

## Overview

Eight milestones, sequenced so each is independently demoable and testable. The critical path runs through Milestones 1–4 (~5–7 weeks), which delivers a working collaborative markdown editor with persistence, auth, and the sidecar. Milestones 5–7 can partially parallelize with two developers.

Total estimated time: 12–18 weeks for a small team (1–2 developers).

## Progress

- [x] Milestone 1: Collaborative Editing Core
- [x] Milestone 2: Persistence and Document Management
- [x] Milestone 3: Auth and Permissions
- [ ] Milestone 4: Node Sidecar and Markdown Export
- [ ] Milestone 5: Comments
- [ ] Milestone 6: Agent API and CLI
- [ ] Milestone 7: Enterprise Foundations
- [ ] Milestone 8: History UI

---

## Milestone 1: Collaborative Editing Core

**Goal:** Two browser tabs collaboratively editing the same document in real time.

**Deliverables:**

- Shared `doc-schema` package with launch schema (StarterKit minus Underline, plus Image, plus Markdown extension)
- Rust server (Axum) with Yrs WebSocket handler using `yrs-axum` BroadcastGroup
- No auth — any connection gets full edit access
- In-memory document state only, no persistence
- Minimal React frontend: Tiptap editor + `y-prosemirror` + `y-websocket` provider
- Basic toolbar (formatting buttons for schema marks/nodes)
- Awareness rendering (colored cursors with user names)

**Validates:** Yjs↔Yrs interop over WebSocket, Tiptap schema with y-prosemirror, editing responsiveness with server-mediated sync.

**Estimate:** 1–2 weeks

---

## Milestone 2: Persistence and Document Management

**Goal:** Documents survive server restarts. Multiple documents.

**Deliverables:**

- PostgreSQL schema: `documents`, `document_permissions`, `update_log` tables
- S3 storage for compacted Yrs snapshots
- Document session manager: load on first connect, periodic flush (5s inactivity / N updates), unload after disconnect + 60s grace
- REST endpoints: create document, list documents, get document metadata, delete document
- Frontend document dashboard: list, create, open

**Validates:** Persistence lifecycle (load/flush/unload), update log captures history correctly, compaction doesn't lose data.

**Estimate:** 1–2 weeks

---

## Milestone 3: Auth and Permissions

**Goal:** Multi-user system with granular document permissions.

**Deliverables:**

- User auth (email/password + JWT, or OAuth provider)
- WebSocket token flow: `POST /api/auth/ws-token` → short-lived JWT, validated on WS upgrade
- Custom `PermissionedProtocol` in Rust: gates Yjs Update messages by role (Read/Comment/Edit)
- Document sharing UI: invite by email, assign roles
- Permissions table in Postgres, middleware enforcement on REST + WebSocket paths

**Validates:** Permission enforcement end-to-end (Read can't type, Comment can't edit content), token refresh for long sessions.

**Estimate:** 1–2 weeks

---

## Milestone 4: Node Sidecar and Markdown Export

**Goal:** Proven markdown↔JSON conversion pipeline via the sidecar.

**Deliverables:**

- Node sidecar service: HTTP server importing `@cadmus/doc-schema`, exposing `/serialize`, `/parse`, `/diff`, `/health`
- Deployed as second container in ECS task definition
- REST endpoint: `GET /api/docs/{id}/content?format=markdown`
- REST endpoint: `GET /api/docs/{id}/content?format=json`
- "Export as Markdown" button in frontend
- Integration test suite: round-trip fidelity (create in editor → export markdown → parse back → compare)

**Validates:** Sidecar architecture works reliably, serialization matches frontend output, round-trips are clean for all schema constructs.

**Estimate:** 1 week

---

## Milestone 5: Comments

**Goal:** Anchored comments with threading, at rough parity with Google Docs.

**Deliverables:**

- Comments table in Postgres (anchors stored as Yjs RelativePositions)
- REST endpoints: create, reply, edit own, resolve, unresolve
- WebSocket comment event broadcasting via y-sync custom messages
- Frontend: highlight anchored ranges, comment sidebar, create by selecting text, threaded replies, resolve/unresolve
- Permission enforcement: Read sees comments but can't create, Comment/Edit can create

**Validates:** Comment anchors survive concurrent edits, REST + WS notification model feels real-time, UX parity with Google Docs.

**Estimate:** 2–3 weeks (frontend-heavy)

**Parallelizable:** Independent from M6 (Agent API).

---

## Milestone 6: Agent API and CLI

**Goal:** Non-browser clients can read, write, and comment on documents.

**Deliverables:**

- Agent token management: create, list, revoke scoped API tokens
- Content read endpoint (already exists from M4, now behind proper auth)
- Full-markdown-push endpoint: `POST /api/docs/{id}/content` with base_version, sidecar diffing, Yrs translation
- Dry-run mode for push endpoint
- CLI tool (`cadmus`): `auth login`, `docs list`, `checkout`, `push`, `push --dry-run`
- `.cadmus/` metadata directory for checkout state
- Agent awareness rendering in frontend (bot icon, status text)

**Validates:** Full checkout→edit→push cycle, diff/merge via ProseMirror Steps, agent auth and interaction, hands-on testing with an actual AI agent.

**Estimate:** 2–3 weeks

**Parallelizable:** Independent from M5 (Comments), except both need M4.

---

## Milestone 7: Enterprise Foundations

**Goal:** Organizational hierarchy and admin controls.

**Deliverables:**

- Organization and workspace data model
- Org-level default permissions for documents
- Admin controls for agent tokens: scope allowlists, lifetime caps, disable BYO
- Default agent integrations configured by org admin
- Audit logging: all document mutations logged with actor, action, timestamp, summary

**Depends on:** M6 (for the agent token model).

**Estimate:** 2 weeks

---

## Milestone 8: History UI

**Goal:** User-facing document history and version comparison.

**Deliverables:**

- Version listing endpoint: `GET /api/docs/{id}/versions` with author attribution
- Diff endpoint: `GET /api/docs/{id}/diff` (two versions → markdown diff via sidecar)
- Named checkpoints: users can label versions
- Frontend history panel: timeline, click to view snapshot, diff between any two versions
- Restore to previous version (creates new version matching old state, never destructive)

**Estimate:** 2–3 weeks

---

## If Time Is Tight

**Cut or compress:**

- History UI (M8) — data is being stored from M2 onward; UI can come later.
- Enterprise controls (M7) — start with minimal org model, defer audit logging.
- Targeted edits API (Pattern 2) — ship with full-markdown-push only.

**Do not cut:**

- The sidecar (M4) and CLI (M6) — the local tooling story is a key differentiator.
- The comments (M5) — core collaboration feature expected by users.

## Dependency Graph

```
M1 (Core editing)
 └── M2 (Persistence)
      └── M3 (Auth)
           └── M4 (Sidecar)
                ├── M5 (Comments)     ─── can run in parallel
                ├── M6 (Agent/CLI)    ─── can run in parallel
                │    └── M7 (Enterprise)
                └── M8 (History UI)
```
