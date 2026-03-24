# PR 5: Comment Threads & Resolution

## Purpose

Complete the comment UX with threaded reply display, resolve/unresolve interactions, comment editing, and the final polish needed for Google Docs-level comment parity. After this PR, comment threads feel like real conversations — replies are nested under their parent, resolved threads collapse, and users can edit their own comments.

## Threaded Reply Display

### Thread grouping

The sidebar groups comments into threads by `parent_id`. Each thread is a top-level comment (where `parent_id === null`) followed by its replies (where `parent_id === thread root id`), sorted by `created_at`.

```
┌─ Alice · 2m ago ───────────────────────┐
│ This section needs more detail on the  │
│ error handling approach.               │
│                                        │
│   ┌─ Bob · 1m ago ──────────────────┐  │
│   │ Agreed — I'll add a retry       │  │
│   │ section with backoff.           │  │
│   └──────────────────────────────────┘  │
│                                        │
│   ┌─ Alice · 30s ago ───────────────┐  │
│   │ Perfect, thanks.                │  │
│   └──────────────────────────────────┘  │
│                                        │
│  [Reply]                    [Resolve]  │
└────────────────────────────────────────┘
```

### `CommentThread.tsx`

A new component that renders a single thread: the root comment + its replies.

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

- Root comment renders with full anchor context (highlighted text snippet).
- Replies are indented and visually connected.
- The Reply button at the thread bottom opens an inline reply form.
- The Resolve button appears on the thread footer (for Comment/Edit users).

## Resolve/Unresolve UX

### Resolving a thread

1. User clicks "Resolve" on a thread.
2. Optimistic update: thread moves to the "Resolved" section with a fade animation.
3. REST call: `POST /api/docs/{id}/comments/{cid}/resolve`.
4. Anchor highlight is removed from the editor (resolved threads don't highlight).
5. If the REST call fails, the thread moves back to the open section with an error toast.

### Unresolving a thread

1. User expands the "Resolved" section and clicks "Reopen" on a thread.
2. Thread moves back to the open section.
3. REST call: `POST /api/docs/{id}/comments/{cid}/unresolve`.
4. Anchor highlight reappears in the editor.

### Resolved section

The "Resolved" section at the bottom of the sidebar:

```
──── Resolved (3) ▸ ────────────────────
```

- Collapsed by default, showing only the count.
- Click to expand and show resolved threads.
- Resolved threads are rendered identically to open threads but with muted styling and a "Reopen" button instead of "Resolve".

## Comment Editing

### Edit flow

1. User clicks a "..." menu (or "Edit" button) on their own comment.
2. The comment body becomes an editable textarea pre-filled with the current text.
3. User modifies the text and clicks "Save" (or presses Cmd+Enter).
4. REST call: `PUT /api/docs/{id}/comments/{cid}`.
5. The comment body updates in place.
6. Cancel returns to the read-only view.

### Edit restrictions

- Only the comment's author sees the edit control.
- Only the body can be edited — anchors, status, and parent are immutable.
- An "(edited)" indicator appears next to the timestamp if `updated_at > created_at + 1 second`.

## Comment Count Badge

The "Comments" toggle button in the editor header shows the count of open threads:

```
[Comments (3)]
```

This updates in real time as comments are created/resolved.

## Updated CommentSidebar

The sidebar from PR 4 is refactored to use `CommentThread` for rendering:

```typescript
function CommentSidebar({ comments, ...props }) {
  const threads = groupIntoThreads(comments);
  const openThreads = threads.filter((t) => t.root.status === 'open');
  const resolvedThreads = threads.filter((t) => t.root.status === 'resolved');

  return (
    <div className="comment-sidebar">
      <div className="comment-sidebar-header">
        <h3>Comments</h3>
        <button onClick={props.onClose}>✕</button>
      </div>

      {/* Creation form (when pending anchor exists) */}
      {props.pendingAnchor && <CommentCreateForm ... />}

      {/* Open threads */}
      {openThreads.map((thread) => (
        <CommentThread key={thread.root.id} root={thread.root} replies={thread.replies} ... />
      ))}

      {/* Resolved section */}
      {resolvedThreads.length > 0 && (
        <ResolvedSection threads={resolvedThreads} ... />
      )}
    </div>
  );
}

function groupIntoThreads(comments: Comment[]): Thread[] {
  const roots = comments.filter((c) => c.parent_id === null);
  return roots.map((root) => ({
    root,
    replies: comments
      .filter((c) => c.parent_id === root.id)
      .sort((a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime()),
  }));
}
```

## Keyboard Shortcuts

- **Cmd+Shift+M** (Mac) / **Ctrl+Shift+M** (Windows): Toggle comment sidebar.
- **Cmd+Enter** in a comment form: Submit the comment.
- **Escape** in a comment form: Cancel creation/editing.

## What's Not Included

- Comment deletion (resolve is sufficient — see milestone README)
- Comment notifications via email/push (WebSocket only)
- Comment mentions (@user)
- Drag-to-reorder threads
- Comment search/filter by author
