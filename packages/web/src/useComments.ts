import { useState, useEffect, useCallback } from 'react';
import {
  listComments,
  createComment,
  replyToComment,
  resolveComment,
  unresolveComment,
  editComment,
} from './api';
import type { Comment } from './api';
import type { WebsocketProvider } from 'y-websocket';
import * as decoding from 'lib0/decoding';

const COMMENT_EVENT_TAG = 100;

/**
 * Register a custom message handler on the y-websocket provider to receive
 * comment events broadcast by the server as y-sync Custom(100, json) messages.
 */
function installCommentHandler(
  provider: WebsocketProvider,
  onEvent: (event: { type: string; comment: Comment }) => void,
) {
  // y-websocket reads the first var-uint from each binary message as
  // `messageType` and dispatches to `provider.messageHandlers[messageType]`.
  // The Rust server encodes comment events as Message::Custom(100, json_bytes),
  // which on the wire is: [tag=100 (varint)] [length-prefixed json bytes].
  // We register a handler at index 100 to decode the remaining payload.
  const handlers = (provider as unknown as { messageHandlers: unknown[] }).messageHandlers;
  handlers[COMMENT_EVENT_TAG] = (_encoder: unknown, decoder: { arr: Uint8Array; pos: number }) => {
    const buf = decoding.readVarUint8Array(decoder);
    const json = new TextDecoder().decode(buf);
    const event = JSON.parse(json);
    onEvent(event);
  };
}

function uninstallCommentHandler(provider: WebsocketProvider) {
  const handlers = (provider as unknown as { messageHandlers: unknown[] }).messageHandlers;
  delete handlers[COMMENT_EVENT_TAG];
}

export function useComments(docId: string, wsProvider: WebsocketProvider | null) {
  const [comments, setComments] = useState<Comment[]>([]);
  const [loading, setLoading] = useState(true);

  // Initial fetch
  useEffect(() => {
    setLoading(true);
    listComments(docId, 'all')
      .then(setComments)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [docId]);

  // WebSocket event listener
  useEffect(() => {
    if (!wsProvider) return;

    const onEvent = (event: { type: string; comment: Comment }) => {
      const comment = event.comment;

      setComments((prev) => {
        switch (event.type) {
          case 'created':
          case 'replied':
            // Add if not already present (dedup with optimistic update)
            if (prev.some((c) => c.id === comment.id)) return prev;
            return [...prev, comment];
          case 'updated':
          case 'resolved':
          case 'unresolved':
            return prev.map((c) => (c.id === comment.id ? comment : c));
          default:
            return prev;
        }
      });
    };

    installCommentHandler(wsProvider, onEvent);
    return () => {
      uninstallCommentHandler(wsProvider);
    };
  }, [wsProvider]);

  // Mutation wrappers
  const handleCreate = useCallback(
    async (body: string, anchorFrom: number, anchorTo: number) => {
      const comment = await createComment(docId, body, anchorFrom, anchorTo);
      setComments((prev) => (prev.some((c) => c.id === comment.id) ? prev : [...prev, comment]));
    },
    [docId],
  );

  const handleReply = useCallback(
    async (commentId: string, body: string) => {
      const reply = await replyToComment(docId, commentId, body);
      setComments((prev) => (prev.some((c) => c.id === reply.id) ? prev : [...prev, reply]));
    },
    [docId],
  );

  const handleResolve = useCallback(
    async (commentId: string) => {
      const updated = await resolveComment(docId, commentId);
      setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
    },
    [docId],
  );

  const handleUnresolve = useCallback(
    async (commentId: string) => {
      const updated = await unresolveComment(docId, commentId);
      setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
    },
    [docId],
  );

  const handleEdit = useCallback(
    async (commentId: string, body: string) => {
      const updated = await editComment(docId, commentId, body);
      setComments((prev) => prev.map((c) => (c.id === commentId ? updated : c)));
    },
    [docId],
  );

  return {
    comments,
    loading,
    createComment: handleCreate,
    replyToComment: handleReply,
    resolveComment: handleResolve,
    unresolveComment: handleUnresolve,
    editComment: handleEdit,
  };
}
