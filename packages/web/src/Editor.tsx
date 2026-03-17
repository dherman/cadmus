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
  editable?: boolean;
}

export function Editor({ ydoc, provider, user, editable = true }: EditorProps) {
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
    editable,
  });

  if (!editor) return null;

  return (
    <div className="editor-wrapper">
      {editable ? (
        <Toolbar editor={editor} />
      ) : (
        <div className="read-only-banner">
          Read only — you can view this document but not edit it
        </div>
      )}
      <EditorContent editor={editor} />
    </div>
  );
}
