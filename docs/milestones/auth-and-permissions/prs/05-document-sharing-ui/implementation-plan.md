# PR 5: Document Sharing UI — Implementation Plan

## Prerequisites

- [ ] PR 3 (Permission Enforcement) merged — sharing endpoints available
- [ ] PR 4 (Frontend Auth UI) merged — auth context, login/register working

## Steps

### Step 1: Extend backend document responses with role info

- [ ] Update `list_accessible_documents` query in `packages/server/src/db.rs` to include role and ownership:

```rust
#[derive(Debug, sqlx::FromRow)]
pub struct DocumentWithRole {
    pub id: Uuid,
    pub title: String,
    pub schema_version: i32,
    pub snapshot_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub role: String,
    pub is_owner: bool,
}

pub async fn list_accessible_documents_with_role(
    &self,
    user_id: Uuid,
) -> Result<Vec<DocumentWithRole>, sqlx::Error> {
    sqlx::query_as::<_, DocumentWithRole>(
        r#"SELECT d.id, d.title, d.schema_version, d.snapshot_key,
                  d.created_at, d.updated_at,
                  dp.role,
                  (d.created_by = $1) AS is_owner
           FROM documents d
           INNER JOIN document_permissions dp ON dp.document_id = d.id
           WHERE dp.user_id = $1
           ORDER BY d.updated_at DESC"#
    )
    .bind(user_id)
    .fetch_all(&self.pool)
    .await
}
```

- [ ] Add a similar `get_document_with_role` method for single-document fetch

- [ ] Update `DocumentSummary` in `api.rs` to include `role` and `is_owner`:

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

- [ ] Update `list_documents` and `get_document` handlers to use the new queries

### Step 2: Update frontend DocumentSummary type

- [ ] In `packages/web/src/api.ts`, extend the type:

```typescript
export interface DocumentSummary {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  role: 'read' | 'comment' | 'edit';
  is_owner: boolean;
}
```

### Step 3: Add sharing API functions

- [ ] Add to `packages/web/src/api.ts`:

```typescript
export interface PermissionEntry {
  user_id: string;
  email: string;
  display_name: string;
  role: string;
  is_owner: boolean;
}

export async function listPermissions(docId: string): Promise<PermissionEntry[]> {
  const res = await authFetch(`${API_BASE}/api/docs/${encodeURIComponent(docId)}/permissions`);
  if (!res.ok) throw new Error('Failed to fetch permissions');
  return res.json();
}

export async function addPermission(docId: string, email: string, role: string): Promise<void> {
  const res = await authFetch(`${API_BASE}/api/docs/${encodeURIComponent(docId)}/permissions`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, role }),
  });
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Failed to add permission');
  }
}

export async function updatePermission(docId: string, userId: string, role: string): Promise<void> {
  /* PATCH /api/docs/{docId}/permissions/{userId} */
}

export async function removePermission(docId: string, userId: string): Promise<void> {
  /* DELETE /api/docs/{docId}/permissions/{userId} */
}
```

### Step 4: Create ShareDialog component

- [ ] Create `packages/web/src/ShareDialog.tsx`:

```tsx
interface ShareDialogProps {
  docId: string;
  docTitle: string;
  onClose: () => void;
}

export function ShareDialog({ docId, docTitle, onClose }: ShareDialogProps) {
  const [permissions, setPermissions] = useState<PermissionEntry[]>([]);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState('comment');
  const [error, setError] = useState<string | null>(null);

  // Load permissions on mount
  useEffect(() => {
    listPermissions(docId).then(setPermissions).catch(console.error);
  }, [docId]);

  // Invite handler
  async function handleInvite() {
    /* ... */
  }

  // Role change handler
  async function handleRoleChange(userId: string, newRole: string) {
    /* ... */
  }

  // Remove handler
  async function handleRemove(userId: string) {
    /* ... */
  }

  return (
    <div className="share-dialog-overlay" onClick={onClose}>
      <div className="share-dialog" onClick={(e) => e.stopPropagation()}>
        <h2>Share "{docTitle}"</h2>
        {/* Collaborator list */}
        {/* Invite form */}
      </div>
    </div>
  );
}
```

- [ ] Style with CSS: modal overlay, centered card, form layout consistent with existing app style

### Step 5: Add ShareDialog to Dashboard

