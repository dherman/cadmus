import { useMemo, useEffect, useCallback } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import { BubbleMenu } from '@tiptap/react/menus';
import { Collaboration } from '@tiptap/extension-collaboration';
import { CollaborationCursor } from './collaboration-cursor-extension';
import { createExtensions } from '@cadmus/doc-schema';
import { getUserIdentity } from './user-identity';
import { Toolbar } from './Toolbar';
import { CommentHighlightExtension, COMMENT_HIGHLIGHT_META } from './comment-highlight-plugin';
import type * as Y from 'yjs';
import type { WebsocketProvider } from 'y-websocket';
import type { UserProfile, Comment } from './api';

interface EditorProps {
  ydoc: Y.Doc;
  provider: WebsocketProvider;
  user: UserProfile | null;
  editable?: boolean;
  canComment?: boolean;
  comments?: Comment[];
  activeCommentId?: string | null;
  onAddComment?: (from: number, to: number) => void;
  onHighlightClick?: (commentId: string) => void;
}

export function Editor({
  ydoc,
  provider,
  user,
  editable = true,
  canComment = false,
  comments = [],
  activeCommentId = null,
  onAddComment,
  onHighlightClick,
}: EditorProps) {
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
      CommentHighlightExtension,
    ],
    editable,
  });

  // Push comment data into the plugin whenever comments or activeCommentId changes
  useEffect(() => {
    if (!editor || editor.isDestroyed || !editor.view?.dom) return;
    const tr = editor.state.tr.setMeta(COMMENT_HIGHLIGHT_META, {
      comments,
      activeCommentId,
    });
    editor.view.dispatch(tr);
  }, [editor, comments, activeCommentId]);

  // Listen for highlight click events from the plugin
  useEffect(() => {
    if (!editor || editor.isDestroyed || !editor.view?.dom || !onHighlightClick) return;
    const dom = editor.view.dom;
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail?.commentId) {
        onHighlightClick(detail.commentId);
      }
    };
    dom.addEventListener('comment-highlight-click', handler);
    return () => dom.removeEventListener('comment-highlight-click', handler);
  }, [editor, onHighlightClick]);

  const handleAddComment = useCallback(() => {
    if (!editor || !onAddComment) return;
    const { from, to } = editor.state.selection;
    if (from === to) return;
    onAddComment(from, to);
    // Collapse selection so the BubbleMenu disappears
    editor.commands.setTextSelection(to);
  }, [editor, onAddComment]);

  if (!editor) return null;

  const showBubble = canComment && onAddComment;

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
      {showBubble && (
        <BubbleMenu editor={editor} tippyOptions={{ placement: 'top', duration: 150 }}>
          <button className="add-comment-button" onClick={handleAddComment}>
            Comment
          </button>
        </BubbleMenu>
      )}
    </div>
  );
}
