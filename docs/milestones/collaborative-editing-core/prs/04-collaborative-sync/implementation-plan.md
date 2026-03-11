# PR 4: Collaborative Sync Integration — Implementation Plan

## Prerequisites

- [ ] PR 2 merged (Rust WebSocket server is running and accepting connections)
- [ ] PR 3 merged (React editor renders and is functional in standalone mode)

## Steps

### Step 1: Install Collaboration Dependencies

- [ ] Add to `web/package.json`:
  - `yjs` — the core CRDT library
  - `y-websocket` — WebSocket transport provider
  - `y-prosemirror` — ProseMirror ↔ Yjs binding (peer dependency of `@tiptap/extension-collaboration`)
  - `@tiptap/extension-collaboration` — Tiptap wrapper for y-prosemirror
- [ ] Run `pnpm install`

### Step 2: Create Collaboration Provider Module

- [ ] Create `web/src/collaboration.ts`:
  - Export a function `createCollaborationProvider(docId: string)` that:
    - Creates a new `Y.Doc`
    - Creates a `WebsocketProvider` connecting to `ws://localhost:8080/ws/${docId}`
    - Returns `{ ydoc, provider }`
  - Export a function `destroyCollaborationProvider(provider)` that disconnects and cleans up
  - Export a constant `DEFAULT_DOC_ID = 'default'` (matching the server's pre-seeded document)
- [ ] Create `web/src/useCollaboration.ts` — a React hook:
  ```typescript
  export function useCollaboration(docId: string) {
    // Creates ydoc + provider on mount, destroys on unmount.
    // Returns { ydoc, provider, isConnected }.
    // Tracks connection state via provider status events.
  }
  ```
  - Use `useEffect` for setup/teardown
  - Use `useState` for `isConnected`, updated via the provider's `'status'` event

### Step 3: Update the Editor to Use Collaboration

- [ ] Modify `web/src/Editor.tsx`:
  - Accept `ydoc` and `provider` as props (or consume from context)
  - Update the `useEditor` call:
    - Remove the `content` prop
    - Add `Collaboration.configure({ document: ydoc, field: 'prosemirror' })` to the extensions array
    - The `Collaboration` extension replaces the built-in history with Yjs UndoManager
  - The extension array becomes: `[...schemaExtensions, Collaboration.configure({ document: ydoc })]`

  Note: The `extensions` from `@cadmus/doc-schema` include StarterKit, which includes the History extension. When Collaboration is added, it automatically disables the History extension (this is handled by Tiptap internally — `Collaboration` sets `history: false` on StarterKit).

### Step 4: Update the App to Manage Collaboration Lifecycle

- [ ] Modify `web/src/App.tsx`:
  - Use the `useCollaboration` hook:
    ```tsx
    const { ydoc, provider, isConnected } = useCollaboration(DEFAULT_DOC_ID);
    ```
  - Pass `ydoc` and `provider` to `<Editor />`
  - Render a connection status indicator in the header:
    ```tsx
    <span className={`status-dot ${isConnected ? 'connected' : 'disconnected'}`} />
    ```

### Step 5: Add Connection Status Styling

- [ ] Update `web/src/editor.css`:
  - `.status-dot`: small circle (8px), positioned in the header
  - `.status-dot.connected`: green background
  - `.status-dot.disconnected`: red background with a subtle pulse animation

### Step 6: Environment Configuration

- [ ] Create `web/.env.example`:
  ```
  VITE_WS_URL=ws://localhost:8080/ws
  ```
- [ ] Update `collaboration.ts` to read from `import.meta.env.VITE_WS_URL`
- [ ] Add `.env` to `.gitignore`

### Step 7: Manual Integration Testing

- [ ] Start the Rust server: `cd server && cargo run`
- [ ] Start the frontend: `cd web && pnpm dev`
- [ ] Open two browser tabs to `http://localhost:5173`
- [ ] Type in Tab A. Verify text appears in Tab B within ~100ms
- [ ] Type simultaneously in both tabs. Verify no content duplication or loss
- [ ] Kill the server. Verify the UI shows "disconnected." Continue typing in both tabs
- [ ] Restart the server. Verify both tabs reconnect and their offline edits merge
- [ ] Test undo (Ctrl+Z) in Tab A — it should only undo Tab A's changes, not Tab B's

### Step 8: Add Dev Script for Full-Stack Development

- [ ] Add to root `package.json`:
  ```json
  "dev": "concurrently \"pnpm dev:server\" \"pnpm dev:web\"",
  "dev:server": "cd server && cargo run",
  "dev:web": "pnpm -F @cadmus/web dev"
  ```
- [ ] Install `concurrently` as a root dev dependency

## Verification

- [ ] `pnpm dev` starts both server and frontend
- [ ] Two tabs sync edits in real time
- [ ] Connection status indicator works
- [ ] Undo/redo only affects local changes
- [ ] Offline edits merge correctly on reconnect
- [ ] No console errors in the browser

## Files Created/Modified

```
web/package.json                  (modified — add yjs deps)
web/src/collaboration.ts          (new)
web/src/useCollaboration.ts       (new)
web/src/Editor.tsx                (modified — add Collaboration extension)
web/src/App.tsx                   (modified — collaboration lifecycle + status)
web/src/editor.css                (modified — status indicator styles)
web/.env.example                  (new)
package.json                      (modified — add concurrently, dev script)
```
