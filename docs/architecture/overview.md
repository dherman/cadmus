# Architecture Overview

This document provides a high-level overview of Cadmus's architecture. Each subsystem has its own detailed design document linked below.

## System Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Browser Client                           │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │   Tiptap +   │  │  y-prosemirror│  │  y-websocket provider │  │
│  │  ProseMirror │◄─┤  (CRDT bind) │◄─┤  (sync + awareness)   │  │
│  └──────────────┘  └──────────────┘  └───────────┬───────────┘  │
└──────────────────────────────────────────────────┼──────────────┘
                                                   │ WebSocket
                                                   ▼
┌──────────────────────────────────────────────────────────────────┐
│                      Rust Server (Axum)                          │
│                                                                  │
│  ┌────────────────┐  ┌────────────────┐  ┌───────────────────┐  │
│  │  WebSocket     │  │   REST API     │  │  Document Session │  │
│  │  Handler       │  │  (docs, auth,  │  │  Manager          │  │
│  │  (Yrs sync +   │  │   comments,   │  │  (load/flush/     │  │
│  │   permissions) │  │   agents)     │  │   unload)         │  │
│  └───────┬────────┘  └───────┬────────┘  └─────────┬─────────┘  │
│          │                   │                      │            │
│          └───────────────────┼──────────────────────┘            │
│                              │                                   │
│                    ┌─────────▼─────────┐                        │
│                    │  Yrs Documents    │                        │
│                    │  (in-memory CRDT) │                        │
│                    └─────────┬─────────┘                        │
└──────────────────────────────┼───────────────────────────────────┘
               │               │                │
               ▼               ▼                ▼
        ┌────────────┐  ┌───────────┐   ┌──────────────┐
        │  Sidecar   │  │ PostgreSQL│   │     S3       │
        │  (Node)    │  │ (metadata,│   │ (CRDT blobs, │
        │            │  │  comments,│   │  update log) │
        │ serialize  │  │  perms)   │   │              │
        │ parse      │  └───────────┘   └──────────────┘
        │ diff       │
        └────────────┘
```

## Core Technology Choices

**CRDT library: Yjs (client) / Yrs (server).** Yjs is the most battle-tested collaborative editing CRDT in the JavaScript ecosystem. Yrs is its official Rust port, maintained by the same team. This gives native CRDT processing on both sides without a translation layer.

**Editor framework: Tiptap (wrapping ProseMirror).** Tiptap provides a modular extension system over ProseMirror's strict document model. It's a transparent wrapper — ProseMirror internals are accessible via `@tiptap/pm` and this is an intended, supported usage pattern.

**Markdown codec: @tiptap/markdown.** Official open-source Tiptap extension (introduced in v3.7.0) providing bidirectional markdown parsing and serialization. Uses MarkedJS for tokenization (CommonMark-compliant). Each Tiptap extension defines its own `parseMarkdown` and `renderMarkdown` hooks, making the codec modular and co-located with the schema.

**Backend framework: Axum.** Rust async web framework with tower middleware ecosystem. WebSocket support for Yrs sync via `yrs-axum` crate.

**WebSocket sync protocol: y-sync.** The standard Yjs synchronization protocol — SyncStep1/SyncStep2/Update message types for document sync, plus Awareness for presence/cursors. The server implements a custom `Protocol` trait for permission enforcement.

## Subsystem Design Documents

- [CRDT Foundation & Sync Protocol](websocket-protocol.md) — Yjs/Yrs sync, connection lifecycle, permission enforcement, awareness
- [Schema Design](schema-design.md) — ProseMirror/Tiptap node and mark types, content expressions, markdown serialization style
- [Node Sidecar](node-sidecar.md) — Markdown↔JSON conversion service, ProseMirror Step diffing, deployment model
- [Agent API](agent-api.md) — REST contract for agent and CLI interactions, token auth, read/write/comment endpoints
- [Comments](comments.md) — Comment data model, anchoring via RelativePositions, REST + WebSocket notification pattern
- [CLI & Local Tools](cli-local-tools.md) — Checkout/push workflow, diff/merge strategy, `.cadmus/` metadata
- [Enterprise](enterprise.md) — Org/workspace hierarchy, agent controls, audit logging
