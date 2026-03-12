# Milestone: Persistence and Document Management

## Goal

Documents survive server restarts. Users can create, list, and switch between multiple documents.

Milestone 1 proved the collaborative editing stack works — but everything lives in memory with a single hardcoded document. This milestone adds the storage layer (PostgreSQL + S3) and the document lifecycle (create, list, open, delete), turning Cadmus from a demo into something that could actually hold real work.

## Success Criteria

- A document edited in the browser survives a server restart with no data loss.
- The flush cycle (5s inactivity / N updates) writes Yrs snapshots to S3 and update log entries to PostgreSQL.
- Unloading a document from memory after all clients disconnect (60s grace period) and reloading it on reconnect produces identical CRDT state.
- REST endpoints for document CRUD work correctly (create, list, get metadata, delete).
- The frontend displays a document dashboard where users can create new documents and open existing ones.
- Multiple documents can be open in separate tabs simultaneously without interference.

## Scope Boundaries

**In scope:** PostgreSQL schema, S3 snapshot storage, document session lifecycle (load/flush/unload), REST API for document CRUD, frontend document dashboard.

**Out of scope:** authentication (anonymous access continues), the Node sidecar (not wired into REST yet), comments, document permissions, version history UI. The `document_permissions` table is created as part of the schema but not enforced until Milestone 3.

## PR Breakdown

The milestone is divided into four PRs, ordered by dependency. Each PR is independently mergeable and produces a testable artifact.

| PR  | Title                                                                    | Depends On          | Estimated Effort |
| --- | ------------------------------------------------------------------------ | ------------------- | ---------------- |
| 1   | [Database Schema & Migrations](prs/01-database-schema-and-migrations/)   | —                   | 1–2 days         |
| 2   | [Document Persistence Lifecycle](prs/02-document-persistence-lifecycle/) | PR 1 (tables exist) | 2–3 days         |
| 3   | [Document CRUD API](prs/03-document-crud-api/)                           | PR 1 (tables exist) | 1–2 days         |
| 4   | [Frontend Document Dashboard](prs/04-frontend-document-dashboard/)       | PR 3 (REST API)     | 1–2 days         |

PRs 2 and 3 can be developed in parallel since both depend on PR 1 but not on each other.

```
PR 1 (Database Schema)
 ├── PR 2 (Persistence Lifecycle) ─── can run in parallel
 └── PR 3 (CRUD API) ─────────────── can run in parallel
      └── PR 4 (Frontend Dashboard)
```

## Architecture Context

Refer to the main architecture docs for full context:

- [Architecture Overview](../../architecture/overview.md) — system diagram, storage layer design
- [WebSocket Sync Protocol](../../architecture/websocket-protocol.md) — Document Session Manager lifecycle (load/flush/unload), persistence strategy

## Key Technical Decisions for This Milestone

**SQLx with compile-time checked queries.** We use SQLx's query macros for type-safe SQL. During development, queries are checked against the live database. For CI, SQLx's offline mode uses cached query metadata (`sqlx-data.json`).

**S3 for snapshots, Postgres for metadata and update log.** The architecture separates concerns: Postgres stores structured metadata (document titles, timestamps, permissions) and the append-only update log. S3 stores the large binary Yrs snapshots. This keeps the database lean and makes snapshots cheap to store/retrieve.

**Write-behind persistence.** Updates are applied to the in-memory Yrs doc and broadcast immediately. Persistence is async — flushed every 5 seconds of inactivity or every 100 updates. On server crash, up to 5 seconds of edits may be lost. This is acceptable for a prototype; the update log captures individual updates between compactions for recovery.

**LocalStack for local S3.** Development uses LocalStack to emulate S3 locally, avoiding AWS credentials during development. Docker Compose is extended with a LocalStack service.

**Anonymous access continues.** No auth in this milestone. Documents are created without owners and accessible to anyone. The `document_permissions` table is created but not enforced.

**SQLx migrations.** We use SQLx's built-in migration system (`sqlx migrate run`) rather than a separate migration tool. Migrations live in `packages/server/migrations/` and run automatically on server startup.

## Repository Changes (End State of This Milestone)

```
packages/
  server/
    migrations/                     # PR 1: SQLx migration files
      20260312000001_initial.sql
    src/
      main.rs                       # PR 1: add migration runner
      lib.rs                        # PR 3: add delete route
      config.rs                     # PR 2: add S3/flush config
      db.rs                         # PR 1: add query methods
      documents/
        mod.rs                      # PR 2: persistence lifecycle
        api.rs                      # PR 3: implement CRUD handlers
        storage.rs                  # PR 2: S3 snapshot operations
      errors.rs                     # (unchanged)
      sidecar.rs                    # (unchanged)
      websocket/
        handler.rs                  # (unchanged)
web/
  src/
    App.tsx                         # PR 4: add routing
    Dashboard.tsx                   # PR 4: document list/create UI
    Editor.tsx                      # PR 4: accept doc ID from route
    collaboration.ts                # PR 4: parameterize doc ID
    useCollaboration.ts             # (minor changes)
docker-compose.yml                  # PR 1: add LocalStack service
```
