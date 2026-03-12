import { useEditor, EditorContent } from '@tiptap/react';
import { Collaboration } from '@tiptap/extension-collaboration';
import { CollaborationCursor } from './collaboration-cursor-extension';
import { createExtensions } from '@cadmus/doc-schema';
import { getOrCreateUserIdentity } from './user-identity';
import { Toolbar } from './Toolbar';
import type * as Y from 'yjs';
import type { WebsocketProvider } from 'y-websocket';

const identity = getOrCreateUserIdentity();

interface EditorProps {
  ydoc: Y.Doc;
  provider: WebsocketProvider;
}

export function Editor({ ydoc, provider }: EditorProps) {
  const editor = useEditor({
    extensions: [
      ...createExtensions({ disableHistory: true }),
      Collaboration.configure({ document: ydoc }),
      CollaborationCursor.configure({
        awareness: provider.awareness,
        user: identity,
      }),
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
