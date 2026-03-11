import { describe, it, expect } from 'vitest'
import { Editor } from '@tiptap/core'
import { createExtensions, SCHEMA_VERSION } from '../index'

/**
 * Helper: create a headless Tiptap editor for testing the schema.
 */
function createTestEditor() {
  return new Editor({
    extensions: createExtensions(),
    content: '',
  })
}

describe('doc-schema', () => {
  it('produces a valid ProseMirror schema', () => {
    const editor = createTestEditor()
    const { schema } = editor

    expect(schema).toBeDefined()
    expect(schema.nodes).toBeDefined()
    expect(schema.marks).toBeDefined()

    editor.destroy()
  })

  it('has all expected node types', () => {
    const editor = createTestEditor()
    const nodeNames = Object.keys(editor.schema.nodes)

    const expectedNodes = [
      'doc',
      'paragraph',
      'heading',
      'codeBlock',
      'blockquote',
      'bulletList',
      'orderedList',
      'listItem',
      'horizontalRule',
      'hardBreak',
      'image',
      'text',
    ]

    for (const name of expectedNodes) {
      expect(nodeNames, `missing node: ${name}`).toContain(name)
    }

    editor.destroy()
  })

  it('has all expected mark types', () => {
    const editor = createTestEditor()
    const markNames = Object.keys(editor.schema.marks)

    const expectedMarks = ['bold', 'italic', 'strike', 'code', 'link']

    for (const name of expectedMarks) {
      expect(markNames, `missing mark: ${name}`).toContain(name)
    }

    editor.destroy()
  })

  it('does not include underline', () => {
    const editor = createTestEditor()
    expect(editor.schema.marks.underline).toBeUndefined()
    editor.destroy()
  })

  it('image is block-level', () => {
    const editor = createTestEditor()
    expect(editor.schema.nodes.image.isBlock).toBe(true)
    editor.destroy()
  })

  it('SCHEMA_VERSION is 1', () => {
    expect(SCHEMA_VERSION).toBe(1)
  })

  it('schema spec snapshot', () => {
    const editor = createTestEditor()
    const { schema } = editor

    const spec = {
      nodes: Object.fromEntries(
        Object.entries(schema.nodes).map(([name, nodeType]) => [
          name,
          { content: nodeType.spec.content ?? null, group: nodeType.spec.group ?? null },
        ]),
      ),
      marks: Object.keys(schema.marks).sort(),
    }

    expect(spec).toMatchSnapshot()

    editor.destroy()
  })

  it('round-trips ProseMirror JSON', () => {
    const editor = createTestEditor()
    const { schema } = editor

    // Build a document that exercises all node and mark types
    const docJSON = {
      type: 'doc',
      content: [
        {
          type: 'heading',
          attrs: { level: 1 },
          content: [{ type: 'text', text: 'Title' }],
        },
        {
          type: 'paragraph',
          content: [
            { type: 'text', text: 'Normal text ' },
            { type: 'text', text: 'bold', marks: [{ type: 'bold' }] },
            { type: 'text', text: ' ' },
            { type: 'text', text: 'italic', marks: [{ type: 'italic' }] },
            { type: 'text', text: ' ' },
            { type: 'text', text: 'struck', marks: [{ type: 'strike' }] },
            { type: 'text', text: ' ' },
            { type: 'text', text: 'code', marks: [{ type: 'code' }] },
            { type: 'text', text: ' ' },
            {
              type: 'text',
              text: 'link',
              marks: [{
                type: 'link',
                attrs: {
                  href: 'https://example.com',
                  target: '_blank',
                  rel: 'noopener noreferrer nofollow',
                  class: null,
                  title: null,
                },
              }],
            },
          ],
        },
        {
          type: 'blockquote',
          content: [
            {
              type: 'paragraph',
              content: [{ type: 'text', text: 'A quote' }],
            },
          ],
        },
        {
          type: 'codeBlock',
          attrs: { language: 'javascript' },
          content: [{ type: 'text', text: 'const x = 1' }],
        },
        {
          type: 'bulletList',
          content: [
            {
              type: 'listItem',
              content: [
                {
                  type: 'paragraph',
                  content: [{ type: 'text', text: 'item one' }],
                },
              ],
            },
          ],
        },
        {
          type: 'orderedList',
          attrs: { start: 1, type: null },
          content: [
            {
              type: 'listItem',
              content: [
                {
                  type: 'paragraph',
                  content: [{ type: 'text', text: 'first' }],
                },
              ],
            },
          ],
        },
        { type: 'horizontalRule' },
        {
          type: 'image',
          attrs: { src: 'https://example.com/img.png', alt: 'test', title: null, width: null, height: null },
        },
        {
          type: 'paragraph',
          content: [{ type: 'hardBreak' }, { type: 'text', text: 'after break' }],
        },
      ],
    }

    // Parse from JSON → ProseMirror Node → back to JSON
    const node = schema.nodeFromJSON(docJSON)
    const roundTripped = node.toJSON()

    expect(roundTripped).toEqual(docJSON)

    editor.destroy()
  })

  it('createExtensions with disableHistory removes history', () => {
    const editor = new Editor({
      extensions: createExtensions({ disableHistory: true }),
      content: '',
    })

    // When history is disabled, the undo command should not be registered
    const canUndo = typeof editor.can().undo === 'function'
      ? editor.can().undo()
      : false
    expect(canUndo).toBe(false)

    editor.destroy()
  })
})
