# PR 4: Export UI

## Purpose

Add an "Export as Markdown" button to the editor that fetches the document's canonical markdown via the content endpoint and triggers a browser file download. This is the user-visible deliverable of the milestone — the end-to-end demo that shows the sidecar pipeline working from the editor's perspective.

## UI Design

### Button Placement

The export button lives in the editor header alongside the existing Share button. It is visible to all users with access to the document (Read, Comment, or Edit), since reading document content requires only Read permission.

```
┌─────────────────────────────────────────────────────────┐
│ ← Documents   Cadmus        ●  [User A] [Share] [Export]│
├─────────────────────────────────────────────────────────┤
│                                                         │
│   Editor content...                                     │
```

### Interaction Flow

1. User clicks "Export" button.
2. Button shows a loading state (spinner or "Exporting…" text).
3. Frontend calls `GET /api/docs/{id}/content?format=markdown` with auth header.
4. On success: browser file download triggers with filename `<document-title>.md`.
5. On error: brief error message shown near the button (e.g., "Export failed — try again").
6. Button returns to normal state.

### File Naming

The downloaded file uses the document title as the filename:

- Title: `"My Document"` → filename: `my-document.md`
- Slugification: lowercase, spaces to hyphens, strip non-alphanumeric characters (except hyphens).
- Fallback: `document.md` if the title is empty or produces an empty slug.

### Implementation via Blob URL

No server-side file generation. The frontend:

1. Receives the markdown string from the API.
2. Creates a `Blob` with type `text/markdown`.
3. Creates an object URL via `URL.createObjectURL(blob)`.
4. Programmatically clicks a hidden `<a>` element with `download` attribute set to the filename.
5. Revokes the object URL after the click.

```typescript
function downloadMarkdown(filename: string, content: string) {
  const blob = new Blob([content], { type: 'text/markdown' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
```

## API Layer

Add a `fetchDocumentContent` function to `packages/web/src/api.ts`:

```typescript
export interface DocumentContent {
  format: 'markdown' | 'json';
  content: string | object;
}

export async function fetchDocumentContent(
  id: string,
  format: 'markdown' | 'json' = 'json',
): Promise<DocumentContent> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(id)}/content?format=${format}`,
  );
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Failed to fetch document content');
  }
  return res.json();
}
```

## Component Changes

### `EditorPage.tsx`

Add export state and handler to `EditorPageInner`:

```typescript
const [exporting, setExporting] = useState(false);
const [exportError, setExportError] = useState<string | null>(null);

async function handleExport() {
  setExporting(true);
  setExportError(null);
  try {
    const result = await fetchDocumentContent(docId, 'markdown');
    const slug =
      doc.title
        .toLowerCase()
        .replace(/\s+/g, '-')
        .replace(/[^a-z0-9-]/g, '') || 'document';
    downloadMarkdown(`${slug}.md`, result.content as string);
  } catch (err) {
    setExportError(err instanceof Error ? err.message : 'Export failed');
  } finally {
    setExporting(false);
  }
}
```

The Export button renders in the header:

```tsx
<button className="btn-export" onClick={handleExport} disabled={exporting}>
  {exporting ? 'Exporting…' : 'Export'}
</button>;
{
  exportError && <span className="export-error">{exportError}</span>;
}
```

### Styling

Minimal CSS additions to `app.css` (or equivalent):

```css
.btn-export {
  /* Same base style as btn-share */
}

.export-error {
  font-size: 0.8rem;
  color: var(--color-error, #e06c75);
  margin-left: 0.5rem;
}
```

## Permission Note

The Export button is shown to all users (any role), since the content endpoint requires only Read permission. However, it is only rendered when the document and session have loaded — the same loading gate that controls the editor rendering.

## What's Not Included

- Export as JSON format (not shown to the user for now — the JSON format is for programmatic access, not manual download).
- Custom filename prompt (the auto-generated slug is sufficient for the prototype).
- PDF or DOCX export (out of scope for this milestone; those would require server-side conversion).
