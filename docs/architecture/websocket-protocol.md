# WebSocket Sync Protocol

## Overview

Real-time collaboration uses the standard Yjs sync protocol (y-sync) over WebSocket, implemented on the server with `yrs-axum`. The server acts as the authoritative merge point — clients sync through it, not peer-to-peer.

## Protocol Layers

### Layer 1: Connection Establishment and Auth

1. Client obtains a short-lived JWT from `POST /api/auth/ws-token` (standard REST auth).
2. Client opens WebSocket to `wss://{host}/api/docs/{doc_id}/ws?token={jwt}`.
3. Server validates the JWT in the Axum handler _before_ upgrading the connection. Invalid token → 401, no upgrade.
4. From the JWT claims, the server determines: user ID, permission level (Read/Comment/Edit), whether the client is an agent (plus `agent_id` if so).
5. Server upgrades the connection and attaches the client to the document's `BroadcastGroup`.

### Layer 2: Y-Sync Handshake

After the WebSocket opens, the standard y-sync initial sync occurs:

```
Client                          Server
  |-- SyncStep1(stateVector) ---->|  client: "here's what I have"
  |<-- SyncStep2(missingUpdates) -|  server: "here's what you need"
  |<-- SyncStep1(stateVector) ----|  server: "what do you have that I don't?"
  |-- SyncStep2(missingUpdates) ->|  client: responds
```

After sync completes, both sides exchange Awareness updates (presence, cursors).

### Layer 3: Steady-State Editing

```
Client → Server:  Update(binary)      // user typed something
Server → Client:  Update(binary)      // broadcast from another client
Client → Server:  Awareness(json)     // cursor moved
Server → Client:  Awareness(json)     // other cursors
Server → Client:  Custom(CommentEvent) // comment created/updated (see below)
```

### Layer 4: Permission Enforcement

Implemented via a custom `Protocol` trait in Rust that wraps `DefaultProtocol`:

- **All roles (Read/Comment/Edit):** receive all Sync and Update messages (needed to render the document), send and receive Awareness updates (needed for presence).
- **Edit only:** can send Update messages (document edits). Read and Comment users' Update messages are rejected (server sends `Auth(denied)` or silently drops).
- **Comment and Edit:** can receive `Custom(CommentEvent)` notifications. Comment creation/mutation happens over REST, not the CRDT.

Key insight: permission enforcement happens on _incoming_ messages, not outgoing. All clients receive the full update stream.

### Layer 5: Comment Notifications

Comments are managed via REST (see [Comments](comments.md)), but connected clients need real-time notification when comments are created, updated, or resolved. We use y-sync's custom message support:

- Server broadcasts `Custom(COMMENT_EVENT_TAG, payload)` to all connected clients on a document when a comment mutation occurs.
- Payload is JSON: `{ "type": "created" | "updated" | "resolved" | "unresolve" | "deleted", "comment": { ... } }`.
- Clients update their local comment state from these events. Initial comment state is fetched via REST on connect.

### Layer 6: Reconnection

The client-side `y-websocket` provider handles reconnection with exponential backoff (configurable `maxBackoffTime`, default 2500ms). On reconnect, the full sync handshake replays — the client sends its state vector, the server sends any missed updates.

**Token expiry during long sessions:** If the JWT expires, the server closes the WebSocket with close code `4401`. The client intercepts this, fetches a new token from `POST /api/auth/ws-token`, and reconnects with the new URL.

## Document Session Manager

The server maintains an in-memory map of active document sessions: `DashMap<DocId, Arc<DocumentSession>>`.

### DocumentSession contains:

- Yrs `Doc` instance (the authoritative CRDT state)
- `Awareness` instance
- `BroadcastGroup` managing connected client subscriptions
- Metadata: connected client count, last activity timestamp, flush state

### Lifecycle:

1. **Load:** First client connects → load Yrs state from S3 (compacted snapshot) + apply any un-compacted updates from the update log. Initialize `BroadcastGroup`.
2. **Flush:** Periodically (every 5 seconds of inactivity or every N updates), compact the Yrs document and write the snapshot to S3. Append raw updates to the update log between compactions.
3. **Unload:** Last client disconnects → start a 60-second grace timer. If no reconnection, flush final state and drop the session from memory.

### Persistence strategy (prototype):

- **Write-behind:** Updates are applied to the in-memory Yrs doc and broadcast immediately. Persistence is async/periodic.
- **Acceptable trade-off:** On server crash, up to 5 seconds of edits may be lost. This is acceptable for a prototype; write-through can be added later for enterprise durability guarantees.

## Awareness State Schema

Each connected client broadcasts awareness state as JSON:

```json
{
  "user": {
    "id": "user-123",
    "name": "Alice",
    "color": "#e06c75",
    "avatar": "https://..."
  },
  "cursor": { "anchor": 142, "head": 142 },
  "selection": { "anchor": 100, "head": 150 },
  "isAgent": false,
  "agentStatus": null
}
```

For agent clients, `isAgent: true` and `agentStatus` can contain a description of what the agent is doing (e.g., `"analyzing section 3"`, `"generating summary"`). The frontend renders agents with a distinct visual indicator.

## Scaling Considerations (Future)

For the prototype, a single server instance handles all documents. For production scale:

- **Option A (PubSub):** Multiple server instances, each handles any document. Document updates are distributed via a PubSub system (Redis). A document may be active on multiple servers. Simple, fault-tolerant, but higher memory usage.
- **Option B (Sharding):** Each document is assigned to a specific server via consistent hashing. Requires a coordination service (etcd) for health checks and routing. Better resource efficiency, but more complex.

The prototype architecture (single instance, in-memory sessions) is designed so either scaling approach can be adopted later without changing the client protocol.
