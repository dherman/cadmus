# PR 4: Collaborative Sync Integration — Design

## Purpose

Wire the React editor to the Rust server via Yjs, enabling real-time collaborative editing. After this PR, two browser tabs editing the same document URL will see each other's changes in real time. This is the central integration point of the milestone — it connects the frontend (PR 3) to the backend (PR 2) through the Yjs/Yrs CRDT protocol.

## Architecture

```
Browser Tab                                    Rust Server
┌──────────────────────────┐                  ┌──────────────┐
│  Tiptap Editor           │                  │              │
│    ▲                     │                  │  Yrs Doc     │
│    │ ProseMirror txns    │                  │  + Broadcast │
│  y-prosemirror           │                  │  Group       │
│    ▲                     │                  │              │
│    │ Yjs updates         │                  │              │
│  Y.Doc ◄── y-websocket ─┼── WebSocket ────►│              │
│    │       provider      │                  │              │
│  Awareness               │                  │              │
└──────────────────────────┘                  └──────────────┘
```

## Key Design Decisions

### y-prosemirror for CRDT ↔ Editor Binding

The `y-prosemirror` library binds a Yjs `Y.Doc` to a ProseMirror editor state. It synchronizes the ProseMirror document model with a Yjs `Y.XmlFragment`, so that local ProseMirror transactions produce Yjs updates, and remote Yjs updates produce ProseMirror transactions.

This is added to the Tiptap editor via `@tiptap/extension-collaboration`, which wraps `y-prosemirror` in a Tiptap extension. The extension replaces Tiptap's default history plugin with Yjs's undo manager (which understands CRDT operations).

### y-websocket for Network Transport

The `y-websocket` package provides a `WebsocketProvider` that connects a `Y.Doc` to a WebSocket server implementing the y-sync protocol. It handles the sync handshake, update broadcasting, awareness synchronization, and reconnection with exponential backoff.

Configuration for this milestone:

- `url`: `ws://localhost:8080/ws` (the Rust server).
- `roomname`: the document ID (maps to the URL path the server expects).
- `connect: true` (auto-connect on creation).
- No auth token (added in Milestone 3).

### Replacing Editor Content with Yjs State

In PR 3, the editor loads static `content`. With collaboration enabled, the `content` prop is removed — the Y.Doc is the source of truth. On first connection to a new document, the Y.Doc is empty, and the editor shows an empty state. On reconnection, the Y.Doc syncs with the server and the editor renders the current document.

The `@tiptap/extension-collaboration` extension takes care of this automatically: it provides `fragment` (the `Y.XmlFragment` to bind to) instead of `content`.

### Undo/Redo with Yjs UndoManager

Standard ProseMirror history doesn't work with collaborative editing (it would undo other users' changes). The `@tiptap/extension-collaboration` extension replaces it with Yjs's `UndoManager`, which tracks only the local user's operations. Undo/redo keyboard shortcuts (Ctrl+Z, Ctrl+Shift+Z) continue to work as expected.

### Provider Lifecycle

The `WebsocketProvider` is created when the editor mounts and destroyed when it unmounts. In a React context, this is managed in a `useEffect` hook (or within the Tiptap extension lifecycle). The provider is shared between the collaboration extension and the cursor extension (PR 6) via React context or a shared ref.

### Connection State UI

The provider exposes connection state (`connecting`, `connected`, `disconnected`). The UI displays a minimal status indicator so the user knows whether their edits are being synced. This is a small colored dot or text label in the header — not a full connection manager UI.

## Integration Points

| Component | Provided By | Consumed By |
|-----------|------------|-------------|
| `Y.Doc` | Created in collaboration setup | y-prosemirror, y-websocket, awareness |
| `WebsocketProvider` | `y-websocket` | Collaboration extension, awareness, connection status UI |
| `Y.XmlFragment` | `Y.Doc.getXmlFragment('prosemirror')` | `@tiptap/extension-collaboration` |
| `Awareness` | `WebsocketProvider.awareness` | Cursor extension (PR 6) |

## Error Handling

- **Server unreachable:** `y-websocket` retries with exponential backoff (max 2500ms default). The UI shows "Disconnected — retrying..." status.
- **Server crashes mid-session:** Same reconnection behavior. On reconnect, the sync handshake replays and the client recovers the latest state.
- **Network partition:** Edits continue locally (Yjs supports offline editing). On reconnect, local and remote changes merge via CRDT.