- [ ] In `packages/web/src/Dashboard.tsx`:
  - Add state for the active share dialog: `const [sharingDocId, setSharingDocId] = useState<string | null>(null)`
  - Show "Share" button on each document card where `doc.is_owner === true`
  - Render `ShareDialog` when `sharingDocId` is set

```tsx
{
  doc.is_owner && <button onClick={() => setSharingDocId(doc.id)}>Share</button>;
}

{
  sharingDocId && (
    <ShareDialog
      docId={sharingDocId}
      docTitle={docs.find((d) => d.id === sharingDocId)?.title || ''}
      onClose={() => setSharingDocId(null)}
    />
  );
}
```

### Step 6: Add role badge to document cards

- [ ] Show the user's role on each document card in the dashboard:

```tsx
function roleBadge(role: string) {
  const labels: Record<string, string> = {
    edit: 'Editor',
    comment: 'Commenter',
    read: 'Viewer',
  };
  return <span className={`role-badge role-${role}`}>{labels[role]}</span>;
}
```

### Step 7: Add Share button to editor header

- [ ] In `packages/web/src/EditorPage.tsx`, add a "Share" button in the header when the user is the owner:

```tsx
const [showShareDialog, setShowShareDialog] = useState(false);

{
  doc?.is_owner && <button onClick={() => setShowShareDialog(true)}>Share</button>;
}
```

### Step 8: Make editor permission-aware

- [ ] In `packages/web/src/EditorPage.tsx`, use the document's `role` to control editability:

```tsx
const isEditable = doc?.role === 'edit';

<Editor editable={isEditable} extensions={extensions} content={null} />;
```

- [ ] Conditionally render the toolbar:

```tsx
{
  isEditable ? (
    <Toolbar editor={editor} />
  ) : (
    <div className="read-only-banner">Read only — you can view this document but not edit it</div>
  );
}
```

- [ ] Pass `editable` prop to the Tiptap `useEditor` hook:

```typescript
const editor = useEditor({
  extensions,
  editable: isEditable,
  // ...
});
```

### Step 9: Add styles

- [ ] Add CSS for:
  - `.share-dialog-overlay` — fixed fullscreen backdrop with semi-transparent background
  - `.share-dialog` — centered white card with padding
  - `.role-badge` — small colored pill (green for edit, blue for comment, gray for read)
  - `.read-only-banner` — subtle top banner in the editor
  - Form inputs and buttons consistent with login/register styling

### Step 10: Handle error states

- [ ] In `ShareDialog`:
  - Catch errors from invite/role change/remove API calls
  - Display inline error messages
  - On role change error, revert the dropdown to the previous value

- [ ] In `EditorPage`:
  - Handle 403 response when loading a document → redirect to dashboard with error message
  - Handle the case where permission is revoked while viewing (WS disconnects, API calls fail)

## Verification

- [ ] Dashboard shows role badges on document cards
- [ ] "Share" button only appears for document owners
- [ ] Share dialog lists current collaborators with correct roles
- [ ] Inviting a user by email works (they appear in the list)
- [ ] Inviting a non-existent email shows "User not found" error
- [ ] Changing a user's role updates immediately
- [ ] Removing a user's access removes them from the list
- [ ] Owner cannot remove their own access
- [ ] Read-only users see the document without toolbar
- [ ] Read-only users see a "Read only" banner
- [ ] Read-only users cannot type in the editor
- [ ] Edit users have full toolbar and editing capability
- [ ] Navigating to a document without permission shows 403 → redirects to dashboard
- [ ] `pnpm run format:check` passes
- [ ] `pnpm run build` succeeds

## Files Created/Modified

- `packages/server/src/db.rs` (modified — add DocumentWithRole, role-aware queries)
- `packages/server/src/documents/api.rs` (modified — extend DocumentSummary with role/is_owner)
- `packages/web/src/api.ts` (modified — extend DocumentSummary type, add sharing API functions)
- `packages/web/src/ShareDialog.tsx` (new)
- `packages/web/src/Dashboard.tsx` (modified — share button, role badges)
- `packages/web/src/EditorPage.tsx` (modified — permission-aware editor, share button)
- `packages/web/src/Editor.tsx` (modified — accept editable prop)
- `packages/web/src/editor.css` (modified — sharing dialog styles, role badges, read-only banner)
