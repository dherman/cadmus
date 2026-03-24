# PR 3: Sidecar Integration Tests — Implementation Plan

## Prerequisites

- [x] PR 1 (Diff Endpoint) is merged
- [x] PR 2 (Content Read Endpoints) is merged
- [x] Sidecar runs correctly (`pnpm dev:sidecar`)
- [x] Full dev stack runs (`pnpm dev`)

## Steps

### 1. Write serialize unit tests

- [x] Create `packages/sidecar/__tests__/serialize.test.ts`:

````typescript
import { describe, it, expect } from 'vitest';
import { serialize } from '../src/serialize';

function doc(...content: object[]) {
  return { type: 'doc', content };
}
function paragraph(...content: object[]) {
  return { type: 'paragraph', content };
}
function heading(level: number, ...content: object[]) {
  return { type: 'heading', attrs: { level }, content };
}
function text(value: string, marks?: object[]) {
  return marks ? { type: 'text', text: value, marks } : { type: 'text', text: value };
}
function codeBlock(language: string | null, code: string) {
  return { type: 'codeBlock', attrs: { language }, content: [{ type: 'text', text: code }] };
}
function bulletList(...items: string[]) {
  return {
    type: 'bulletList',
    content: items.map((item) => ({
      type: 'listItem',
      content: [paragraph(text(item))],
    })),
  };
}
function orderedList(start: number, ...items: string[]) {
  return {
    type: 'orderedList',
    attrs: { start },
    content: items.map((item) => ({
      type: 'listItem',
      content: [paragraph(text(item))],
    })),
  };
}

describe('serialize', () => {
  it('serializes a paragraph', () => {
    expect(serialize(doc(paragraph(text('hello'))))).toContain('hello');
  });

  it('serializes headings with ATX style', () => {
    for (let level = 1; level <= 6; level++) {
      const md = serialize(doc(heading(level, text('Title'))));
      expect(md).toContain('#'.repeat(level) + ' Title');
    }
  });

  it('serializes bold with **', () => {
    const md = serialize(doc(paragraph(text('bold', [{ type: 'bold' }]))));
    expect(md).toContain('**bold**');
  });

  it('serializes italic with *', () => {
    const md = serialize(doc(paragraph(text('italic', [{ type: 'italic' }]))));
    expect(md).toContain('*italic*');
  });

  it('serializes inline code with backticks', () => {
    const md = serialize(doc(paragraph(text('code', [{ type: 'code' }]))));
    expect(md).toContain('`code`');
  });

  it('serializes strikethrough with ~~', () => {
    const md = serialize(doc(paragraph(text('strike', [{ type: 'strike' }]))));
    expect(md).toContain('~~strike~~');
  });

  it('serializes code block with triple backticks', () => {
    const md = serialize(doc(codeBlock('rust', 'fn main() {}')));
    expect(md).toContain('```rust');
    expect(md).toContain('fn main() {}');
    expect(md).toContain('```');
  });

  it('serializes code block with no language', () => {
    const md = serialize(doc(codeBlock(null, 'plain code')));
    expect(md).toContain('```\nplain code');
  });

  it('serializes bullet list with - prefix', () => {
    const md = serialize(doc(bulletList('item one', 'item two')));
    expect(md).toContain('- item one');
    expect(md).toContain('- item two');
    expect(md).not.toContain('* item');
  });

  it('serializes ordered list with 1. prefix', () => {
    const md = serialize(doc(orderedList(1, 'first', 'second')));
    expect(md).toContain('1. first');
    expect(md).toContain('2. second');
  });

  it('serializes link with [text](url)', () => {
    const md = serialize(
      doc(
        paragraph(
          text('link', [{ type: 'link', attrs: { href: 'https://example.com', title: null } }]),
        ),
      ),
    );
    expect(md).toContain('[link](https://example.com)');
  });

  it('serializes horizontal rule as ---', () => {
    const md = serialize(
      doc(paragraph(text('before')), { type: 'horizontalRule' }, paragraph(text('after'))),
    );
    expect(md).toContain('---');
  });
});
````

### 2. Write parse unit tests

- [x] Create `packages/sidecar/__tests__/parse.test.ts`:

````typescript
import { describe, it, expect } from 'vitest';
import { parse } from '../src/parse';

