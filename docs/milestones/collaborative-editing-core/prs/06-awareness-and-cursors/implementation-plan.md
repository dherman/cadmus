# PR 6: Awareness & Cursors — Implementation Plan

## Prerequisites

- [x] PR 4 merged (collaborative sync is working — `WebsocketProvider` and `Y.Doc` are connected)
- [x] PR 5 merged (toolbar exists, but this is a soft dependency — cursors work without the toolbar)

## Steps

### Step 1: Install Cursor Extension

- [x] ~~Add `@tiptap/extension-collaboration-cursor` to `web/package.json`~~ — used `yCursorPlugin` from `@tiptap/y-tiptap` directly (already a dependency), wrapped in a custom Tiptap extension. The v2 wrapper package is deprecated and incompatible with Tiptap v3.
- [x] Run `pnpm install`

### Step 2: Generate Random User Identity

- [x] Create `web/src/user-identity.ts`
- [x] Define the color palette (12 distinct colors)
- [x] Define a word list for random names (20–30 adjectives, 20–30 nouns)
- [x] Export `getOrCreateUserIdentity()`:
  - Check `localStorage` for a stored identity (`cadmus-user-name`, `cadmus-user-color`)
  - If not found, generate a random name (adjective + noun) and pick a random color
  - Store in `localStorage`
  - Return `{ name: string, color: string }`
- [x] Export `clearUserIdentity()` for testing purposes

### Step 3: Add Collaboration Cursor Extension to the Editor

- [x] Modify `web/src/Editor.tsx`:
  - Import `CollaborationCursor` from custom `collaboration-cursor-extension.ts`
  - Import the user identity
  - Add the cursor extension to the extensions array
  - The `provider.awareness` is passed from the `WebsocketProvider` (PR 4)

### Step 4: Custom Cursor Renderer

- [x] Create `web/src/cursor-renderer.ts`
- [x] Export a `cursorBuilder` function matching the `yCursorPlugin` builder API
- [x] Pass this to the extension configuration: `cursorBuilder`

### Step 5: Add Cursor Styles

- [x] Add to `web/src/editor.css`:
  - [x] `.collaboration-cursor`: `border-left: 2px solid` (color set inline), `position: relative`
  - [x] `.collaboration-cursor-label`: `position: absolute`, `top: -1.4em`, `left: -1px`, `font-size: 0.75rem`, `color: white`, `padding: 1px 6px`, `border-radius: 3px`, `white-space: nowrap`, `pointer-events: none`, `opacity: 0` by default, `opacity: 1` on parent hover
  - [x] `.yRemoteSelection`: background-color is set inline by the plugin
  - [x] `.yRemoteSelectionHead`: the cursor caret marker

### Step 6: Create Presence User List Component

- [x] Create `web/src/Presence.tsx`
- [x] Accept `provider: WebsocketProvider` as a prop
- [x] Subscribe to `provider.awareness.on('change', ...)`
- [x] Read all awareness states: `provider.awareness.getStates()`
- [x] Filter to states that have a `user` field (skip entries without it)
- [x] Render a compact list

### Step 7: Integrate Presence into the App

- [x] Modify `web/src/App.tsx`:
  - Render `<Presence provider={provider} />` in the header, next to the connection status indicator

### Step 8: Add Presence Styles

- [x] Add to `web/src/editor.css`:
  - [x] `.presence`: flex row, gap, align items center
  - [x] `.presence-user`: flex row, gap, align items center
  - [x] `.presence-dot`: 10px circle, `border-radius: 50%`
  - [x] `.presence-name`: small font, truncate with ellipsis if needed

## Verification

- [x] Open two browser tabs. Each shows a different random name/color
- [x] Tab A's cursor is visible in Tab B (colored line + name label) and vice versa
- [x] Selecting text in Tab A shows a colored highlight in Tab B
- [x] The presence list in the header shows both users with correct colors
- [x] Closing Tab A causes its cursor to disappear from Tab B (after awareness timeout, ~30s)
- [x] Reloading a tab preserves the same user name and color (from sessionStorage)
- [x] Three or more tabs all see each other's cursors simultaneously

## Files Created/Modified

```
web/package.json                  (modified — add cursor extension)
web/src/user-identity.ts          (new)
web/src/cursor-renderer.ts        (new)
web/src/Presence.tsx              (new)
web/src/Editor.tsx                (modified — add CollaborationCursor extension)
web/src/App.tsx                   (modified — render Presence component)
web/src/editor.css                (modified — cursor + presence styles)
```
