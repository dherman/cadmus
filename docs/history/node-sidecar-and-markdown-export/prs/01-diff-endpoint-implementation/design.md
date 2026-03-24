# PR 1: Diff Endpoint Implementation

## Purpose

Implement the sidecar's `/diff` endpoint using `prosemirror-recreate-transform` to compute ProseMirror Step sequences between two document states. This is currently a TODO placeholder that returns an empty array. The diff endpoint is essential for M6's push/merge workflow, but we implement and validate it now so the full sidecar pipeline is proven before downstream milestones depend on it.

## Approach

The existing `diff.ts` already creates a headless Tiptap editor and reconstructs ProseMirror nodes from JSON. The missing piece is the actual diff computation. We'll add the `prosemirror-recreate-transform` library, which provides a `recreateTransform` function that takes two ProseMirror `Node` instances and returns a `Transform` containing the Step sequence.

### Diff Implementation

```typescript
import { recreateTransform } from 'prosemirror-recreate-transform';

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

### Step Types Produced

`prosemirror-recreate-transform` produces standard ProseMirror Steps:

| Step Type            | When Produced                                 | Example                       |
| -------------------- | --------------------------------------------- | ----------------------------- |
| `replace`            | Text inserted, deleted, or nodes restructured | Adding a paragraph of text    |
| `replaceAround`      | Content wrapped or unwrapped                  | Toggling a blockquote         |
| `addMark`            | Formatting applied to a range                 | Bolding a word                |
| `removeMark`         | Formatting removed from a range               | Removing italic from a phrase |
| `replaceStep` (attr) | Node attributes changed                       | Changing heading level        |

Each Step includes position information (`from`, `to`) and the content/mark being applied. The Rust server will consume these in M6 to translate into Yrs operations.

### Error Handling

| Scenario                                | Response           |
| --------------------------------------- | ------------------ |
| Invalid `old_doc` JSON                  | 500, parse error   |
| Invalid `new_doc` JSON                  | 500, parse error   |
| Documents don't match schema            | 500, schema error  |
| Identical documents                     | 200, empty `steps` |
| `prosemirror-recreate-transform` throws | 500, diff error    |

## Unit Tests

The diff endpoint needs its own test suite (`__tests__/diff.test.ts`) covering:

1. **Identical documents** → empty steps array
2. **Text insertion** → ReplaceStep with inserted content
3. **Text deletion** → ReplaceStep with empty slice
4. **Text replacement** → ReplaceStep
5. **Mark addition** (bold a word) → AddMarkStep
6. **Mark removal** (unbold a word) → RemoveMarkStep
7. **Structural change** (paragraph → heading) → ReplaceStep changing node type
8. **List manipulation** (add/remove list items) → ReplaceStep(s)
9. **Complex multi-change** (multiple edits in one diff) → multiple Steps

Each test creates two ProseMirror JSON documents, runs `diff()`, and verifies the Steps array is non-empty and that applying the Steps to `oldDoc` produces `newDoc`.

## Dependencies

**New npm dependency:**

- `prosemirror-recreate-transform` — ProseMirror Step reconstruction from two document states

**Existing (already in package.json):**

- `@cadmus/doc-schema`, `@tiptap/core`, `@tiptap/pm`

## What's Not Included

- Server-side consumption of the Steps (deferred to M6 — the Step → Yrs translation layer)
- The `POST /api/docs/{id}/content` endpoint (deferred to M6)
- Performance optimization for large documents (acceptable for prototype)
