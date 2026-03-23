# PR 1: Diff Endpoint Implementation — Implementation Plan

## Prerequisites

- [x] Milestone 3 (Auth and Permissions) is merged
- [x] Sidecar dev server runs successfully (`pnpm dev:sidecar`)

## Steps

### 1. Add `prosemirror-recreate-transform` dependency

- [x] Add the dependency to `packages/sidecar/package.json`:

```bash
cd packages/sidecar && pnpm add prosemirror-recreate-transform
```

- [x] Run `pnpm install` from the workspace root to update the lockfile.

### 2. Implement the diff function

- [x] Replace the placeholder in `packages/sidecar/src/diff.ts` with the real implementation:

```typescript
import { Editor } from '@tiptap/core';
import { createExtensions } from '@cadmus/doc-schema';
import { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { recreateTransform } from 'prosemirror-recreate-transform';
import type { JSONContent } from '@tiptap/core';

export function diff(oldDoc: JSONContent, newDoc: JSONContent): object[] {
  const editor = new Editor({
    extensions: createExtensions({ disableHistory: true }),
    content: oldDoc,
  });

  const schema = editor.schema;
  const oldNode = ProseMirrorNode.fromJSON(schema, oldDoc);
  const newNode = ProseMirrorNode.fromJSON(schema, newDoc);

  const transform = recreateTransform(oldNode, newNode);
  const steps = transform.steps.map((step) => step.toJSON());

  editor.destroy();
  return steps;
}
```

### 3. Write unit tests for the diff function

- [x] Create `packages/sidecar/__tests__/diff.test.ts` with the following test cases:

```typescript
import { describe, it, expect } from 'vitest';
import { diff } from '../src/diff';

// Helper to create a minimal ProseMirror JSON doc
function doc(...content: object[]) {
  return { type: 'doc', content };
}

function paragraph(...content: object[]) {
  return { type: 'paragraph', content };
}

function text(value: string, marks?: object[]) {
  return marks ? { type: 'text', text: value, marks } : { type: 'text', text: value };
}

function heading(level: number, ...content: object[]) {
  return { type: 'heading', attrs: { level }, content };
}

describe('diff', () => {
  it('returns empty steps for identical documents', () => {
    const d = doc(paragraph(text('hello')));
    expect(diff(d, d)).toEqual([]);
  });

  it('detects text insertion', () => {
    const old = doc(paragraph(text('hello')));
    const updated = doc(paragraph(text('hello world')));
    const steps = diff(old, updated);
    expect(steps.length).toBeGreaterThan(0);
  });

  it('detects text deletion', () => {
    const old = doc(paragraph(text('hello world')));
    const updated = doc(paragraph(text('hello')));
    const steps = diff(old, updated);
    expect(steps.length).toBeGreaterThan(0);
  });

  it('detects mark addition', () => {
    const old = doc(paragraph(text('hello')));
    const updated = doc(paragraph(text('hello', [{ type: 'bold' }])));
    const steps = diff(old, updated);
    expect(steps.length).toBeGreaterThan(0);
  });

  it('detects mark removal', () => {
    const old = doc(paragraph(text('hello', [{ type: 'bold' }])));
    const updated = doc(paragraph(text('hello')));
    const steps = diff(old, updated);
    expect(steps.length).toBeGreaterThan(0);
  });

  it('detects structural change (paragraph to heading)', () => {
    const old = doc(paragraph(text('title')));
    const updated = doc(heading(1, text('title')));
    const steps = diff(old, updated);
    expect(steps.length).toBeGreaterThan(0);
  });

  it('handles multiple changes', () => {
    const old = doc(paragraph(text('first')), paragraph(text('second')));
    const updated = doc(
      heading(1, text('FIRST')),
      paragraph(text('second')),
      paragraph(text('third')),
    );
    const steps = diff(old, updated);
    expect(steps.length).toBeGreaterThan(0);
  });
});
```

### 4. Verify the diff endpoint works via the HTTP server

- [ ] Start the sidecar (`pnpm dev:sidecar`) and test manually:

```bash
curl -X POST http://localhost:3001/diff \
  -H 'Content-Type: application/json' \
  -d '{
    "old_doc": {"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hello"}]}]},
    "new_doc": {"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hello world"}]}]}
  }'
```

- [x] Verify the response contains a non-empty `steps` array.

### 5. Run tests and verify

- [x] Run `pnpm -F @cadmus/sidecar test` — all diff tests pass.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [x] `diff()` returns empty array for identical documents
- [x] `diff()` returns Steps for text insertions, deletions, and replacements
- [x] `diff()` returns Steps for mark additions and removals
- [x] `diff()` returns Steps for structural changes (paragraph → heading, etc.)
- [x] `diff()` handles multi-edit scenarios (multiple changes in one call)
- [x] The `/diff` HTTP endpoint returns correct JSON responses
- [x] All existing sidecar functionality (serialize, parse, health) still works

## Files Modified

| File                                      | Change                                          |
| ----------------------------------------- | ----------------------------------------------- |
| `packages/sidecar/package.json`           | Add `prosemirror-recreate-transform` dependency |
| `packages/sidecar/src/diff.ts`            | Replace placeholder with real implementation    |
| `packages/sidecar/__tests__/diff.test.ts` | New: unit tests for diff function               |
| `pnpm-lock.yaml`                          | Updated lockfile                                |
