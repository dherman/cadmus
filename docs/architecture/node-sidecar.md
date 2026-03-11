# Node Sidecar

## Purpose

The sidecar exists to solve one problem: the Rust server needs to convert between Yrs document state (ProseMirror JSON) and canonical markdown. Rather than reimplementing Tiptap's markdown codec in Rust — which would be a maintenance burden and a source of subtle serialization divergence — we run a small Node service that uses the _exact same schema code_ as the frontend editor.

## Responsibilities

The sidecar is a stateless conversion service. It does not manage CRDT state, handle WebSocket connections, or know about auth. It exposes three operations:

1. **Serialize:** ProseMirror JSON → canonical markdown string
2. **Parse:** Markdown string → ProseMirror JSON
3. **Diff:** Given two ProseMirror JSON documents (`old_doc`, `new_doc`), compute the ProseMirror `Step` sequence that transforms one into the other

## Why a sidecar, not a Lambda

We evaluated deploying this as an AWS Lambda function. The sidecar wins on:

- **Schema deployment consistency.** The sidecar is deployed atomically with the server (same ECS task definition). Schema changes take effect simultaneously in both services. Lambda would require coordinating two independent deployments with a version mismatch window.
- **Latency.** Localhost HTTP is sub-millisecond overhead. Lambda adds cold starts (hundreds of ms for a Node function with Tiptap's dependency tree) and cross-service network hops.
- **Debuggability.** `docker exec` into the sidecar container and test with curl. Lambda requires CloudWatch log diving.

Lambda would be appropriate if this service were independently owned/scaled. It isn't — it's a tightly-coupled codec that exists to share code with the frontend.

## Interface

Internal HTTP API, accessible only from within the same ECS task (not exposed through the load balancer):

```
POST /serialize
  Body: { "doc": <ProseMirror JSON>, "schema_version": 1 }
  Response: { "markdown": "<string>" }

POST /parse
  Body: { "markdown": "<string>", "schema_version": 1 }
  Response: { "doc": <ProseMirror JSON> }

POST /diff
  Body: { "old_doc": <ProseMirror JSON>, "new_doc": <ProseMirror JSON> }
  Response: { "steps": [<ProseMirror Step JSON>, ...] }

GET /health
  Response: { "ok": true, "schema_version": 1 }
```

The `schema_version` field enables the server to verify it's talking to a compatible sidecar. If the versions mismatch (possible during a rolling deployment race), the server can return a 503 to the client rather than producing corrupt data.

## The Diff Endpoint

This is the most important endpoint. When a CLI user or agent pushes modified markdown, the server needs to compute what changed and apply it to the live CRDT document. The flow:

1. Server receives pushed markdown + `base_version` reference.
2. Server loads the ProseMirror JSON at `base_version` (`old_doc`).
3. Server sends the pushed markdown to sidecar's `/parse` → gets `new_doc`.
4. Server sends `old_doc` and `new_doc` to sidecar's `/diff` → gets a sequence of ProseMirror `Step`s.
5. Server translates each Step into Yrs operations and applies them to the current live document.

The diff computation uses `prosemirror-recreate-transform` (or equivalent) to compute Steps between two document states. This approach leverages ProseMirror's native transform model rather than operating on raw text diffs, which produces more semantically meaningful operations.

Using ProseMirror Steps is not breaking the Tiptap abstraction — Tiptap is explicitly designed as a transparent wrapper over ProseMirror, and accessing ProseMirror internals via `@tiptap/pm` is a first-class supported pattern.

## Shared Schema Package

The critical architectural invariant: the sidecar imports the document schema from the same shared package as the frontend.

```
packages/
  doc-schema/           # Shared package — THE source of truth
    src/
      extensions.ts     # Configured Tiptap extension array
      index.ts          # Public API
    package.json
  sidecar/
    (imports @cadmus/doc-schema)
  web/
    (imports @cadmus/doc-schema)
```

This ensures serialization consistency by construction. If a developer adds a Tiptap extension or modifies a markdown serialization hook, both the frontend and sidecar pick up the change automatically.

## Deployment

**Prototype:** A second container in the same ECS Fargate task definition as the Rust server. Communicates over localhost. The sidecar container has its own health check (`GET /health`).

**Production:** If serialization workload needs independent scaling, extract to a separate ECS service with service discovery. No code changes required — only infrastructure.

## The ProseMirror Step → Yrs Translation Layer

The sidecar produces ProseMirror Steps. The Rust server consumes them and translates to Yrs operations. This translation is a bounded problem — ProseMirror has a fixed set of Step types:

- `ReplaceStep` — insert, delete, or replace a range of content
- `ReplaceAroundStep` — wrap/unwrap content (e.g., toggling a blockquote)
- `AddMarkStep` / `RemoveMarkStep` — apply or remove formatting
- `AddNodeMarkStep` / `RemoveNodeMarkStep` — marks on node boundaries
- `AttrStep` — change node attributes

Each of these maps to specific Yrs XML/Text operations. The translation layer is written in Rust and lives in the server crate.