describe('parse', () => {
  it('parses a paragraph', () => {
    const json = parse('Hello world\n');
    expect(json.type).toBe('doc');
    expect(json.content[0].type).toBe('paragraph');
  });

  it('parses ATX headings', () => {
    for (let level = 1; level <= 6; level++) {
      const json = parse('#'.repeat(level) + ' Title\n');
      expect(json.content[0].type).toBe('heading');
      expect(json.content[0].attrs.level).toBe(level);
    }
  });

  it('parses bold **text**', () => {
    const json = parse('**bold**\n');
    const textNode = json.content[0].content[0];
    expect(textNode.marks?.some((m: { type: string }) => m.type === 'bold')).toBe(true);
  });

  it('parses italic *text*', () => {
    const json = parse('*italic*\n');
    const textNode = json.content[0].content[0];
    expect(textNode.marks?.some((m: { type: string }) => m.type === 'italic')).toBe(true);
  });

  it('parses inline code `text`', () => {
    const json = parse('`code`\n');
    const textNode = json.content[0].content[0];
    expect(textNode.marks?.some((m: { type: string }) => m.type === 'code')).toBe(true);
  });

  it('parses fenced code block with language', () => {
    const json = parse('```rust\nfn main() {}\n```\n');
    expect(json.content[0].type).toBe('codeBlock');
    expect(json.content[0].attrs?.language).toBe('rust');
  });

  it('parses bullet list', () => {
    const json = parse('- first\n- second\n');
    expect(json.content[0].type).toBe('bulletList');
    expect(json.content[0].content).toHaveLength(2);
  });

  it('parses ordered list', () => {
    const json = parse('1. first\n2. second\n');
    expect(json.content[0].type).toBe('orderedList');
  });

  it('parses links', () => {
    const json = parse('[text](https://example.com)\n');
    const textNode = json.content[0].content[0];
    const linkMark = textNode.marks?.find((m: { type: string }) => m.type === 'link');
    expect(linkMark?.attrs?.href).toBe('https://example.com');
  });

  it('parses blockquote', () => {
    const json = parse('> quoted text\n');
    expect(json.content[0].type).toBe('blockquote');
  });

  it('parses horizontal rule', () => {
    const json = parse('---\n');
    expect(json.content[0].type).toBe('horizontalRule');
  });
});
````

### 3. Write round-trip integration tests

