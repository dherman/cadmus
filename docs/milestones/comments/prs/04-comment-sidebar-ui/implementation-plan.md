# PR 4: Comment Sidebar UI — Implementation Plan

## Prerequisites

- [x] PR 1 (Comments Table & CRUD API) is merged — REST endpoints work
- [x] PR 3 (WebSocket Comment Events) is merged — real-time events are broadcast

## Steps

### 1. Create the `useComments` hook

- [x] Create `packages/web/src/useComments.ts`:

```typescript
import { useState, useEffect, useCallback } from 'react';
import {
  listComments,
  createComment,
  replyToComment,
  resolveComment,
  unresolveComment,
  editComment,
} from './api';
import type { Comment } from './api';
import type { WebsocketProvider } from 'y-websocket';

const COMMENT_EVENT_TAG = 100;

export function useComments(docId: string, wsProvider: WebsocketProvider | null) {
  const [comments, setComments] = useState<Comment[]>([]);
  const [loading, setLoading] = useState(true);

  // Initial fetch
  useEffect(() => {
    listComments(docId, 'all')
      .then(setComments)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [docId]);

  // WebSocket event listener
  useEffect(() => {
    if (!wsProvider) return;

    const handler = (tag: number, data: Uint8Array) => {
      if (tag !== COMMENT_EVENT_TAG) return;
      const event = JSON.parse(new TextDecoder().decode(data));
      const comment: Comment = event.comment;

      setComments((prev) => {
        switch (event.type) {
          case 'created':
          case 'replied':
            // Add if not already present (dedup with optimistic update)
            if (prev.some((c) => c.id === comment.id)) return prev;
            return [...prev, comment];
          case 'updated':
            return prev.map((c) => (c.id === comment.id ? comment : c));
          case 'resolved':
          case 'unresolved':
            return prev.map((c) => (c.id === comment.id ? comment : c));
          default:
            return prev;
        }
      });
    };

    wsProvider.on('custom-message', handler);
    return () => {
      wsProvider.off('custom-message', handler);
    };
  }, [wsProvider]);

  // Mutation wrappers with optimistic updates
  const handleCreate = useCallback(
    async (body: string, anchorFrom: number, anchorTo: number) => {
      const comment = await createComment(docId, body, anchorFrom, anchorTo);
      setComments((prev) => (prev.some((c) => c.id === comment.id) ? prev : [...prev, comment]));
    },
    [docId],
  );

  const handleReply = useCallback(
    async (commentId: string, body: string) => {
      const reply = await replyToComment(docId, commentId, body);
      setComments((prev) => (prev.some((c) => c.id === reply.id) ? prev : [...prev, reply]));
    },
    [docId],
  );

  const handleResolve = useCallback(
    async (commentId: string) => {
      const updated = await resolveComment(docId, commentId);
      setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
    },
    [docId],
  );

  const handleUnresolve = useCallback(
    async (commentId: string) => {
      const updated = await unresolveComment(docId, commentId);
      setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
    },
    [docId],
  );

  const handleEdit = useCallback(
    async (commentId: string, body: string) => {
      const updated = await editComment(docId, commentId, body);
      setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
    },
    [docId],
  );

  return {
    comments,
    loading,
    createComment: handleCreate,
    replyToComment: handleReply,
    resolveComment: handleResolve,
    unresolveComment: handleUnresolve,
    editComment: handleEdit,
  };
}
```

- [x] Verify the `y-websocket` provider emits `custom-message` events. If it doesn't natively, we may need to hook into the WebSocket `message` event directly and parse custom messages from the y-sync wire format. Check the `y-websocket` source and adjust the listener accordingly.

### 2. Create the `CommentSidebar` component

- [x] Create `packages/web/src/CommentSidebar.tsx`:
  - Render a list of top-level comments (where `parent_id === null`), sorted by `created_at`
  - Each comment card shows: author avatar/name, relative time (e.g., "2m ago"), body text
  - "Reply" and "Resolve" buttons on each card (visible only for Comment/Edit roles)
  - A "Resolved" collapsible section at the bottom showing resolved comments
  - Comment creation form: textarea + submit button, appears when `pendingAnchor` is set
  - Handle `onCommentClick` to notify the editor to scroll to the anchor

### 3. Add anchor highlighting to the editor

- [x] Create a Tiptap plugin (or ProseMirror plugin) that renders comment highlight decorations:

```typescript
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

export const commentHighlightKey = new PluginKey('commentHighlight');

export function commentHighlightPlugin(comments: Comment[]) {
  return new Plugin({
    key: commentHighlightKey,
    state: {
      init() {
        return buildDecorations(comments);
      },
      apply(tr, decorations) {
        // Rebuild if comments changed or document changed
        return decorations.map(tr.mapping, tr.doc);
      },
    },
    props: {
      decorations(state) {
        return this.getState(state);
      },
    },
  });
}

function buildDecorations(comments: Comment[]): DecorationSet {
  const decorations = comments
    .filter((c) => c.anchor_from != null && c.anchor_to != null && c.status === 'open')
    .map((c) =>
      Decoration.inline(c.anchor_from!, c.anchor_to!, {
        class: 'comment-highlight',
        'data-comment-id': c.id,
      }),
    );
  return DecorationSet.create(/* doc */, decorations);
}
```

- [x] Register the plugin with the Tiptap editor in `Editor.tsx`. The plugin needs to be updated when the comment list changes — use `editor.registerPlugin()` / `editor.unregisterPlugin()` or a reactive Tiptap extension that accepts comment state.

