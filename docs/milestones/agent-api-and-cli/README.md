# Milestone: Agent API and CLI

## Goal

Non-browser clients can read, write, and comment on documents. A developer or AI agent can check out a document as markdown, edit it locally with any tool, and push changes back — with the server handling three-way merge via the CRDT.

Milestone 5 gave us comments and Milestone 4 proved the sidecar pipeline in the read direction. But the write direction — accepting markdown from external clients, diffing it against the current state, and applying changes to the live CRDT — is the missing piece. This milestone implements the full checkout→edit→push cycle: agent token management for programmatic auth, the content push endpoint with sidecar-powered diffing and ProseMirror Step → Yrs translation, a working CLI tool, and agent presence indicators in the frontend.

## Success Criteria

- Users can create scoped API tokens via REST, with optional document restrictions, permission ceilings, and expiration.
- Tokens can be listed and revoked. The secret is shown only once at creation time.
- `POST /api/docs/{id}/content` accepts markdown with a `base_version`, diffs against the base via the sidecar, translates ProseMirror Steps to Yrs operations, and applies to the live document.
- The push endpoint returns the new version ID and a change summary.
- `?dry_run=true` returns a preview diff and any detected conflicts without applying changes.
- Concurrent edits (push while browser users are editing) merge cleanly when changes are to different regions.
- The CLI authenticates with email/password, stores credentials in `~/.config/cadmus/credentials`.
- `cadmus docs list` shows accessible documents in a formatted table.
- `cadmus checkout <doc-id>` downloads markdown and records the version in `.cadmus/<doc-id>.json`.
- `cadmus push <doc-id> <file>` reads the base version from `.cadmus/`, pushes changes, and updates the recorded version on success.
- `cadmus push --dry-run` shows the diff preview without applying.
- Agent tokens authenticate via Bearer header on all REST endpoints and via `token` query param on WebSocket.
- Agent clients appear in the editor's presence indicators with a bot icon and optional status text.

## Scope Boundaries

**In scope:** Agent token CRUD (create, list, revoke), token auth middleware (accept agent tokens alongside user JWTs), content push endpoint with sidecar diffing, ProseMirror Step → Yrs translation layer, dry-run mode, CLI implementation (auth login, docs list, checkout, push with --dry-run), `.cadmus/` metadata directory, agent awareness rendering in frontend (bot icon + status text).

**Out of scope:** Targeted edits API (Pattern 2 from architecture doc — deferred until agent usage patterns are clear), real-time WebSocket agent editing (the infrastructure supports it but CLI uses REST), CLI comment command (the endpoint exists from M5 but CLI comment UX needs more design — can be added as a fast follow), rate limiting and change size limits (deferred to M7 enterprise controls), audit logging (M7), organization-level token policies (M7).

## PR Breakdown

The milestone is divided into six PRs, ordered by dependency. Each PR is independently mergeable and produces a testable artifact.

| PR  | Title                                                                 | Depends On                     | Estimated Effort |
| --- | --------------------------------------------------------------------- | ------------------------------ | ---------------- |
| 1   | [Agent Token Management](prs/01-agent-token-management/)              | —                              | 2–3 days         |
| 2   | [Content Push Endpoint](prs/02-content-push-endpoint/)                | PR 1 (agent auth)              | 2–3 days         |
| 3   | [ProseMirror Step → Yrs Translation](prs/03-step-to-yrs-translation/) | PR 2 (push endpoint structure) | 2–3 days         |
| 4   | [CLI Auth & Checkout](prs/04-cli-auth-and-checkout/)                  | PR 1 (token auth works)        | 1–2 days         |
| 5   | [CLI Push & Dry-Run](prs/05-cli-push-and-dry-run/)                    | PR 2 + PR 3 (push works)       | 1–2 days         |
| 6   | [Agent Awareness UI](prs/06-agent-awareness-ui/)                      | PR 1 (agent identity in auth)  | 1 day            |

PRs 2 and 4 can be developed in parallel after PR 1 lands — PR 2 builds the server-side push endpoint while PR 4 builds the CLI client that calls existing read endpoints. PR 3 completes the push endpoint's core logic (Step → Yrs translation). PR 5 requires both the working push endpoint (PR 2 + PR 3) and the CLI foundation (PR 4). PR 6 is a small frontend-only change that can be done any time after PR 1.

```
PR 1 (Agent Token Management)
 ├── PR 2 (Content Push Endpoint) ──── can run in parallel with PR 4
 │    └── PR 3 (Step → Yrs Translation)
 │         └── PR 5 (CLI Push & Dry-Run) ← also depends on PR 4
 ├── PR 4 (CLI Auth & Checkout) ────── can run in parallel with PR 2
 └── PR 6 (Agent Awareness UI) ────── independent after PR 1
```

