# PR 6: Awareness & Cursors — Design

## Purpose

Render colored cursors and selection highlights for all connected users, making the collaborative experience visible and tangible. This is the feature that transforms "two tabs editing a document" into "two people editing a document together." It's also the visual proof that collaboration is working — without cursors, users can't tell whether changes are from another user or from a bug.

## What Awareness Provides

The Yjs Awareness protocol broadcasts ephemeral state (not persisted, not part of the CRDT) between all connected clients. Each client publishes its awareness state, and all other clients receive it. The `WebsocketProvider` from PR 4 already handles awareness transport — this PR consumes the awareness data and renders it.

### Awareness State Shape

Each client broadcasts:

```json
{
  "user": {
    "name": "Alice",
    "color": "#e06c75"
  },
  "cursor": null
}
```

The `cursor` field is managed automatically by `y-prosemirror`'s cursor plugin — it updates whenever the user's ProseMirror selection changes. The `user` object is set once when the client initializes.

## Key Design Decisions

### Random User Identity (This Milestone)

Since there's no auth in this milestone, each browser tab generates a random identity on first load:

- **Name:** a random two-word combination (e.g., "Crimson Falcon", "Blue Otter") or "User {N}".
- **Color:** a random color from a curated palette of 12–16 visually distinct, accessibility-friendly colors.

The identity is stored in `localStorage` so it persists across page reloads (but not across browsers/devices). In Milestone 3, this is replaced with the authenticated user's real name and a deterministic color derived from their user ID.

### Cursor Rendering via y-prosemirror

The `y-prosemirror` package provides a `yCursorPlugin` that handles cursor rendering using ProseMirror decorations. It reads other clients' awareness states and renders:

- A colored **cursor line** (thin vertical bar) at each remote user's cursor position.
- A **name label** above the cursor (shown on hover or always, configurable).
- A colored **selection highlight** when a remote user has text selected.

This plugin is wrapped by `@tiptap/extension-collaboration-cursor`, which provides a cleaner configuration API.

### Customizing Cursor Appearance

The default `yCursorPlugin` rendering is functional but plain. We customize it by providing a `render` function to the collaboration cursor extension:

```typescript
CollaborationCursor.configure({
  provider,
  user: { name, color },
  render: (user) => {
    // Returns a DOM element for the cursor decoration.
    // Cursor line: 2px wide, colored div.
    // Name label: small tag above the cursor with the user's name.
  },
})
```

The custom render function gives us control over the cursor's visual style (thickness, label positioning, animation) without patching y-prosemirror internals.

### User List / Presence Sidebar

In addition to in-editor cursors, the UI displays a small presence indicator showing all connected users. This is rendered from the awareness states and shows each user's name and color dot. It's a lightweight component in the header area — not a full sidebar.

The user list updates in real time as users connect and disconnect. The awareness protocol handles disconnection detection automatically (awareness state is removed after a configurable timeout, default 30 seconds).

## Awareness Lifecycle

1. **On editor mount:** Set local awareness state with the user's name and color.
2. **On selection change:** `yCursorPlugin` automatically updates the cursor position in awareness.
3. **On remote awareness update:** `yCursorPlugin` re-renders remote cursors as ProseMirror decorations.
4. **On remote disconnect:** After the awareness timeout, the remote user's cursor and name disappear. The user list updates.

## Color Palette

A curated set of colors chosen for visual distinctiveness against a white editor background and adequate contrast for the name label text:

```
#e06c75  (red)
#e5c07b  (yellow)
#98c379  (green)
#56b6c2  (cyan)
#61afef  (blue)
#c678dd  (purple)
#d19a66  (orange)
#be5046  (dark red)
#7ec699  (mint)
#f472b6  (pink)
#a78bfa  (violet)
#fb923c  (amber)
```

Colors are assigned randomly at client initialization. Collisions are possible (two users might get the same color) but unlikely with 12 colors and a typical 2–5 user session.

## Performance Considerations

Awareness updates are frequent (every cursor movement). The `yCursorPlugin` batches decoration updates efficiently via ProseMirror's transaction system. The presence user list subscribes to awareness `change` events, which fire at a lower rate than individual cursor updates.