### 4. Add the "Add Comment" selection button

- [x] Add a floating button that appears when the user selects text:
  - Use Tiptap's `BubbleMenu` extension or a custom positioned element
  - Only show for Comment/Edit roles
  - On click: capture the selection's `from`/`to` positions, set `pendingAnchor` state, open the sidebar

```typescript
const [pendingAnchor, setPendingAnchor] = useState<{ from: number; to: number } | null>(null);

function handleAddComment() {
  const { from, to } = editor.state.selection;
  if (from === to) return; // No selection
  setPendingAnchor({ from, to });
  setSidebarOpen(true);
}
```

### 5. Integrate into EditorPage

- [x] Modify `packages/web/src/EditorPage.tsx`:
  - Add sidebar open/close state: `const [sidebarOpen, setSidebarOpen] = useState(false)`
  - Add a "Comments" toggle button in the header
  - Call `useComments(docId, wsProvider)` to get comment state and mutation functions
  - Render `<CommentSidebar>` conditionally when sidebar is open
  - Pass comments to the editor for highlight decorations
  - Handle bidirectional navigation: sidebar click → scroll editor, highlight click → scroll sidebar

### 6. Add comment-related styles

- [x] Add to `packages/web/src/editor.css`:

```css
/* Comment sidebar */
.comment-sidebar {
  width: 320px;
  border-left: 1px solid var(--color-border, #e0e0e0);
  overflow-y: auto;
  padding: 1rem;
  background: var(--color-bg, #fff);
}

.comment-card {
  border: 1px solid var(--color-border, #e0e0e0);
  border-radius: 6px;
  padding: 0.75rem;
  margin-bottom: 0.75rem;
}

.comment-card.active {
  border-color: var(--color-primary, #4a9eff);
  box-shadow: 0 0 0 1px var(--color-primary, #4a9eff);
}

.comment-author {
  font-weight: 600;
  font-size: 0.85rem;
}

.comment-time {
  font-size: 0.75rem;
  color: var(--color-muted, #888);
  margin-left: 0.5rem;
}

.comment-body {
  margin-top: 0.5rem;
  font-size: 0.9rem;
  line-height: 1.4;
}

.comment-actions {
  margin-top: 0.5rem;
  display: flex;
  gap: 0.5rem;
}

/* Anchor highlights */
.comment-highlight {
  background: rgba(255, 212, 0, 0.25);
  border-bottom: 2px solid rgba(255, 212, 0, 0.6);
  cursor: pointer;
}

.comment-highlight.active {
  background: rgba(255, 212, 0, 0.45);
}

/* Floating comment button */
.add-comment-button {
  position: absolute;
  background: var(--color-primary, #4a9eff);
  color: white;
  border: none;
  border-radius: 4px;
  padding: 0.25rem 0.5rem;
  font-size: 0.8rem;
  cursor: pointer;
  z-index: 10;
}

/* Creation form */
.comment-create-form textarea {
  width: 100%;
  min-height: 60px;
  resize: vertical;
  border: 1px solid var(--color-border, #e0e0e0);
  border-radius: 4px;
  padding: 0.5rem;
  font-size: 0.9rem;
}

.comment-create-form .comment-form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 0.5rem;
}

/* Resolved section */
.comment-resolved-toggle {
  font-size: 0.85rem;
  color: var(--color-muted, #888);
  cursor: pointer;
  border: none;
  background: none;
  padding: 0.5rem 0;
}
```

### 7. Manual verification

- [x] Start the full dev stack: `pnpm dev`
- [x] Create a document and add several paragraphs of content.
- [x] Select text and verify the "Add Comment" button appears.
- [x] Create a comment — verify it appears in the sidebar with the correct author and timestamp.
- [x] Verify the anchor text is highlighted in the editor.
- [x] Click the highlight — verify the sidebar scrolls to the comment.
- [x] Click a comment in the sidebar — verify the editor scrolls to the anchor.
- [x] Open the same document in a second tab. Create a comment in tab 1 — verify it appears in tab 2's sidebar in real time.
- [x] Test with a Read-role user — verify they can see comments but the "Add Comment" button and action buttons are hidden.
- [x] Close and reopen the sidebar — verify comments persist (loaded via REST).

### 8. Build and format check

- [x] Run `pnpm -F @cadmus/web build` — TypeScript compiles without errors.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [x] Comment sidebar opens and closes via header button
- [x] Comments load from REST on sidebar open
- [x] Comment creation from text selection works end-to-end
- [x] Anchor highlights appear on commented text ranges
- [x] Clicking a highlight activates the corresponding comment in the sidebar
- [x] Clicking a comment scrolls the editor to the anchor
- [x] WebSocket events update the sidebar in real time (new comments appear without refresh)
- [x] Read-role users see comments but cannot create/reply/resolve
- [x] Comment/Edit users see all action buttons
- [x] Sidebar resizes the editor layout cleanly

## Files Modified

| File                                  | Change                                             |
| ------------------------------------- | -------------------------------------------------- |
| `packages/web/src/useComments.ts`     | New: comment state management + WS event handling  |
| `packages/web/src/CommentSidebar.tsx` | New: comment sidebar component                     |
| `packages/web/src/EditorPage.tsx`     | Integrate sidebar, comments toggle, pending anchor |
| `packages/web/src/Editor.tsx`         | Add comment highlight plugin/decorations           |
| `packages/web/src/editor.css`         | Add sidebar, highlight, and comment card styles    |
