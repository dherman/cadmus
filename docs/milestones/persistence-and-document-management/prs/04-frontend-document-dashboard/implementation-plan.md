# PR 4: Frontend Document Dashboard — Implementation Plan

## Prerequisites

- [ ] PR 3 (Document CRUD API) merged
- [ ] REST endpoints working: `GET /api/docs`, `POST /api/docs`, `GET /api/docs/{id}`

## Steps

### Step 1: Add React Router

- [ ] Install React Router: `pnpm -F @cadmus/web add react-router`
- [ ] Set up the router in `src/main.tsx`:

```tsx
import { createBrowserRouter, RouterProvider } from 'react-router';
import { Dashboard } from './Dashboard';
import { EditorPage } from './EditorPage';

const router = createBrowserRouter([
  { path: '/', element: <Dashboard /> },
  { path: '/docs/:id', element: <EditorPage /> },
]);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
);
```

### Step 2: Create the API client

- [ ] Create `src/api.ts`:

```typescript
const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

export interface DocumentSummary {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export async function listDocuments(): Promise<DocumentSummary[]> {
  const res = await fetch(`${API_BASE}/api/docs`);
  if (!res.ok) throw new Error('Failed to fetch documents');
  return res.json();
}

export async function createDocument(title: string): Promise<DocumentSummary> {
  const res = await fetch(`${API_BASE}/api/docs`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ title }),
  });
  if (!res.ok) throw new Error('Failed to create document');
  return res.json();
}

export async function getDocument(id: string): Promise<DocumentSummary> {
  const res = await fetch(`${API_BASE}/api/docs/${id}`);
  if (!res.ok) throw new Error('Document not found');
  return res.json();
}
```

### Step 3: Create the Dashboard component

- [ ] Create `src/Dashboard.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { listDocuments, createDocument, DocumentSummary } from './api';

export function Dashboard() {
  const [docs, setDocs] = useState<DocumentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const navigate = useNavigate();

  useEffect(() => {
    listDocuments()
      .then(setDocs)
      .finally(() => setLoading(false));
  }, []);

  const handleCreate = async () => {
    const doc = await createDocument('Untitled');
    navigate(`/docs/${doc.id}`);
  };

  return (
    <div className="dashboard">
      <header className="dashboard-header">
        <h1>Cadmus</h1>
        <button onClick={handleCreate} className="btn-primary">
          New Document
        </button>
      </header>
      <main className="dashboard-content">
        {loading ? (
          <p className="dashboard-loading">Loading...</p>
        ) : docs.length === 0 ? (
          <div className="dashboard-empty">
            <p>No documents yet.</p>
            <p>Create your first document to get started.</p>
          </div>
        ) : (
          <div className="document-list">
            {docs.map((doc) => (
              <button
                key={doc.id}
                className="document-card"
                onClick={() => navigate(`/docs/${doc.id}`)}
              >
                <h3>{doc.title}</h3>
                <p className="document-meta">
                  Updated {new Date(doc.updated_at).toLocaleDateString()}
                </p>
              </button>
            ))}
          </div>
        )}
      </main>
    </div>
  );
}
```

### Step 4: Create the EditorPage component

- [ ] Create `src/EditorPage.tsx`:

```tsx
import { useParams, useNavigate } from 'react-router';
import { Editor } from './Editor';
import { Presence } from './Presence';
import { useCollaboration } from './useCollaboration';

export function EditorPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { ydoc, provider, connectionStatus } = useCollaboration(id!);

  return (
    <div className="app">
      <header className="app-header">
        <button className="back-button" onClick={() => navigate('/')}>
          ← Documents
        </button>
        <h1>Cadmus</h1>
        <span className={`status-dot ${connectionStatus}`} />
        {provider && <Presence provider={provider} />}
      </header>
      <main className="app-main">
        {ydoc && provider && <Editor ydoc={ydoc} provider={provider} />}
      </main>
    </div>
  );
}
```

### Step 5: Update collaboration to use dynamic doc IDs

- [ ] In `src/collaboration.ts`, remove `DEFAULT_DOC_ID`
- [ ] The `createProvider(docId)` function already accepts a doc ID parameter — verify it works with arbitrary UUIDs
- [ ] In `src/useCollaboration.ts`, ensure the hook recreates the provider when the `docId` parameter changes (cleanup old provider, create new one)

### Step 6: Update App.tsx to be the router root

- [ ] Simplify `src/App.tsx` or remove it — the router in `main.tsx` now handles top-level routing
- [ ] If `App.tsx` had any global providers or error boundaries, move them to `main.tsx`

### Step 7: Add dashboard styles

- [ ] Add CSS for the dashboard in the existing stylesheet:

```css
/* Dashboard layout */
.dashboard { ... }
.dashboard-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1rem 2rem;
  border-bottom: 1px solid #eee;
}
.dashboard-content {
  max-width: 800px;
  margin: 2rem auto;
  padding: 0 1rem;
}

/* Document list */
.document-list {
  display: grid;
  gap: 1rem;
}
.document-card {
  /* Card styling: border, padding, hover state */
}

/* Empty state */
.dashboard-empty {
  text-align: center;
  color: #666;
  padding: 4rem 0;
}

/* Back button */
.back-button {
  /* Minimal styling, looks like a link */
}

/* Primary button */
.btn-primary {
  /* Consistent with toolbar button style */
}
```

### Step 8: Update Vite config for SPA fallback

- [ ] Verify Vite dev server handles SPA fallback for `/docs/:id` routes
- [ ] Vite's default behavior serves `index.html` for unknown routes in dev mode, so this should work out of the box
- [ ] Add `VITE_API_URL` to `.env.example`

### Step 9: Update tests

- [ ] Update any existing frontend tests to account for routing
- [ ] Add basic tests for the Dashboard component if a test framework is configured

## Verification

- [ ] `pnpm -F @cadmus/web dev` starts without errors
- [ ] Navigate to `http://localhost:5173/` — see the empty dashboard
- [ ] Click "New Document" — creates a document and navigates to `/docs/{id}`
- [ ] The editor loads and is functional (type text, formatting works)
- [ ] Click "← Documents" — returns to the dashboard
- [ ] The created document appears in the document list
- [ ] Click the document card — navigates back to the editor with the same content
- [ ] Open a second browser tab to the same `/docs/{id}` — collaborative editing works
- [ ] Direct navigation to `/docs/{id}` works (paste URL in new tab)
- [ ] Browser back/forward navigation works correctly
- [ ] `pnpm -F @cadmus/web build` succeeds

## Files Created/Modified

- `packages/web/src/main.tsx` (modified — add router setup)
- `packages/web/src/Dashboard.tsx` (new)
- `packages/web/src/EditorPage.tsx` (new)
- `packages/web/src/api.ts` (new)
- `packages/web/src/App.tsx` (modified or removed)
- `packages/web/src/collaboration.ts` (modified — remove DEFAULT_DOC_ID)
- `packages/web/src/editor.css` (modified — add dashboard styles)
- `packages/web/.env.example` (modified — add VITE_API_URL)
- `packages/web/package.json` (modified — add react-router dependency)
