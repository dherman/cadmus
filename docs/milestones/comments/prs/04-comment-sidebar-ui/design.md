# PR 4: Comment Sidebar UI

## Purpose

Build the frontend comment experience: a sidebar that displays comments, a flow for creating comments from text selections, anchor range highlighting in the editor, and real-time updates via WebSocket events. After this PR, users can see, create, and navigate comments — the core comment UX is functional.

## UI Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ ← Documents   Cadmus        ● [User A] [Share] [Export] [Comments]  │
├──────────────────────────────────────┬───────────────────────────────┤
│                                      │  Comments                  ✕ │
│  B I S ~ ⌘ H1 H2 • 1. "" <> ─      │                              │
│                                      │  ┌─ Alice · 2m ago ───────┐ │
│  This is the document content.       │  │ This needs more detail  │ │
│  Here is a ████████████████ that     │  │ on the error handling.  │ │
│  has a comment anchored to it.       │  │                         │ │
│                                      │  │ [Reply]    [Resolve]    │ │
│  More content below...               │  └─────────────────────────┘ │
│                                      │                              │
│                                      │  ┌─ Bob · 5m ago ─────────┐ │
│                                      │  │ Should we add a diagram │ │
│                                      │  │ here?                   │ │
│                                      │  │                         │ │
│                                      │  │ [Reply]    [Resolve]    │ │
│                                      │  └─────────────────────────┘ │
│                                      │                              │
│                                      │  ──── Resolved (1) ──────── │
│                                      │                              │
├──────────────────────────────────────┴───────────────────────────────┤
```

### Key interactions:

1. **Toggle sidebar**: A "Comments" button in the editor header opens/closes the sidebar. The sidebar slides in from the right, shrinking the editor width.
2. **Create comment**: User selects text → a floating "Comment" button appears near the selection → clicking it opens a text input in the sidebar anchored to the selection → submit creates the comment via REST.
3. **Anchor highlighting**: Text ranges with comments are highlighted with a semi-transparent background color. Clicking a highlight scrolls the sidebar to the corresponding comment. Clicking a comment in the sidebar scrolls the editor to the anchored text.
4. **Real-time updates**: New comments from other users appear in the sidebar immediately via WebSocket events.

## Components

### `CommentSidebar.tsx`

The main sidebar component. Renders:

- Header with close button and comment count
- List of top-level comments (open by default), each showing:
  - Author name and relative timestamp
  - Comment body
  - Reply button and Resolve button (for Comment/Edit users)
  - Anchor badge showing the highlighted text snippet (truncated)
- "Resolved" section (collapsed by default, expandable)
- Comment creation form (appears when user initiates a comment from a text selection)

Props:

```typescript
interface CommentSidebarProps {
  docId: string;
  comments: Comment[];
  onCreateComment: (body: string, anchorFrom: number, anchorTo: number) => Promise<void>;
  onReply: (commentId: string, body: string) => Promise<void>;
  onResolve: (commentId: string) => Promise<void>;
  onUnresolve: (commentId: string) => Promise<void>;
  onEditComment: (commentId: string, body: string) => Promise<void>;
  activeCommentId: string | null;
  onCommentClick: (commentId: string) => void;
  pendingAnchor: { from: number; to: number } | null;
  onCancelCreate: () => void;
  currentUserId: string;
  userRole: string;
}
```

### `useComments.ts`

React hook that manages comment state and WebSocket event handling:

```typescript
function useComments(docId: string, wsProvider: WebsocketProvider | null) {
  const [comments, setComments] = useState<Comment[]>([]);
  const [loading, setLoading] = useState(true);

  // Fetch initial comments on mount
  useEffect(() => {
    listComments(docId)
      .then(setComments)
      .finally(() => setLoading(false));
  }, [docId]);

  // Listen for WebSocket custom messages
  useEffect(() => {
    if (!wsProvider) return;
    const handler = (tag: number, data: Uint8Array) => {
      if (tag !== COMMENT_EVENT_TAG) return;
      const event = JSON.parse(new TextDecoder().decode(data));
      // Update local state based on event type
    };
    wsProvider.on('custom-message', handler);
    return () => wsProvider.off('custom-message', handler);
  }, [wsProvider]);

  return { comments, loading /* mutation functions */ };
}
```

The hook handles:

- Initial fetch via REST
- Real-time updates via WebSocket custom messages
- Optimistic updates on local mutations (update local state before REST confirms)
- Error rollback if the REST call fails

### Anchor Highlighting

Comment anchors are rendered as highlights in the editor using Tiptap decorations (ProseMirror `DecorationSet`). A Tiptap extension or plugin:

1. Receives the list of comments with resolved anchor positions.
2. Creates a `Decoration.inline(from, to, { class: 'comment-highlight', 'data-comment-id': id })` for each anchored comment.
3. Updates decorations when comments change (new comment, resolved comment removed from highlights).

Clicking a highlight dispatches a custom event that the sidebar listens for to scroll to the comment.

### Selection → Comment Flow

1. User selects text in the editor.
2. A floating "Add Comment" button appears near the selection (using a Tiptap `BubbleMenu` or a custom positioned element).
3. User clicks "Add Comment".
4. The sidebar opens (if closed) with a comment creation form pre-filled with the anchor range.
5. User types the comment body and submits.
6. `createComment(docId, body, anchorFrom, anchorTo)` is called.
7. On success, the comment appears in the sidebar and the anchor is highlighted.

The "Add Comment" button only appears for users with Comment or Edit permission.

## Styles

Add to `editor.css`:

```css
/* Sidebar */
.comment-sidebar {
  /* Right panel, resizable or fixed width */
}
.comment-card {
  /* Individual comment card */
}
.comment-author {
  /* Author name + timestamp */
}
.comment-body {
  /* Comment text */
}
.comment-actions {
  /* Reply, Resolve buttons */
}

/* Anchor highlights */
.comment-highlight {
  background: rgba(255, 212, 0, 0.25);
  cursor: pointer;
}
.comment-highlight.active {
  background: rgba(255, 212, 0, 0.45);
}

/* Creation form */
.comment-create-form {
  /* Textarea + submit button */
}

/* Resolved section */
.comment-resolved-section {
  /* Collapsible section header */
}
```

## Permission Behavior

| Role    | Can see sidebar | Can create | Can reply | Can resolve | Can edit own |
| ------- | --------------- | ---------- | --------- | ----------- | ------------ |
| Read    | Yes             | No         | No        | No          | No           |
| Comment | Yes             | Yes        | Yes       | Yes         | Yes          |
| Edit    | Yes             | Yes        | Yes       | Yes         | Yes          |

Read-role users see the sidebar and highlights but the "Add Comment" button, reply inputs, and resolve buttons are hidden.

## What's Not Included

- Threaded reply display (PR 5 — this PR shows top-level comments only, replies are listed flat)
- Resolve/unresolve UI wiring (PR 5 — the buttons are rendered but the full thread collapse UX comes in PR 5)
- Edit comment UI (PR 5 — the edit flow with a modal or inline form)
- Comment count badge on the sidebar toggle button (nice-to-have, can add in PR 5)
