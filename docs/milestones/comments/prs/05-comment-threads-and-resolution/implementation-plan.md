# PR 5: Comment Threads & Resolution — Implementation Plan

## Prerequisites

- [x] PR 4 (Comment Sidebar UI) is merged — sidebar and basic comment display exist

## Steps

### 1. Create the `CommentThread` component

- [x] Create `packages/web/src/CommentThread.tsx`:
  - Render the root comment with author, relative timestamp, and body
  - Render replies indented below the root, each with author and timestamp
  - Show "(edited)" indicator when `updated_at` is more than 1 second after `created_at`
  - Add a reply form at the bottom of the thread (initially hidden, shown on "Reply" click)
  - Reply form: textarea + Submit/Cancel buttons, Cmd+Enter to submit, Escape to cancel

```typescript
interface CommentThreadProps {
  root: Comment;
  replies: Comment[];
  onReply: (body: string) => Promise<void>;
  onResolve: () => Promise<void>;
  onUnresolve: () => Promise<void>;
  onEdit: (commentId: string, body: string) => Promise<void>;
  isActive: boolean;
  onThreadClick: () => void;
  currentUserId: string;
  canComment: boolean;
}
```

### 2. Add thread grouping logic

- [x] Add a `groupIntoThreads` utility function (in `CommentSidebar.tsx` or a separate utility):

```typescript
interface CommentThread {
  root: Comment;
  replies: Comment[];
}

function groupIntoThreads(comments: Comment[]): CommentThread[] {
  const roots = comments.filter((c) => c.parent_id === null);
  const replyMap = new Map<string, Comment[]>();

  for (const c of comments) {
    if (c.parent_id) {
      const existing = replyMap.get(c.parent_id) || [];
      existing.push(c);
      replyMap.set(c.parent_id, existing);
    }
  }

  return roots.map((root) => ({
    root,
    replies: (replyMap.get(root.id) || []).sort(
      (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
    ),
  }));
}
```

### 3. Refactor `CommentSidebar` to use threads

- [x] Update `packages/web/src/CommentSidebar.tsx`:
  - Replace flat comment list with `CommentThread` components
  - Split into open threads and resolved threads sections
  - Add a collapsible "Resolved (N)" section at the bottom

```typescript
const threads = groupIntoThreads(comments);
const openThreads = threads
  .filter((t) => t.root.status === 'open')
  .sort((a, b) => new Date(b.root.created_at).getTime() - new Date(a.root.created_at).getTime());
const resolvedThreads = threads.filter((t) => t.root.status === 'resolved');
```

### 4. Add resolve/unresolve UI

- [x] In `CommentThread.tsx`, add action buttons to the thread footer:
  - "Resolve" button for open threads (calls `onResolve`)
  - "Reopen" button for resolved threads (calls `onUnresolve`)
  - Only visible for users with Comment or Edit role

