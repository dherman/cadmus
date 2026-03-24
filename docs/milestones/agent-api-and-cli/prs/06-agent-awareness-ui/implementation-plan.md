# PR 6: Agent Awareness UI — Implementation Plan

## Prerequisites

- [ ] PR 1 (Agent Token Management) is merged

## Steps

### 1. Update WebSocket handler to set agent awareness fields

- [ ] In `packages/server/src/websocket/handler.rs`, update the initial awareness state to include agent fields:

```rust
let awareness_state = json!({
    "user": {
        "id": auth.user_id.to_string(),
        "name": if auth.is_agent {
            auth.agent_name.clone().unwrap_or_else(|| "Agent".to_string())
        } else {
            user.display_name.clone()
        },
        "color": if auth.is_agent { "#888888".to_string() } else { generate_color(&auth.user_id) },
        "avatar": null,
    },
    "cursor": null,
    "selection": null,
    "isAgent": auth.is_agent,
    "agentStatus": if auth.is_agent { Some("connected") } else { None },
});
```

- [ ] Verify the awareness state is broadcast to all connected clients when an agent connects.

### 2. Update awareness type definitions in frontend

- [ ] In `packages/web/src/`, find where awareness state types are defined and add agent fields:

```typescript
interface AwarenessState {
  user: {
    id: string;
    name: string;
    color: string;
    avatar?: string;
  };
  cursor?: { anchor: number; head: number } | null;
  selection?: { anchor: number; head: number } | null;
  isAgent?: boolean;
  agentStatus?: string | null;
}
```

### 3. Create bot icon component

- [ ] Add a simple bot icon SVG component or use a Unicode symbol (🤖). If using SVG:

```tsx
function BotIcon({ className }: { className?: string }) {
  return (
    <svg className={className} width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
      <path d="M12 2a2 2 0 012 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 017 7v1a2 2 0 01-2 2h-1v2a2 2 0 01-2 2H8a2 2 0 01-2-2v-2H5a2 2 0 01-2-2v-1a7 7 0 017-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 012-2zm-3 9a1.5 1.5 0 100 3 1.5 1.5 0 000-3zm6 0a1.5 1.5 0 100 3 1.5 1.5 0 000-3z" />
    </svg>
  );
}
```

### 4. Update presence indicator rendering

- [ ] Find the component that renders the presence/awareness indicators (likely in `Editor.tsx` or a toolbar component).

- [ ] Update the rendering to distinguish agent clients:

```tsx
{
  awarenessStates.map((state) =>
    state.isAgent ? (
      <div key={state.user.id} className="presence-agent">
        <BotIcon />
        <span className="agent-name">{state.user.name}</span>
        {state.agentStatus && <span className="agent-status">{state.agentStatus}</span>}
      </div>
    ) : (
      <div key={state.user.id} className="presence-user">
        <span className="presence-dot" style={{ backgroundColor: state.user.color }} />
        <span>{state.user.name}</span>
      </div>
    ),
  );
}
```

### 5. Add agent cursor styles

- [ ] In `packages/web/src/editor.css`, add styles for agent cursors:

```css
/* Agent presence indicator */
.presence-agent {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--color-surface-secondary, #f0f0f0);
  font-size: 13px;
}

.agent-name {
  font-weight: 500;
}

.agent-status {
  font-size: 11px;
  color: var(--color-text-secondary, #666);
  font-style: italic;
}

/* Agent cursor in editor (dashed style) */
.yjs-cursor[data-agent='true'] {
  border-left-style: dashed !important;
  border-left-color: #888 !important;
}

.yjs-cursor[data-agent='true'] > .yjs-cursor-label {
  background-color: #888 !important;
}
```

### 6. Update cursor rendering for agent awareness

- [ ] If the Yjs awareness cursor rendering supports custom attributes, add `data-agent="true"` to cursor elements for agent clients. This depends on how `y-prosemirror`'s cursor plugin is configured.

- [ ] If the cursor plugin doesn't support custom attributes natively, consider extending the cursor builder or applying CSS based on the muted color (#888) that agents use.

### 7. Test with an agent WebSocket connection

- [ ] Start the dev stack: `pnpm dev`
- [ ] Create an agent token via the API.
- [ ] Connect via WebSocket using the agent token (can use `wscat` or a simple script):

```bash
wscat -c "ws://localhost:8080/api/docs/{doc-id}/ws?token=cadmus_..."
```

- [ ] Verify in the browser that:
  - The agent appears in the presence indicators with a bot icon.
  - The agent's name shows as the token name.
  - If the agent has a cursor, it renders with dashed style.

### 8. Build and format check

- [ ] Run `pnpm -F @cadmus/web build` — TypeScript compiles without errors.
- [ ] Run `cargo build` in `packages/server/` — compiles without errors.
- [ ] Run `pnpm run format:check` — no formatting issues.

## Verification

- [ ] Agent clients show with bot icon in presence indicators
- [ ] Agent name displays (from token name, not user display name)
- [ ] Agent status text displays when present (e.g., "connected")
- [ ] Human users still show with colored dot and name (no regression)
- [ ] Agent cursors use dashed style and muted color
- [ ] Multiple agents show as separate entries
- [ ] Agent disconnection removes the presence indicator

## Files Modified

| File                                          | Change                                     |
| --------------------------------------------- | ------------------------------------------ |
| `packages/server/src/websocket/handler.rs`    | Set isAgent/agentStatus in awareness state |
| `packages/web/src/Editor.tsx` (or equivalent) | Render agent presence with bot icon        |
| `packages/web/src/editor.css`                 | Add agent awareness styles                 |
