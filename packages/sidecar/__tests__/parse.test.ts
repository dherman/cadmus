import { describe, it, expect } from 'vitest';
import { parse } from '../src/parse';

describe('parse', () => {
  it('parses a paragraph', () => {
    const json = parse('Hello world\n');
    expect(json.type).toBe('doc');
    expect(json.content![0].type).toBe('paragraph');
  });

  it('parses ATX headings', () => {
    for (let level = 1; level <= 6; level++) {
      const json = parse('#'.repeat(level) + ' Title\n');
      expect(json.content![0].type).toBe('heading');
      expect(json.content![0].attrs!.level).toBe(level);
    }
  });

  it('parses bold **text**', () => {
    const json = parse('**bold**\n');
    const textNode = json.content![0].content![0];
    expect(textNode.marks?.some((m: { type: string }) => m.type === 'bold')).toBe(true);
  });

  it('parses italic *text*', () => {
    const json = parse('*italic*\n');
    const textNode = json.content![0].content![0];
    expect(textNode.marks?.some((m: { type: string }) => m.type === 'italic')).toBe(true);
  });

  it('parses inline code `text`', () => {
    const json = parse('`code`\n');
    const textNode = json.content![0].content![0];
    expect(textNode.marks?.some((m: { type: string }) => m.type === 'code')).toBe(true);
  });

  it('parses strikethrough ~~text~~', () => {
    const json = parse('~~strike~~\n');
    const textNode = json.content![0].content![0];
    expect(textNode.marks?.some((m: { type: string }) => m.type === 'strike')).toBe(true);
  });

  it('parses fenced code block with language', () => {
    const json = parse('```rust\nfn main() {}\n```\n');
    expect(json.content![0].type).toBe('codeBlock');
    expect(json.content![0].attrs?.language).toBe('rust');
  });

  it('parses fenced code block without language', () => {
    const json = parse('```\nplain code\n```\n');
    expect(json.content![0].type).toBe('codeBlock');
  });

  it('parses bullet list', () => {
    const json = parse('- first\n- second\n');
    expect(json.content![0].type).toBe('bulletList');
    expect(json.content![0].content).toHaveLength(2);
  });

  it('parses ordered list', () => {
    const json = parse('1. first\n2. second\n');
    expect(json.content![0].type).toBe('orderedList');
  });

  it('parses links', () => {
    const json = parse('[text](https://example.com)\n');
    const textNode = json.content![0].content![0];
    const linkMark = textNode.marks?.find((m: { type: string }) => m.type === 'link');
    expect(linkMark?.attrs?.href).toBe('https://example.com');
  });

  it('parses blockquote', () => {
    const json = parse('> quoted text\n');
    expect(json.content![0].type).toBe('blockquote');
  });

  it('parses horizontal rule', () => {
    const json = parse('---\n');
    expect(json.content![0].type).toBe('horizontalRule');
  });

  it('parses bold + italic ***text***', () => {
    const json = parse('***both***\n');
    const textNode = json.content![0].content![0];
    expect(textNode.marks?.some((m: { type: string }) => m.type === 'bold')).toBe(true);
    expect(textNode.marks?.some((m: { type: string }) => m.type === 'italic')).toBe(true);
  });
});
