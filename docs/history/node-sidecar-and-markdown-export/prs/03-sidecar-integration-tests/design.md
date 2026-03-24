# PR 3: Sidecar Integration Tests

## Purpose

Validate end-to-end round-trip fidelity: every schema construct can be created as ProseMirror JSON, serialized to canonical markdown, parsed back, and produce the identical JSON. This is the milestone's key validation — it proves the sidecar architecture works reliably and that serialization matches what the frontend editor produces.

Also adds sidecar unit tests for serialize and parse (the diff unit tests land in PR 1), and a server-side integration test that exercises the full content endpoint pipeline with the sidecar running.

## Test Suite Structure

```
packages/sidecar/__tests__/
  serialize.test.ts     # Unit: ProseMirror JSON → markdown
  parse.test.ts         # Unit: markdown → ProseMirror JSON
  diff.test.ts          # Unit: (lands in PR 1)
  round-trip.test.ts    # Integration: JSON → markdown → JSON round-trips
```

## Round-Trip Test Coverage

Every schema construct defined in `docs/architecture/schema-design.md` must have a round-trip test. The test structure for each:

```typescript
it('round-trips <construct>', () => {
  const original = /* ProseMirror JSON */;
  const markdown = serialize(original);
  const reparsed = parse(markdown);
  expect(reparsed).toEqual(original);
});
```

### Required Test Cases

**Nodes:**

| Test Case                | Original JSON                                           | Expected Markdown               |
| ------------------------ | ------------------------------------------------------- | ------------------------------- |
| Simple paragraph         | `paragraph > text("hello")`                             | `hello`                         |
| Heading levels 1–6       | `heading(level: N) > text("title")`                     | `# title` … `###### title`      |
| Fenced code block        | `codeBlock(language: "rust") > text("fn main() {}")`    | ` ```rust\nfn main() {}\n``` `  |
| Code block (no language) | `codeBlock > text("plain")`                             | ` ```\nplain\n``` `             |
| Blockquote               | `blockquote > paragraph > text("quote")`                | `> quote`                       |
| Bullet list              | `bulletList > listItem > paragraph > text("item")`      | `- item`                        |
| Ordered list             | `orderedList > listItem > paragraph > text("item")`     | `1. item`                       |
| Ordered list with start  | `orderedList(start: 3) > ...`                           | `3. item`                       |
| Nested lists             | bullet inside bullet, ordered inside bullet             | `- item\n  - nested`            |
| Horizontal rule          | `horizontalRule`                                        | `---`                           |
| Image                    | `image(src, alt, title)`                                | `![alt](src "title")`           |
| Image (no title)         | `image(src, alt, title: null)`                          | `![alt](src)`                   |
| Hard break               | `paragraph > [text("line1"), hardBreak, text("line2")]` | `line1\\\nline2`                |
| Multiple paragraphs      | Two paragraphs                                          | Two blank-line-separated blocks |

**Marks:**

| Test Case            | Original JSON                             | Expected Markdown      |
| -------------------- | ----------------------------------------- | ---------------------- |
| Bold                 | `text("bold", [bold])`                    | `**bold**`             |
| Italic               | `text("italic", [italic])`                | `*italic*`             |
| Inline code          | `text("code", [code])`                    | `` `code` ``           |
| Strikethrough        | `text("strike", [strike])`                | `~~strike~~`           |
| Link                 | `text("link", [link(href, title)])`       | `[link](href "title")` |
| Link (no title)      | `text("link", [link(href, title: null)])` | `[link](href)`         |
| Bold + italic        | `text("both", [bold, italic])`            | `***both***`           |
| Code excludes others | `text("code", [code, bold])` → code wins  | `` `code` ``           |

**Complex compositions:**

| Test Case                        | Description                                          |
| -------------------------------- | ---------------------------------------------------- |
| Heading with inline marks        | `heading(1) > [text("bold", [bold]), text(" rest")]` |
| List item with nested code block | list item containing a `codeBlock` child             |
| Blockquote containing a list     | `blockquote > bulletList > listItem > ...`           |
| Mixed marks in paragraph         | Multiple mark types applied to different text spans  |
| Deep nesting                     | Three levels of nested lists                         |

## Serialize and Parse Unit Tests

### `serialize.test.ts`

Tests that specific ProseMirror JSON inputs produce the expected canonical markdown string:

```typescript
it('serializes heading to ATX style', () => {
  const doc = {
    type: 'doc',
    content: [{ type: 'heading', attrs: { level: 2 }, content: [{ type: 'text', text: 'Title' }] }],
  };
  expect(serialize(doc)).toBe('## Title\n');
});
```

Key cases: heading format, code fence syntax, bullet prefix (`-` not `*`), ordered list numbering, bold/italic syntax, canonical link format.

### `parse.test.ts`

Tests that specific markdown strings produce the expected ProseMirror JSON:

```typescript
it('parses ATX heading', () => {
  const json = parse('# Hello\n');
  expect(json.content[0].type).toBe('heading');
  expect(json.content[0].attrs.level).toBe(1);
});
```

Key cases: heading parsing, list detection, code fence language extraction, link attribute extraction.

## Full-Stack Integration Test

In addition to the Node-side tests, a Rust integration test validates the full pipeline:

**`packages/server/tests/content_test.rs`:**

1. Start a test server with the sidecar URL configured.
2. Register a user, create a document.
3. Connect via WebSocket, insert content via Yrs updates.
4. Call `GET /api/docs/{id}/content?format=markdown`.
5. Assert the returned markdown matches the inserted content.

This requires the sidecar to be running. The test should be gated behind a feature flag or environment variable (`SIDECAR_URL`) so it doesn't fail in CI environments where the sidecar isn't available. If `SIDECAR_URL` is not set, the test is skipped.

## What This Doesn't Test

- The diff endpoint's Step correctness when applied via Yrs (deferred to M6, where the translation layer is built).
- Performance under load (sidecar latency, memory usage with large documents).
- Schema version mismatch behavior (would require running two different sidecar versions simultaneously).

## Dependencies

- PR 1 (diff tests) must be merged so all three sidecar endpoints are covered.
- PR 2 (content endpoint) must be merged for the full-stack integration test.
