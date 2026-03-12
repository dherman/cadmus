import { useEditor, EditorContent } from '@tiptap/react';
import { createExtensions } from '@cadmus/doc-schema';
import { sampleDocument } from './fixtures/sample-document';

export function Editor() {
  const editor = useEditor({
    extensions: createExtensions(),
    content: sampleDocument,
  });

  if (!editor) return null;

  return (
    <div className="editor-wrapper">
      <EditorContent editor={editor} />
    </div>
  );
}
