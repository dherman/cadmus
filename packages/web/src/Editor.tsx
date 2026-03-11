/**
 * Cadmus Editor Component
 *
 * This is the main editor component. It sets up:
 * - Tiptap editor with the shared schema extensions
 * - Yjs document + WebSocket provider for real-time sync
 * - y-prosemirror binding between Yjs and ProseMirror
 * - Awareness for cursor/presence rendering
 */

import { useEditor, EditorContent } from '@tiptap/react'
import { createExtensions } from '@cadmus/doc-schema'
import * as Y from 'yjs'
import { WebsocketProvider } from 'y-websocket'
// import { yProsemirror, yCursorPlugin, ySyncPlugin, yUndoPlugin } from 'y-prosemirror'
import { useEffect, useMemo } from 'react'

interface EditorProps {
  documentId: string
  wsUrl: string
  token: string
  userName: string
  userColor: string
}

export function CollabEditor({ documentId, wsUrl, token, userName, userColor }: EditorProps) {
  // Create Yjs document and WebSocket provider
  const ydoc = useMemo(() => new Y.Doc(), [])

  useEffect(() => {
    const provider = new WebsocketProvider(
      wsUrl,
      documentId,
      ydoc,
      { params: { token } }
    )

    // Set local awareness state
    provider.awareness.setLocalState({
      user: {
        name: userName,
        color: userColor,
      },
    })

    return () => {
      provider.destroy()
    }
  }, [documentId, wsUrl, token, ydoc, userName, userColor])

  // Create Tiptap editor with shared schema + collaboration extensions
  const editor = useEditor({
    extensions: [
      ...createExtensions({ disableHistory: true }),
      // TODO: Add y-prosemirror collaboration extension
      // Collaboration.configure({ document: ydoc }),
      // CollaborationCursor.configure({ provider, user: { name, color } }),
    ],
    content: '',
  })

  return (
    <div className="cadmus-editor">
      {/* TODO: Toolbar component */}
      <EditorContent editor={editor} />
      {/* TODO: Comment sidebar */}
    </div>
  )
}
