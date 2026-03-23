import { describe, it, expect } from 'vitest';
import { serialize } from '../src/serialize';
import { parse } from '../src/parse';
import type { JSONContent } from '@tiptap/core';

function roundTrip(doc: JSONContent): JSONContent {
  const markdown = serialize(doc);
  return parse(markdown);
}

function doc(...content: object[]): JSONContent {
  return { type: 'doc', content } as JSONContent;
}
function paragraph(...content: object[]): JSONContent {
  return { type: 'paragraph', content } as JSONContent;
}
function heading(level: number, ...content: object[]): JSONContent {
  return { type: 'heading', attrs: { level }, content } as JSONContent;
}
function text(value: string, marks?: object[]): JSONContent {
  return marks
    ? ({ type: 'text', text: value, marks } as JSONContent)
    : ({ type: 'text', text: value } as JSONContent);
}
function codeBlock(language: string | null, code: string): JSONContent {
  return {
    type: 'codeBlock',
    attrs: { language },
    content: [{ type: 'text', text: code }],
  } as JSONContent;
}

describe('round-trip fidelity', () => {
  // --- Nodes ---

  it('paragraph with plain text', () => {
    const original = doc(paragraph(text('Hello world')));
    expect(roundTrip(original)).toEqual(original);
  });

  it('heading levels 1-6', () => {
    for (let level = 1; level <= 6; level++) {
      const original = doc(heading(level, text('Title')));
      expect(roundTrip(original)).toEqual(original);
    }
  });

  it('code block with language', () => {
    const original = doc(codeBlock('rust', 'fn main() {}'));
    expect(roundTrip(original)).toEqual(original);
  });

  it('code block without language', () => {
    const original = doc(codeBlock(null, 'plain code'));
    expect(roundTrip(original)).toEqual(original);
  });

  it('blockquote', () => {
    const original = doc({
      type: 'blockquote',
      content: [paragraph(text('quote'))],
    } as JSONContent);
    expect(roundTrip(original)).toEqual(original);
  });

  it('bullet list', () => {
    const original = doc({
      type: 'bulletList',
      content: [
        {
          type: 'listItem',
          content: [paragraph(text('one'))],
        },
        {
          type: 'listItem',
          content: [paragraph(text('two'))],
        },
      ],
    } as JSONContent);
    expect(roundTrip(original)).toEqual(original);
  });

  it('ordered list', () => {
    const original = doc({
      type: 'orderedList',
      attrs: { start: 1 },
      content: [
        {
          type: 'listItem',
          content: [paragraph(text('first'))],
        },
        {
          type: 'listItem',
          content: [paragraph(text('second'))],
        },
      ],
    } as JSONContent);
    const result = roundTrip(original);
    // TipTap adds a `type` attr to orderedList — normalize before comparing
    expect(result.content![0].attrs).toMatchObject({ start: 1 });
    expect(result.content![0].content).toEqual(original.content![0].content);
  });

  it('horizontal rule', () => {
    const original = doc({ type: 'horizontalRule' } as JSONContent, paragraph(text('after')));
    expect(roundTrip(original)).toEqual(original);
  });

  it('multiple paragraphs', () => {
    const original = doc(paragraph(text('first')), paragraph(text('second')));
    expect(roundTrip(original)).toEqual(original);
  });

  // --- Marks ---

  it('bold text', () => {
    const original = doc(paragraph(text('bold', [{ type: 'bold' }])));
    expect(roundTrip(original)).toEqual(original);
  });

  it('italic text', () => {
    const original = doc(paragraph(text('italic', [{ type: 'italic' }])));
    expect(roundTrip(original)).toEqual(original);
  });

  it('inline code', () => {
    const original = doc(paragraph(text('code', [{ type: 'code' }])));
    expect(roundTrip(original)).toEqual(original);
  });

  it('strikethrough', () => {
    const original = doc(paragraph(text('strike', [{ type: 'strike' }])));
    expect(roundTrip(original)).toEqual(original);
  });

  it('link', () => {
    const original = doc(
      paragraph(
        text('link', [{ type: 'link', attrs: { href: 'https://example.com', title: null } }]),
      ),
    );
    const result = roundTrip(original);
    const linkMark = result.content![0].content![0].marks![0];
    // TipTap adds default link attrs (class, rel, target) — check core attrs
    expect(linkMark.type).toBe('link');
    expect(linkMark.attrs).toMatchObject({ href: 'https://example.com', title: null });
  });

  it('bold + italic', () => {
    const original = doc(paragraph(text('both', [{ type: 'bold' }, { type: 'italic' }])));
    expect(roundTrip(original)).toEqual(original);
  });

  // --- Complex compositions ---

  it('heading with bold text', () => {
    const original = doc(heading(2, text('bold title', [{ type: 'bold' }])));
    expect(roundTrip(original)).toEqual(original);
  });

  it('heading with mixed inline marks', () => {
    const original = doc(heading(1, text('bold', [{ type: 'bold' }]), text(' rest')));
    expect(roundTrip(original)).toEqual(original);
  });

  it('nested lists', () => {
    const original = doc({
      type: 'bulletList',
      content: [
        {
          type: 'listItem',
          content: [
            paragraph(text('outer')),
            {
              type: 'bulletList',
              content: [
                {
                  type: 'listItem',
                  content: [paragraph(text('inner'))],
                },
              ],
            },
          ],
        },
      ],
    } as JSONContent);
    expect(roundTrip(original)).toEqual(original);
  });

  it('blockquote containing a list', () => {
    const original = doc({
      type: 'blockquote',
      content: [
        {
          type: 'bulletList',
          content: [
            {
              type: 'listItem',
              content: [paragraph(text('item in quote'))],
            },
          ],
        },
      ],
    } as JSONContent);
    expect(roundTrip(original)).toEqual(original);
  });

  it('mixed marks in paragraph', () => {
    const original = doc(
      paragraph(
        text('bold', [{ type: 'bold' }]),
        text(' and '),
        text('italic', [{ type: 'italic' }]),
        text(' text'),
      ),
    );
    expect(roundTrip(original)).toEqual(original);
  });

  it('multiple block types in sequence', () => {
    const original = doc(
      heading(1, text('Title')),
      paragraph(text('intro paragraph')),
      {
        type: 'bulletList',
        content: [
          {
            type: 'listItem',
            content: [paragraph(text('item'))],
          },
        ],
      } as JSONContent,
      codeBlock('typescript', 'const x = 1;'),
    );
    expect(roundTrip(original)).toEqual(original);
  });

  it('deep nesting (three levels)', () => {
    const original = doc({
      type: 'bulletList',
      content: [
        {
          type: 'listItem',
          content: [
            paragraph(text('level 1')),
            {
              type: 'bulletList',
              content: [
                {
                  type: 'listItem',
                  content: [
                    paragraph(text('level 2')),
                    {
                      type: 'bulletList',
                      content: [
                        {
                          type: 'listItem',
                          content: [paragraph(text('level 3'))],
                        },
                      ],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    } as JSONContent);
    expect(roundTrip(original)).toEqual(original);
  });
});
