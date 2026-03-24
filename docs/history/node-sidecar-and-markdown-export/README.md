# Milestone: Node Sidecar and Markdown Export

## Goal

Proven markdown↔JSON conversion pipeline via the sidecar, with REST endpoints for reading document content and an "Export as Markdown" button in the frontend.

Milestone 3 gave us a multi-user system with authentication and permissions — but there's no way to get document content out of the CRDT except by viewing it in the browser. This milestone completes the sidecar's diff implementation, wires the content-read endpoints through the sidecar, and gives users a one-click markdown export. After this milestone, documents can be read programmatically in both JSON and markdown formats, and the sidecar architecture is validated end-to-end with a comprehensive integration test suite.

## Success Criteria

- The sidecar's `/diff` endpoint computes correct ProseMirror Step sequences between two document states using `prosemirror-recreate-transform`.
- `GET /api/docs/{id}/content?format=markdown` returns the document's canonical markdown representation via the sidecar.
- `GET /api/docs/{id}/content?format=json` returns the document's ProseMirror JSON representation directly from the Yrs document.
- Both content endpoints respect permissions — only users with Read (or higher) access can fetch content.
- The "Export as Markdown" button in the editor downloads a `.md` file of the current document.
- Round-trip integration tests validate fidelity: create content in the editor schema → serialize to markdown → parse back → compare JSON.
- Round-trip tests cover all schema constructs: headings, lists (bullet + ordered, nested), code blocks, blockquotes, images, horizontal rules, all marks (bold, italic, code, link, strike), hard breaks.
- The sidecar's diff output produces semantically correct Steps for insert, delete, reformat, and restructure operations.

## Scope Boundaries

**In scope:** Diff endpoint implementation (`prosemirror-recreate-transform`), content-read REST endpoints (JSON + markdown), sidecar round-trip integration tests, frontend export button, sidecar unit tests for serialize/parse/diff.

**Out of scope:** `POST /api/docs/{id}/content` (push/merge — deferred to M6 Agent API), ProseMirror Step → Yrs translation layer (deferred to M6), Docker container definition for sidecar (exists as dev process; production containerization can come with M6 or M7), version-pinned content reads (the `version` query parameter — deferred to M8 History UI).

## PR Breakdown

The milestone is divided into four PRs, ordered by dependency. Each PR is independently mergeable and produces a testable artifact.

| PR  | Title                                                                | Depends On               | Estimated Effort |
| --- | -------------------------------------------------------------------- | ------------------------ | ---------------- |
| 1   | [Diff Endpoint Implementation](prs/01-diff-endpoint-implementation/) | —                        | 1–2 days         |
| 2   | [Content Read Endpoints](prs/02-get-content-endpoint/)               | PR 1 (sidecar serialize) | 1–2 days         |
| 3   | [Sidecar Integration Tests](prs/03-sidecar-integration-tests/)       | PR 1 + PR 2              | 1 day            |
| 4   | [Export UI](prs/04-export-ui/)                                       | PR 2 (content endpoint)  | 1 day            |

PRs 1 and 2 are partially independent — PR 2's markdown format relies on the sidecar's `/serialize` endpoint (which already works), not on the `/diff` endpoint from PR 1. However, PR 1 should land first so that PR 3's integration tests can cover all three sidecar endpoints.

```
PR 1 (Diff Endpoint)
 └── PR 2 (Content Read Endpoints) ─── can start in parallel (serialize already works)
      ├── PR 3 (Integration Tests) ←── also depends on PR 1
      └── PR 4 (Export UI)
```

## Architecture Context

Refer to the main architecture docs for full context:

- [Architecture Overview](../../architecture/overview.md) — system diagram, sidecar placement
- [Node Sidecar](../../architecture/node-sidecar.md) — sidecar interface, diff endpoint design, shared schema package, deployment model
- [Schema Design](../../architecture/schema-design.md) — launch schema definition, canonical markdown style, round-trip fidelity requirements

## Key Technical Decisions for This Milestone

**`prosemirror-recreate-transform` for diffing.** The sidecar's `/diff` endpoint uses this library to compute ProseMirror Steps between two document states. This is the standard approach in the ProseMirror ecosystem — it reconstructs a Transform (sequence of Steps) that converts one document into another. The library handles all Step types (ReplaceStep, AddMarkStep, etc.) and produces semantically meaningful operations rather than raw text diffs.

**Content read via session manager + sidecar.** The `get_content` handler loads the document from the session manager (or from S3 + update log if not in memory), extracts ProseMirror JSON from the Yrs XML fragment, and optionally sends it through the sidecar's `/serialize` endpoint for markdown conversion. This ensures the content returned is always consistent with the live CRDT state.

**Yrs XML → ProseMirror JSON extraction.** The Yrs document stores content in an XML-like CRDT structure that mirrors ProseMirror's node tree. To extract ProseMirror JSON, we read the Yrs XML fragment and convert it to the JSON format that Tiptap/ProseMirror understands. This is a bounded translation — the Yrs XML structure maps directly to ProseMirror's node/mark/text model.

**Push endpoint deferred to M6.** The `POST /api/docs/{id}/content` endpoint (which would accept markdown, diff it against the current state, and apply changes via Yrs) requires the ProseMirror Step → Yrs translation layer. This is a significant piece of work that's better aligned with M6 (Agent API), where it's actually needed. For this milestone, we validate the sidecar pipeline in the read direction only.

**Export as file download.** The frontend export button fetches markdown via the content endpoint and triggers a browser file download. No server-side file generation — the client receives the markdown string and creates a Blob URL. This keeps the implementation simple and avoids server-side temp files.

## Repository Changes (End State of This Milestone)

```
packages/
  sidecar/
    src/
      server.ts                    # (unchanged)
      serialize.ts                 # (unchanged — already works)
      parse.ts                     # (unchanged — already works)
      diff.ts                      # PR 1: implement with prosemirror-recreate-transform
    __tests__/
      serialize.test.ts            # PR 3: unit tests for serialization
      parse.test.ts                # PR 3: unit tests for parsing
      diff.test.ts                 # PR 1: unit tests for diff
      round-trip.test.ts           # PR 3: round-trip fidelity tests
    package.json                   # PR 1: add prosemirror-recreate-transform dependency
  server/
    src/
      documents/
        api.rs                     # PR 2: implement get_content handler
        yrs_json.rs                # PR 2: Yrs XML → ProseMirror JSON extraction
      sidecar.rs                   # (unchanged — client already exists)
  web/
    src/
      api.ts                       # PR 4: add fetchDocumentContent function
      EditorPage.tsx               # PR 4: add Export button
```
