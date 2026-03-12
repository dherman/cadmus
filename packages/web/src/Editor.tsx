import { useEditor, EditorContent } from '@tiptap/react';
import { Collaboration } from '@tiptap/extension-collaboration';
import { createExtensions } from '@cadmus/doc-schema';
import { Toolbar } from './Toolbar';
import type * as Y from 'yjs';
import type { WebsocketProvider } from 'y-websocket';

interface EditorProps {
  ydoc: Y.Doc;
  provider: WebsocketProvider;
}

export function Editor({ ydoc, provider: _provider }: EditorProps) {
  const editor = useEditor({
    extensions: [
      ...createExtensions({ disableHistory: true }),
      Collaboration.configure({ document: ydoc }),
    ],
  });

  if (!editor) return null;

  return (
    <div className="editor-wrapper">
      <Toolbar editor={editor} />
      <EditorContent editor={editor} />
    </div>
  );
}