- [ ] ~~Add optimistic state updates~~ — deferred to [#34](https://github.com/dherman/cadmus/issues/34)

- [x] Update the highlight plugin in `Editor.tsx` to exclude resolved comments from decorations.

### 5. Add comment editing UI

- [x] In `CommentThread.tsx`, add an edit mode for individual comments:
  - A "..." menu or "Edit" text button on comments authored by the current user
  - Click toggles the comment body into an editable textarea
  - "Save" and "Cancel" buttons below the textarea
  - Cmd+Enter submits, Escape cancels
  - On save: call `onEdit(commentId, newBody)`, update local state

- [x] Add the "(edited)" indicator:

```typescript
const isEdited =
  new Date(comment.updated_at).getTime() - new Date(comment.created_at).getTime() > 1000;
```

### 6. Add comment count badge

- [x] In `EditorPage.tsx`, compute the open thread count:

```typescript
const openCount = comments.filter((c) => c.parent_id === null && c.status === 'open').length;
```

- [x] Render the count in the Comments toggle button:

```tsx
<button onClick={() => setSidebarOpen(!sidebarOpen)}>
  Comments{openCount > 0 ? ` (${openCount})` : ''}
</button>
```

### 7. Add keyboard shortcuts

- [x] In `EditorPage.tsx`, add a keyboard event listener:

```typescript
useEffect(() => {
  function handleKeyDown(e: KeyboardEvent) {
    // Cmd+Shift+M or Ctrl+Shift+M toggles sidebar
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'm') {
      e.preventDefault();
      setSidebarOpen((prev) => !prev);
    }
  }
  document.addEventListener('keydown', handleKeyDown);
  return () => document.removeEventListener('keydown', handleKeyDown);
}, []);
```

- [x] In comment forms (create, reply, edit), handle Cmd+Enter to submit and Escape to cancel.

### 8. Add thread and resolution styles

- [x] Add to `packages/web/src/editor.css`:

```css
/* Thread layout */
.comment-thread {
  border: 1px solid var(--color-border, #e0e0e0);
  border-radius: 6px;
  margin-bottom: 0.75rem;
  overflow: hidden;
}

.comment-thread.active {
  border-color: var(--color-primary, #4a9eff);
}

.comment-thread-root {
  padding: 0.75rem;
}

.comment-thread-replies {
  border-top: 1px solid var(--color-border, #e0e0e0);
  padding-left: 1rem;
  background: var(--color-bg-subtle, #fafafa);
}

.comment-reply {
  padding: 0.5rem 0.75rem;
  border-top: 1px solid var(--color-border-light, #f0f0f0);
}

.comment-reply:first-child {
  border-top: none;
}

/* Edited indicator */
.comment-edited {
  font-size: 0.7rem;
  color: var(--color-muted, #888);
  font-style: italic;
  margin-left: 0.25rem;
}

/* Thread actions */
.comment-thread-actions {
  padding: 0.5rem 0.75rem;
  border-top: 1px solid var(--color-border, #e0e0e0);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

/* Reply form */
.comment-reply-form {
  padding: 0.75rem;
  border-top: 1px solid var(--color-border, #e0e0e0);
  background: var(--color-bg-subtle, #fafafa);
}

/* Resolved section */
.comment-resolved-section {
  margin-top: 1rem;
  border-top: 1px solid var(--color-border, #e0e0e0);
  padding-top: 0.5rem;
}

.comment-resolved-toggle {
  font-size: 0.85rem;
  color: var(--color-muted, #888);
  cursor: pointer;
  background: none;
  border: none;
  padding: 0.5rem 0;
  width: 100%;
  text-align: left;
}

.comment-resolved-toggle:hover {
  color: var(--color-text, #333);
}

.comment-thread.resolved {
  opacity: 0.7;
}

.comment-thread.resolved:hover {
  opacity: 1;
}

/* Edit mode */
.comment-edit-textarea {
  width: 100%;
  min-height: 40px;
  resize: vertical;
  border: 1px solid var(--color-primary, #4a9eff);
  border-radius: 4px;
  padding: 0.5rem;
  font-size: 0.9rem;
  font-family: inherit;
}

.comment-edit-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 0.25rem;
}

/* Comment count badge */
.comment-count {
  font-size: 0.8rem;
  background: var(--color-primary, #4a9eff);
  color: white;
  border-radius: 10px;
  padding: 0 0.4rem;
  margin-left: 0.25rem;
  min-width: 1.2rem;
  text-align: center;
  display: inline-block;
}
```

### 9. Manual verification

- [x] Start the full dev stack: `pnpm dev`
- [x] Create a comment on a text selection. Verify the thread appears in the sidebar.
- [x] Reply to the comment. Verify the reply appears indented under the root.
- [x] Reply again. Verify replies are ordered chronologically.
- [x] Open in a second tab — verify replies appear in real time via WebSocket.
- [x] Resolve the thread. Verify it moves to the "Resolved" section and the highlight disappears.
- [x] Expand "Resolved" and click "Reopen". Verify it moves back to open and the highlight reappears.
- [x] Edit your own comment. Verify the "(edited)" indicator appears.
- [x] Verify you cannot see the edit button on another user's comment.
- [x] Test Cmd+Shift+M toggles the sidebar.
- [x] Test Cmd+Enter submits in comment forms.
- [x] Test with a Read-role user — verify they can see threads but cannot reply, resolve, or edit.
- [x] Verify the comment count badge in the header updates as comments are created/resolved.

### 10. Build and format check

- [x] Run `pnpm -F @cadmus/web build` — TypeScript compiles without errors.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [x] Comments are grouped into threads by parent_id
- [x] Replies render indented under their parent comment
- [x] Reply form opens inline at the bottom of the thread
- [x] Resolve moves thread to the "Resolved" section and removes highlight
- [x] Unresolve moves thread back to open and restores highlight
- [x] Resolved section is collapsible with a count indicator
- [x] Comment editing works for the author only
- [x] "(edited)" indicator appears on edited comments
- [x] Comment count badge updates in real time
- [x] Cmd+Shift+M toggles the sidebar
- [x] Cmd+Enter submits forms, Escape cancels
- [x] All interactions broadcast via WebSocket to other connected clients
- [x] Read-role users see threads but cannot interact

## Files Modified

| File                                  | Change                                                |
| ------------------------------------- | ----------------------------------------------------- |
| `packages/web/src/CommentThread.tsx`  | New: threaded comment display with reply/edit/resolve |
| `packages/web/src/CommentSidebar.tsx` | Refactor to use threads, add resolved section         |
| `packages/web/src/EditorPage.tsx`     | Add comment count badge, keyboard shortcut            |
| `packages/web/src/Editor.tsx`         | Exclude resolved comments from highlight decorations  |
| `packages/web/src/editor.css`         | Thread, reply, edit, resolved section styles          |