- [x] Create `packages/sidecar/__tests__/round-trip.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { serialize } from '../src/serialize';
import { parse } from '../src/parse';
import type { JSONContent } from '@tiptap/core';

function roundTrip(doc: JSONContent): JSONContent {
  const markdown = serialize(doc);
  return parse(markdown);
}

// Helpers
function doc(...content: object[]): JSONContent {
  return { type: 'doc', content } as JSONContent;
}
// ... (same helpers as above)

describe('round-trip fidelity', () => {
  it('paragraph with plain text', () => {
    const original = doc({ type: 'paragraph', content: [{ type: 'text', text: 'Hello world' }] });
    expect(roundTrip(original)).toEqual(original);
  });

  it('heading levels 1-6', () => {
    for (let level = 1; level <= 6; level++) {
      const original = doc({
        type: 'heading',
        attrs: { level },
        content: [{ type: 'text', text: 'Title' }],
      });
      expect(roundTrip(original)).toEqual(original);
    }
  });

  it('bold text', () => {
    const original = doc({
      type: 'paragraph',
      content: [{ type: 'text', text: 'bold', marks: [{ type: 'bold' }] }],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('italic text', () => {
    const original = doc({
      type: 'paragraph',
      content: [{ type: 'text', text: 'italic', marks: [{ type: 'italic' }] }],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('inline code', () => {
    const original = doc({
      type: 'paragraph',
      content: [{ type: 'text', text: 'code', marks: [{ type: 'code' }] }],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('strikethrough', () => {
    const original = doc({
      type: 'paragraph',
      content: [{ type: 'text', text: 'strike', marks: [{ type: 'strike' }] }],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('link', () => {
    const original = doc({
      type: 'paragraph',
      content: [
        {
          type: 'text',
          text: 'link',
          marks: [{ type: 'link', attrs: { href: 'https://example.com', title: null } }],
        },
      ],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('code block with language', () => {
    const original = doc({
      type: 'codeBlock',
      attrs: { language: 'rust' },
      content: [{ type: 'text', text: 'fn main() {}' }],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('bullet list', () => {
    const original = doc({
      type: 'bulletList',
      content: [
        {
          type: 'listItem',
          content: [{ type: 'paragraph', content: [{ type: 'text', text: 'one' }] }],
        },
        {
          type: 'listItem',
          content: [{ type: 'paragraph', content: [{ type: 'text', text: 'two' }] }],
        },
      ],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('ordered list', () => {
    const original = doc({
      type: 'orderedList',
      attrs: { start: 1 },
      content: [
        {
          type: 'listItem',
          content: [{ type: 'paragraph', content: [{ type: 'text', text: 'first' }] }],
        },
        {
          type: 'listItem',
          content: [{ type: 'paragraph', content: [{ type: 'text', text: 'second' }] }],
        },
      ],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('blockquote', () => {
    const original = doc({
      type: 'blockquote',
      content: [{ type: 'paragraph', content: [{ type: 'text', text: 'quote' }] }],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('horizontal rule', () => {
    const original = doc(
      { type: 'horizontalRule' },
      { type: 'paragraph', content: [{ type: 'text', text: 'after' }] },
    );
    expect(roundTrip(original)).toEqual(original);
  });

  it('heading with bold text', () => {
    const original = doc({
      type: 'heading',
      attrs: { level: 2 },
      content: [{ type: 'text', text: 'bold title', marks: [{ type: 'bold' }] }],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('nested lists', () => {
    const original = doc({
      type: 'bulletList',
      content: [
        {
          type: 'listItem',
          content: [
            { type: 'paragraph', content: [{ type: 'text', text: 'outer' }] },
            {
              type: 'bulletList',
              content: [
                {
                  type: 'listItem',
                  content: [{ type: 'paragraph', content: [{ type: 'text', text: 'inner' }] }],
                },
              ],
            },
          ],
        },
      ],
    });
    expect(roundTrip(original)).toEqual(original);
  });

  it('multiple block types in sequence', () => {
    const original = doc(
      { type: 'heading', attrs: { level: 1 }, content: [{ type: 'text', text: 'Title' }] },
      { type: 'paragraph', content: [{ type: 'text', text: 'intro paragraph' }] },
      {
        type: 'bulletList',
        content: [
          {
            type: 'listItem',
            content: [{ type: 'paragraph', content: [{ type: 'text', text: 'item' }] }],
          },
        ],
      },
      {
        type: 'codeBlock',
        attrs: { language: 'typescript' },
        content: [{ type: 'text', text: 'const x = 1;' }],
      },
    );
    expect(roundTrip(original)).toEqual(original);
  });
});
```

### 4. Add a `test` script to the sidecar package.json (if not already present)

- [x] Verify `packages/sidecar/package.json` has a test script:

```json
{
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest"
  }
}
```

- [x] Add a `vitest.config.ts` if needed (often not required with default config).

### 5. Run all tests

- [x] Run sidecar tests: `pnpm -F @cadmus/sidecar test`
- [x] Verify all round-trip, serialize, parse, and diff tests pass.
- [x] Run server tests: `cargo test` (with sidecar integration tests gated on env var)
- [x] Run `pnpm run format:check`

### 6. Document known non-round-trippable constructs

If any schema constructs fail to round-trip (unlikely with the designed schema but possible for edge cases), document them explicitly in the test as `it.skip` with a comment explaining why. Do not silently suppress failures.

## Verification

- [x] All serialize unit tests pass
- [x] All parse unit tests pass
- [x] All round-trip tests pass for every node type in the schema
- [x] All round-trip tests pass for every mark type in the schema
- [x] Round-trip tests pass for complex compositions (nested lists, mixed marks, etc.)
- [x] Diff unit tests from PR 1 continue to pass

## Files Modified

| File                                            | Change                            |
| ----------------------------------------------- | --------------------------------- |
| `packages/sidecar/__tests__/serialize.test.ts`  | New: serialize unit tests         |
| `packages/sidecar/__tests__/parse.test.ts`      | New: parse unit tests             |
| `packages/sidecar/__tests__/round-trip.test.ts` | New: round-trip integration tests |
| `packages/sidecar/package.json`                 | Add `test` script if missing      |
