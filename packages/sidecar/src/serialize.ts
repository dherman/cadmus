/**
 * Serialize ProseMirror JSON → canonical markdown.
 *
 * Uses the shared schema extensions to ensure output matches
 * what the frontend editor produces.
 */

import { Editor } from '@tiptap/core';
import { createExtensions } from '@cadmus/doc-schema';
import type { JSONContent } from '@tiptap/core';

export function serialize(doc: JSONContent): string {
  // Create a headless editor instance with our shared schema
  const editor = new Editor({
    extensions: createExtensions({ disableHistory: true }),
    content: doc,
  });

  const markdown = editor.getMarkdown();
  editor.destroy();

  // ProseMirror uses non-breaking spaces (\u00a0 or &nbsp;) as cursor
  // placeholders in empty blocks. Strip them and remove resulting blank
  // trailing lines so they don't leak into exported markdown.
  return (
    markdown
      .replace(/&nbsp;/g, ' ')
      .replace(/\u00a0/g, ' ')
      .trimEnd() + '\n'
  );
}
