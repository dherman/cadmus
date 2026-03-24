# PR 6: Agent Awareness UI

## Purpose

Render agent clients distinctly in the editor's presence indicators. When an agent connects via WebSocket, it appears with a bot icon and optional status text instead of a colored cursor dot and user name. This gives human users visibility into agent activity on their document.

## Agent Awareness State

PR 1 adds `is_agent` and `agent_name` fields to `AuthUser`. When an agent connects via WebSocket, the server sets its awareness state with these fields:

```json
{
  "user": {
    "id": "user-123",
    "name": "my-coding-agent",
    "color": "#888888",
    "avatar": null
  },
  "cursor": null,
  "selection": null,
  "isAgent": true,
  "agentStatus": "connected"
}
```

Agents that connect via WebSocket can update their `agentStatus` to communicate what they're doing (e.g., `"analyzing document"`, `"generating summary"`, `"idle"`). REST-only agents (CLI, most batch agents) don't have a WebSocket connection and don't appear in awareness — their activity is visible only through document changes.

## Frontend Changes

### Presence indicator list

The existing presence indicator (colored dots with user names in the editor toolbar) is extended:

- **Human users:** colored circle + display name (unchanged).
- **Agent clients:** bot icon (🤖 or SVG icon) + agent name + optional status text.

```tsx
// In the awareness/presence rendering:
{
  isAgent ? (
    <div className="presence-agent">
      <BotIcon />
      <span className="agent-name">{name}</span>
      {agentStatus && <span className="agent-status">{agentStatus}</span>}
    </div>
  ) : (
    <div className="presence-user">
      <span className="presence-dot" style={{ backgroundColor: color }} />
      <span>{name}</span>
    </div>
  );
}
```

### Cursor rendering

Agent cursors (if present) use a distinct style:

- Dashed cursor line instead of solid.
- Bot icon label instead of name label.
- Muted color (#888) instead of a vibrant user color.

Most agents won't have active cursors (they use REST, not real-time editing), but the rendering handles it for agents that do connect via WebSocket.

## Styling

```css
.presence-agent {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--color-surface-secondary);
}

.agent-name {
  font-size: 12px;
  font-weight: 500;
}

.agent-status {
  font-size: 11px;
  color: var(--color-text-secondary);
  font-style: italic;
}

/* Agent cursor in editor */
.yjs-cursor.agent-cursor {
  border-left-style: dashed;
  border-left-color: #888;
}

.yjs-cursor.agent-cursor .yjs-cursor-label {
  background: #888;
}
```

## Server-Side Changes

The WebSocket handler in `handler.rs` already sets initial awareness state when a client connects. This PR adds the `isAgent` and `agentStatus` fields based on the `AuthUser`:

```rust
let awareness_state = json!({
    "user": {
        "id": auth.user_id,
        "name": auth.agent_name.unwrap_or(user.display_name),
        "color": if auth.is_agent { "#888888" } else { random_color() },
    },
    "isAgent": auth.is_agent,
    "agentStatus": if auth.is_agent { "connected" } else { null },
});
```

## What's Not Included

- Agent status updates via a REST endpoint (agents update status only via WebSocket awareness)
- Agent activity log in the UI (showing what changes an agent made — deferred to M8 history)
- Agent-specific notification preferences
- Offline agent status indicators
