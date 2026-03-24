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

  // Commenters need the editor to be "editable" so they can select text for the
  // BubbleMenu, but they shouldn't be able to modify content. We set editable=true
  // for commenters and rely on the Collaboration extension + server authority to
  // prevent actual edits. A ProseMirror transaction filter blocks local edits for
  // non-editors as an extra safeguard.
  const proseMirrorEditable = editable || canComment;

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
    editable: proseMirrorEditable,
  });

  // Block DOM-level input for comment-only users. The editor is set to
  // editable (so text selection and BubbleMenu work), but we prevent typing,
  // pasting, and dropping by intercepting beforeinput/paste/drop events.
  // Programmatic transactions (y-sync, plugin meta, selections) are unaffected.
  useEffect(() => {
    if (!editor || editor.isDestroyed || editable) return;
    const dom = editor.view.dom;
    const blockInput = (e: Event) => e.preventDefault();
    dom.addEventListener('beforeinput', blockInput);
    dom.addEventListener('paste', blockInput);
    dom.addEventListener('drop', blockInput);
    return () => {
      dom.removeEventListener('beforeinput', blockInput);
      dom.removeEventListener('paste', blockInput);
      dom.removeEventListener('drop', blockInput);
    };
  }, [editor, editable]);

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
      ) : canComment ? (
        <div className="read-only-banner">
          Comment only — you can comment on this document but not edit it
        </div>
      ) : (
        <div className="read-only-banner">
          Read only — you can view this document but not edit it
        </div>
      )}
      <EditorContent editor={editor} />
      {showBubble && (
        <BubbleMenu editor={editor}>
          <button className="add-comment-button" onClick={handleAddComment}>
            Comment
          </button>
        </BubbleMenu>
      )}
    </div>
  );
}
