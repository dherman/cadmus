# PR 6: Awareness & Cursors — Implementation Plan

## Prerequisites

- [ ] PR 4 merged (collaborative sync is working — `WebsocketProvider` and `Y.Doc` are connected)
- [ ] PR 5 merged (toolbar exists, but this is a soft dependency — cursors work without the toolbar)

## Steps

### Step 1: Install Cursor Extension

- [ ] Add `@tiptap/extension-collaboration-cursor` to `web/package.json` — Tiptap wrapper for y-prosemirror's cursor plugin
- [ ] Run `pnpm install`

### Step 2: Generate Random User Identity

- [ ] Create `web/src/user-identity.ts`
- [ ] Define the color palette (12 distinct colors)
- [ ] Define a word list for random names (20–30 adjectives, 20–30 nouns)
- [ ] Export `getOrCreateUserIdentity()`:
  - Check `localStorage` for a stored identity (`cadmus-user-name`, `cadmus-user-color`)
  - If not found, generate a random name (adjective + noun) and pick a random color
  - Store in `localStorage`
  - Return `{ name: string, color: string }`
- [ ] Export `clearUserIdentity()` for testing purposes

### Step 3: Add Collaboration Cursor Extension to the Editor

- [ ] Modify `web/src/Editor.tsx`:
  - Import `CollaborationCursor` from `@tiptap/extension-collaboration-cursor`
  - Import the user identity
  - Add the cursor extension to the extensions array:
    ```typescript
    CollaborationCursor.configure({
      provider,
      user: { name: identity.name, color: identity.color },
    })
    ```
  - The `provider` is the `WebsocketProvider` from the collaboration setup (PR 4)

### Step 4: Custom Cursor Renderer

- [ ] Create `web/src/cursor-renderer.ts`
- [ ] Export a `renderCursor` function matching the `CollaborationCursor` render API:
  ```typescript
  export function renderCursor(user: { name: string; color: string }) {
    const cursor = document.createElement('span');
    cursor.classList.add('collaboration-cursor');
    cursor.style.borderColor = user.color;

    const label = document.createElement('span');
    label.classList.add('collaboration-cursor-label');
    label.style.backgroundColor = user.color;
    label.textContent = user.name;

    cursor.appendChild(label);
    return cursor;
  }
  ```
- [ ] Pass this to the extension configuration: `render: renderCursor`

### Step 5: Add Cursor Styles

- [ ] Add to `web/src/editor.css`:
  - [ ] `.collaboration-cursor`: `border-left: 2px solid` (color set inline), `position: relative`
  - [ ] `.collaboration-cursor-label`: `position: absolute`, `top: -1.4em`, `left: -1px`, `font-size: 0.75rem`, `color: white`, `padding: 1px 6px`, `border-radius: 3px`, `white-space: nowrap`, `pointer-events: none`, `opacity: 0` by default, `opacity: 1` on parent hover (or always visible — decide based on testing)
  - [ ] `.yRemoteSelection`: background-color is set inline by the plugin
  - [ ] `.yRemoteSelectionHead`: the cursor caret marker

### Step 6: Create Presence User List Component

- [ ] Create `web/src/Presence.tsx`
- [ ] Accept `provider: WebsocketProvider` as a prop
- [ ] Subscribe to `provider.awareness.on('change', ...)`
- [ ] Read all awareness states: `provider.awareness.getStates()`
- [ ] Filter to states that have a `user` field (skip entries without it)
- [ ] Render a compact list:
  ```tsx
  export function Presence({ provider }: { provider: WebsocketProvider }) {
    const [users, setUsers] = useState<{ name: string; color: string }[]>([]);

    useEffect(() => {
      const update = () => {
        const states = Array.from(provider.awareness.getStates().values());
        setUsers(
          states
            .filter((s) => s.user)
            .map((s) => s.user as { name: string; color: string })
        );
      };
      provider.awareness.on('change', update);
      update(); // Initial read
      return () => provider.awareness.off('change', update);
    }, [provider]);

    return (
      <div className="presence" aria-label="Connected users">
        {users.map((user, i) => (
          <div key={i} className="presence-user" title={user.name}>
            <span
              className="presence-dot"
              style={{ backgroundColor: user.color }}
            />
            <span className="presence-name">{user.name}</span>
          </div>
        ))}
      </div>
    );
  }
  ```

### Step 7: Integrate Presence into the App

- [ ] Modify `web/src/App.tsx`:
  - Render `<Presence provider={provider} />` in the header, next to the connection status indicator

### Step 8: Add Presence Styles

- [ ] Add to `web/src/editor.css`:
  - [ ] `.presence`: flex row, gap, align items center
  - [ ] `.presence-user`: flex row, gap, align items center
  - [ ] `.presence-dot`: 10px circle, `border-radius: 50%`
  - [ ] `.presence-name`: small font, truncate with ellipsis if needed

## Verification

- [ ] Open two browser tabs. Each shows a different random name/color
- [ ] Tab A's cursor is visible in Tab B (colored line + name label) and vice versa
- [ ] Selecting text in Tab A shows a colored highlight in Tab B
- [ ] The presence list in the header shows both users with correct colors
- [ ] Closing Tab A causes its cursor to disappear from Tab B (after awareness timeout, ~30s)
- [ ] Reloading a tab preserves the same user name and color (from localStorage)
- [ ] Three or more tabs all see each other's cursors simultaneously

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
