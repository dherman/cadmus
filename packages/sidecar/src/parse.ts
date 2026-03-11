/**
 * Parse markdown → ProseMirror JSON.
 *
 * Uses the shared schema extensions to ensure parsing matches
 * the frontend editor's behavior.
 */

import { Editor } from '@tiptap/core'
import { createExtensions } from '@cadmus/doc-schema'
import type { JSONContent } from '@tiptap/core'

export function parse(markdown: string): JSONContent {
  const editor = new Editor({
    extensions: createExtensions({ disableHistory: true }),
    content: markdown,
    // @ts-expect-error — contentType is available when Markdown extension is loaded
    contentType: 'markdown',
  })

  const json = editor.getJSON()
  editor.destroy()

  return json
}
