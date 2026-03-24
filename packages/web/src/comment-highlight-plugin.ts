import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { Comment } from './api';

export const commentHighlightKey = new PluginKey('commentHighlight');

/**
 * Transaction metadata key used to pass new comment data into the plugin.
 */
export const COMMENT_HIGHLIGHT_META = 'commentHighlightUpdate';

function buildDecorations(
  doc: Parameters<typeof DecorationSet.create>[0],
  comments: Comment[],
  activeCommentId: string | null,
): DecorationSet {
  const decorations: Decoration[] = [];
  for (const c of comments) {
    if (c.anchor_from == null || c.anchor_to == null) continue;
    if (c.status !== 'open') continue;
    // Clamp positions to document size
    const from = Math.max(0, Math.min(c.anchor_from, doc.content.size));
    const to = Math.max(from, Math.min(c.anchor_to, doc.content.size));
    if (from === to) continue;
    const isActive = c.id === activeCommentId;
    decorations.push(
      Decoration.inline(from, to, {
        class: isActive ? 'comment-highlight active' : 'comment-highlight',
        'data-comment-id': c.id,
      }),
    );
  }
  return DecorationSet.create(doc, decorations);
}

export interface CommentHighlightState {
  comments: Comment[];
  activeCommentId: string | null;
}

export const CommentHighlightExtension = Extension.create({
  name: 'commentHighlight',

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: commentHighlightKey,
        state: {
          init() {
            return {
              decorations: DecorationSet.empty,
              comments: [] as Comment[],
              activeCommentId: null as string | null,
            };
          },
          apply(tr, prev) {
            const meta = tr.getMeta(COMMENT_HIGHLIGHT_META) as CommentHighlightState | undefined;
            if (meta) {
              return {
                comments: meta.comments,
                activeCommentId: meta.activeCommentId,
                decorations: buildDecorations(tr.doc, meta.comments, meta.activeCommentId),
              };
            }
            // If document changed, map decorations
            if (tr.docChanged) {
              return {
                ...prev,
                decorations: buildDecorations(tr.doc, prev.comments, prev.activeCommentId),
              };
            }
            return prev;
          },
        },
        props: {
          decorations(state) {
            return (this as unknown as Plugin).getState(state)?.decorations ?? DecorationSet.empty;
          },
          handleClick(view, pos) {
            // Find if click was on a comment highlight decoration
            const pluginState = commentHighlightKey.getState(view.state);
            if (!pluginState) return false;
            const decos = pluginState.decorations.find(pos, pos);
            for (const deco of decos) {
              const commentId = (deco as unknown as { type: { attrs: Record<string, string> } })
                .type.attrs['data-comment-id'];
              if (commentId) {
                // Dispatch a custom event so the sidebar can react
                view.dom.dispatchEvent(
                  new CustomEvent('comment-highlight-click', { detail: { commentId } }),
                );
                return false; // Don't consume the click
              }
            }
            return false;
          },
        },
      }),
    ];
  },
});