## Architecture Context

Refer to the main architecture docs for full context:

- [Architecture Overview](../../architecture/overview.md) — system diagram, agent placement
- [Agent API](../../architecture/agent-api.md) — token model, REST contract for read/write/comment, dry-run mode, rate limiting
- [CLI & Local Tools](../../architecture/cli-local-tools.md) — checkout/push workflow, `.cadmus/` metadata, push merge strategy
- [Node Sidecar](../../architecture/node-sidecar.md) — diff endpoint, Step → Yrs translation layer
- [WebSocket Sync Protocol](../../architecture/websocket-protocol.md) — awareness state schema (agent fields)

## Key Technical Decisions for This Milestone

**Agent tokens, not OAuth apps.** Agent authentication uses scoped bearer tokens created by users, not a separate OAuth application model. This is simpler, aligns with the "agents act on behalf of users" philosophy, and avoids the complexity of OAuth flows for non-browser clients. Each token is tied to a user, inherits their permissions (with optional restrictions), and can be revoked independently.

**Dual auth middleware.** The existing `AuthUser` extractor validates JWT access tokens. For M6, we extend it to also accept agent tokens (a different format, looked up in the database). The handler receives an `AuthUser` either way — it doesn't need to know whether the caller is a browser or agent. Agent tokens include an `is_agent` flag and optional `agent_name` that flow through to awareness state.

**Three-way merge via sidecar diffing.** The push endpoint implements: (1) parse pushed markdown via sidecar → `new_doc`, (2) load ProseMirror JSON at `base_version` → `old_doc`, (3) diff `old_doc` vs `new_doc` via sidecar → ProseMirror Steps, (4) translate Steps to Yrs operations and apply to the current live document. The `base_version` enables three-way merge — the diff captures the agent's intent relative to what it last saw, and the CRDT handles concurrent edits from browser users.

**ProseMirror Step → Yrs translation is a bounded problem.** ProseMirror has a fixed set of Step types (ReplaceStep, ReplaceAroundStep, AddMarkStep, RemoveMarkStep, AddNodeMarkStep, RemoveNodeMarkStep, AttrStep). Each maps to specific Yrs XML/Text operations. The translation layer is written in Rust and operates on the Step JSON emitted by the sidecar's `/diff` endpoint. This is the most technically complex piece of the milestone but is well-defined and testable in isolation.

**Version tracking via document snapshots.** Each flush to S3 produces a version identifier. The content-read endpoint returns the current version, which becomes the `base_version` for the next push. The CLI stores this in `.cadmus/<doc-id>.json`. If the document has changed since checkout, the three-way merge handles it; the dry-run mode lets the user preview the merge result before applying.

**CLI stores credentials and checkout state locally.** Credentials go in `~/.config/cadmus/credentials` (platform-standard config directory). Checkout metadata goes in `.cadmus/` relative to the output file (analogous to `.git/`). The CLI reads these automatically — no flags needed after initial setup.

## Repository Changes (End State of This Milestone)

```
packages/
  server/
    migrations/
      20260312000001_initial.sql              # (unchanged)
      20260312000002_users.sql                # (unchanged)
      20260312000003_comments.sql             # (unchanged)
      20260324000001_agent_tokens.sql         # PR 1: agent_tokens table
    src/
      main.rs                                 # (unchanged)
      lib.rs                                  # PR 1: add token routes, PR 2: push_content wired up
      db.rs                                   # PR 1: token query methods
      sidecar.rs                              # (unchanged — client already exists)
      auth/
        jwt.rs                                # PR 1: extend to validate agent tokens
        middleware.rs                          # PR 1: dual auth (JWT + agent token)
        handlers.rs                           # PR 1: token CRUD handlers
      documents/
        api.rs                                # PR 2: implement push_content handler
        mod.rs                                # (unchanged)
        yrs_json.rs                           # (unchanged)
        step_translator.rs                    # PR 3: ProseMirror Step → Yrs operations
        storage.rs                            # PR 2: version tracking additions
      websocket/
        handler.rs                            # PR 1: accept agent tokens on WS upgrade
        protocol.rs                           # (unchanged)
        events.rs                             # (unchanged)
  cli/
    src/
      main.ts                                 # PR 4+5: full CLI implementation
      api.ts                                  # PR 4: HTTP client for Cadmus server
      auth.ts                                 # PR 4: credential storage and login flow
      config.ts                               # PR 4: config/metadata file management
    package.json                              # PR 4: add dependencies (inquirer, chalk, etc.)
    tsconfig.json                             # PR 4: TypeScript config
  web/
    src/
      Editor.tsx                              # PR 6: render agent presence with bot icon
      editor.css                              # PR 6: agent awareness styles
```
