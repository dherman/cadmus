import { useMemo } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import { Collaboration } from '@tiptap/extension-collaboration';
import { CollaborationCursor } from './collaboration-cursor-extension';
import { createExtensions } from '@cadmus/doc-schema';
import { getUserIdentity } from './user-identity';
import { Toolbar } from './Toolbar';
import type * as Y from 'yjs';
import type { WebsocketProvider } from 'y-websocket';
import type { UserProfile } from './api';

interface EditorProps {
  ydoc: Y.Doc;
  provider: WebsocketProvider;
  user: UserProfile | null;
}

export function Editor({ ydoc, provider, user }: EditorProps) {
  const identity = useMemo(
    () => (user ? getUserIdentity(user) : { name: 'Anonymous', color: '#888888' }),
    [user],
  );

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
