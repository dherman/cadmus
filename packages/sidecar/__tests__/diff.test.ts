import { describe, it, expect } from 'vitest';
import { diff } from '../src/diff';

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

  it('returns serializable step objects', () => {
    const old = doc(paragraph(text('hello')));
    const updated = doc(paragraph(text('hello world')));
    const steps = diff(old, updated);
    expect(steps.length).toBeGreaterThan(0);
    // Each step should be a plain object with a stepType field
    for (const step of steps) {
      expect(step).toBeTypeOf('object');
      expect(step).toHaveProperty('stepType');
    }
  });
});
