# PR 4: Frontend Document Dashboard

## Purpose

Add a document dashboard to the React frontend so users can create new documents, browse existing ones, and navigate to the editor. This replaces the current single-document experience (hardcoded `DEFAULT_DOC_ID`) with multi-document support.

After this PR, users land on a dashboard showing all documents, can create a new one, and click to open it in the editor. The browser URL reflects which document is open, so direct links and browser navigation work.

## User Experience

### Dashboard View (`/`)

- Header: "Cadmus" title, "New Document" button
- Document list: cards or rows showing title, last updated time
- Click a document → navigate to `/docs/{id}`
- Empty state: friendly message prompting the user to create their first document

### Editor View (`/docs/{id}`)

- Same editor experience as Milestone 1
- Header shows document title (editable inline), back arrow to dashboard
- Connection status dot and presence list remain

### Navigation Flow

```
/ (Dashboard)
  → click doc → /docs/{id} (Editor)
  → click "New" → POST /api/docs → redirect to /docs/{new_id}

/docs/{id} (Editor)
  → click back → / (Dashboard)
```

## Routing

We add client-side routing with React Router. The app has two routes:

| Path         | Component   | Description              |
| ------------ | ----------- | ------------------------ |
| `/`          | `Dashboard` | Document list and create |
| `/docs/:id`  | `App`       | Editor (renamed/refactored from current `App`) |

React Router is the standard choice for React SPAs. We use `BrowserRouter` with `createBrowserRouter` for data loading support.

### Why Not Hash Routing

Hash routing (`/#/docs/123`) avoids server-side config but looks worse in URLs and breaks if we later want server-side rendering. `BrowserRouter` with history API is the right default. The Vite dev server already handles SPA fallback.

## API Integration

The dashboard fetches documents from the REST API:

```typescript
const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

async function listDocuments(): Promise<DocumentSummary[]> {
  const res = await fetch(`${API_BASE}/api/docs`);
  return res.json();
}

async function createDocument(title: string): Promise<DocumentSummary> {
  const res = await fetch(`${API_BASE}/api/docs`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ title }),
  });
  return res.json();
}

async function deleteDocument(id: string): Promise<void> {
  await fetch(`${API_BASE}/api/docs/${id}`, { method: 'DELETE' });
}
```

A new `VITE_API_URL` env var is introduced for the REST API base URL. This is separate from `VITE_WS_URL` because the WebSocket and REST endpoints could be on different hosts in production (though they're the same server for now).

## Component Structure

```
src/
  main.tsx          — Router setup
  Dashboard.tsx     — Document list + create
  EditorPage.tsx    — Editor wrapper (reads doc ID from URL params)
  Editor.tsx        — (existing, unchanged)
  Toolbar.tsx       — (existing, unchanged)
  Presence.tsx      — (existing, unchanged)
  api.ts            — REST API client functions
  collaboration.ts  — (modified: remove DEFAULT_DOC_ID)
```

### Why EditorPage, Not Just App?

The current `App.tsx` conflates routing-level concerns (which document to load) with rendering (editor, header, presence). `EditorPage` takes the document ID from the URL and passes it to the collaboration hook, keeping the editor components clean. The old `App.tsx` becomes the router root.

## Styling

Dashboard styling follows the same approach as M1 — vanilla CSS in `editor.css` (renamed to `app.css` or kept as-is). No component library.

- Document list uses a simple card layout
- Responsive: single column on mobile, multi-column grid on wider screens
- "New Document" button styled consistently with the toolbar

## What's Not Included

- **Document rename from dashboard** — editing the title is available in the editor header, not the dashboard list. Keeps the dashboard simple.
- **Document delete from dashboard** — deferred. Delete is a destructive action that should have a confirmation dialog, which adds UI complexity. The API endpoint exists; the UI can be added later.
- **Search/filter** — no auth means all documents are shown. Filtering becomes useful when users have their own documents (M3).
- **Pagination** — not needed until the document count warrants it.
