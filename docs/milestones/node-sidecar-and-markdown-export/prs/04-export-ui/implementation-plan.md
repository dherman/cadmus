# PR 4: Export UI — Implementation Plan

## Prerequisites

- [ ] PR 2 (Content Read Endpoints) is merged — the `GET /api/docs/{id}/content` endpoint must work

## Steps

### 1. Add `fetchDocumentContent` to the API module

- [x] Add to `packages/web/src/api.ts`:

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

### 2. Add the export handler and button to `EditorPage.tsx`

- [x] Add a `downloadMarkdown` helper at the top of the file (or in a utility module):

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

function slugify(title: string): string {
  const slug = title
    .toLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^a-z0-9-]/g, '');
  return slug || 'document';
}
```

- [x] In `EditorPageInner`, add export state and handler:

```typescript
const [exporting, setExporting] = useState(false);
const [exportError, setExportError] = useState<string | null>(null);

async function handleExport() {
  setExporting(true);
  setExportError(null);
  try {
    const result = await fetchDocumentContent(docId, 'markdown');
    downloadMarkdown(`${slugify(doc.title)}.md`, result.content as string);
  } catch (err) {
    setExportError(err instanceof Error ? err.message : 'Export failed');
  } finally {
    setExporting(false);
  }
}
```

- [x] Add the Export button and error display to the header JSX, alongside the existing Share button:

```tsx
<button className="btn-export" onClick={handleExport} disabled={exporting}>
  {exporting ? 'Exporting…' : 'Export'}
</button>;
{
  exportError && <span className="export-error">{exportError}</span>;
}
```

- [x] Add `fetchDocumentContent` to the import from `./api`.

### 3. Add styles

- [x] Add minimal CSS to the existing stylesheet (whichever file contains `.btn-share` styles):

```css
.btn-export {
  /* Match btn-share base style */
  padding: 0.25rem 0.75rem;
  border-radius: 4px;
  border: 1px solid var(--color-border, #e0e0e0);
  background: transparent;
  cursor: pointer;
  font-size: 0.875rem;
}

.btn-export:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.export-error {
  font-size: 0.8rem;
  color: var(--color-error, #e06c75);
  margin-left: 0.5rem;
}
```

### 4. Manual verification

- [ ] Start the full dev stack: `pnpm dev`
- [ ] Create a document and add content with various formatting (headings, bold, lists, code).
- [ ] Click Export — verify the file downloads with the correct name (`<slug>.md`).
- [ ] Open the downloaded `.md` file and verify it contains canonical markdown matching the editor content.
- [ ] Verify the button shows "Exporting…" briefly during the request.
- [ ] Stop the sidecar (`Ctrl+C` in that terminal) and click Export again — verify an error message appears.
- [ ] Verify the Export button appears for Read-role users (open the doc as a shared user with read access).

### 5. Run tests and check formatting

- [x] Run `pnpm -F @cadmus/web build` — verify TypeScript compiles without errors.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [ ] Export button visible in editor header for all role levels (read, comment, edit)
- [ ] Clicking Export downloads a `.md` file
- [ ] Downloaded filename is `<document-slug>.md`
- [ ] Markdown content in downloaded file matches the document as rendered in the editor
- [ ] Button shows disabled/loading state during export
- [ ] Error message appears (and disappears on next successful export) when export fails
- [ ] TypeScript compiles cleanly with no new type errors

## Files Modified

| File                                       | Change                                                  |
| ------------------------------------------ | ------------------------------------------------------- |
| `packages/web/src/api.ts`                  | Add `fetchDocumentContent`, `DocumentContent` interface |
| `packages/web/src/EditorPage.tsx`          | Add export handler, button, error display               |
| `packages/web/src/app.css` (or equivalent) | Add `.btn-export` and `.export-error` styles            |
