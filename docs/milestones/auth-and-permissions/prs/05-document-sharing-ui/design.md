# PR 5: Document Sharing UI

## Purpose

Add the frontend components for managing document permissions: a sharing dialog to invite users by email, assign/change roles, and remove access. Also make the editor permission-aware — read-only users see the document but cannot edit. This is the final PR of Milestone 3, tying together the backend permission system (PR 3) with the frontend auth UI (PR 4).

## Sharing Dialog

### Access Point

- A "Share" button appears on each document card in the Dashboard.
- A "Share" button appears in the editor toolbar/header when viewing a document.
- Only the document owner sees the Share button. Other users with Edit access can view the document but cannot manage sharing.

### Dialog Content

The sharing dialog is a modal with two sections:

**1. Current collaborators list:**

```
┌─────────────────────────────────────────────────┐
│  Share "Design Spec"                        [×] │
│─────────────────────────────────────────────────│
│                                                 │
│  Alice (you)          Owner                     │
│  bob@example.com      [Comment ▾]    [Remove]   │
│  carol@example.com    [Read ▾]       [Remove]   │
│                                                 │
│─────────────────────────────────────────────────│
│  Invite someone                                 │
│  [email@example.com    ] [Comment ▾] [Invite]   │
│                                                 │
└─────────────────────────────────────────────────┘
```

- Each row shows the user's display name (or email if no display name), their current role, and a remove button.
- The owner row is not editable — shows "Owner" badge, no role dropdown or remove button.
- Role dropdowns allow changing between Read, Comment, and Edit.
- Role changes take effect immediately (PATCH request on change).
- Remove button triggers a confirmation, then DELETE request.

**2. Invite section:**

- Email input + role selector (dropdown defaulting to "Comment") + "Invite" button.
- On submit, POST to `/api/docs/{id}/permissions`.
- If the email doesn't match a registered user, show error: "No user found with this email. They need to register first."
- On success, the user appears in the collaborators list.

### Data Flow

```
ShareDialog
  ├── useEffect → GET /api/docs/{id}/permissions → populate collaborator list
  ├── onInvite  → POST /api/docs/{id}/permissions → refresh list
  ├── onChangeRole → PATCH /api/docs/{id}/permissions/{userId} → refresh list
  └── onRemove  → DELETE /api/docs/{id}/permissions/{userId} → refresh list
```

## Permission-Aware Editor

### Read-Only Mode

When a user has Read permission:

- The Tiptap editor is set to `editable: false`.
- The toolbar is hidden or all buttons are disabled.
- A banner or indicator shows "Read only" at the top of the editor.
- The user's cursor still appears to other users (via Awareness), but they cannot type.

### Comment Mode

When a user has Comment permission:

- Same as Read-only for document editing (toolbar disabled, `editable: false`).
- Comment-related UI will be enabled in M5 (Comments milestone).
- For now, behaves identically to Read from the editor perspective.

### Edit Mode

Full editing access — no changes from current behavior.

### Implementation

The `EditorPage` component fetches the user's permission level for the current document:

```typescript
// Option A: Derive from the permissions endpoint
const permission = await fetchUserPermission(docId);

// Option B: Include permission in the document metadata response
// (requires a small backend change to GET /api/docs/{id})
const doc = await getDocument(docId); // response includes { ..., "role": "edit" }
```

**We use Option B** — it's more efficient (one request instead of two) and the information is naturally available when loading a document. The `GET /api/docs/{id}` response is extended to include the requesting user's role:

```json
{
  "id": "...",
  "title": "...",
  "created_at": "...",
  "updated_at": "...",
  "role": "edit",
  "is_owner": true
}
```

This requires a small backend change: the `get_document` handler joins against `document_permissions` to include the role.

### Toolbar Visibility

```tsx
const isEditable = permission === 'edit';

<Editor editable={isEditable} /* ... */ />;
{
  isEditable && <Toolbar editor={editor} />;
}
{
  !isEditable && <ReadOnlyBanner />;
}
```

## Dashboard Changes

### Share Button

Each document card in the dashboard gains a "Share" button, visible only to the owner:

```tsx
{
  doc.is_owner && <button onClick={() => openShareDialog(doc.id)}>Share</button>;
}
```

This requires the document listing to include `is_owner` information. The `GET /api/docs` response is extended:

```json
[
  {
    "id": "...",
    "title": "...",
    "created_at": "...",
    "updated_at": "...",
    "role": "edit",
    "is_owner": true
  }
]
```

### Role Badge

Each document card shows the user's role as a small badge (e.g., "Editor", "Viewer", "Commenter").

## Backend Changes

### Extend Document Response

The `DocumentSummary` response type gains `role` and `is_owner` fields. This requires updating the `list_documents` and `get_document` handlers to join against `document_permissions`:

```rust
#[derive(Serialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub role: String,
    pub is_owner: bool,
}
```

The `list_accessible_documents` query is updated:

```sql
SELECT d.id, d.title, d.created_at, d.updated_at,
       dp.role,
       (d.created_by = $1) AS is_owner
FROM documents d
INNER JOIN document_permissions dp ON dp.document_id = d.id
WHERE dp.user_id = $1
ORDER BY d.updated_at DESC
```

## Error States

| Scenario                       | UX Behavior                                           |
| ------------------------------ | ----------------------------------------------------- |
| Invite email not found         | Error text: "No user found with this email"           |
| User already has access        | Error text: "User already has access"                 |
| Network error on role change   | Toast/inline error, revert dropdown to previous value |
| Network error on remove        | Toast/inline error, keep user in list                 |
| Opening doc with no permission | 403 → redirect to dashboard with error message        |

## What's Not Included

- **Comment UI** — M5. Comment-role users see the editor in read-only mode; comment creation/viewing comes later.
- **Email invitations** — the invited user must already have a Cadmus account. No email notification system.
- **Bulk sharing** — one invite at a time.
- **Public/link sharing** — documents are private by default, shared only to specific users.
