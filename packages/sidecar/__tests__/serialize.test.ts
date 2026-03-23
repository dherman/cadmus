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

  it('serializes bold + italic as ***', () => {
    const md = serialize(doc(paragraph(text('both', [{ type: 'bold' }, { type: 'italic' }]))));
    expect(md).toContain('***both***');
  });

  it('serializes blockquote', () => {
    const md = serialize(doc({ type: 'blockquote', content: [paragraph(text('quoted'))] }));
    expect(md).toContain('> quoted');
  });

  it('serializes hard break', () => {
    const md = serialize(doc(paragraph(text('line1'), { type: 'hardBreak' }, text('line2'))));
    // Hard breaks in markdown are represented as trailing backslash or two spaces + newline
    expect(md).toContain('line1');
    expect(md).toContain('line2');
  });
});
