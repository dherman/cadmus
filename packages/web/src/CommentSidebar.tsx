import { useState, useRef, useEffect } from 'react';
import { CommentThread } from './CommentThread';
import type { Comment } from './api';

interface ThreadGroup {
  root: Comment;
  replies: Comment[];
}

function groupIntoThreads(comments: Comment[]): ThreadGroup[] {
  const roots = comments.filter((c) => c.parent_id === null);
  const replyMap = new Map<string, Comment[]>();

  for (const c of comments) {
    if (c.parent_id) {
      const existing = replyMap.get(c.parent_id) || [];
      existing.push(c);
      replyMap.set(c.parent_id, existing);
    }
  }

  return roots.map((root) => ({
    root,
    replies: (replyMap.get(root.id) || []).sort(
      (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
    ),
  }));
}

interface CommentSidebarProps {
  comments: Comment[];
  onCreateComment: (body: string, anchorFrom: number, anchorTo: number) => Promise<void>;
  onReply: (commentId: string, body: string) => Promise<void>;
  onResolve: (commentId: string) => Promise<void>;
  onUnresolve: (commentId: string) => Promise<void>;
  onEdit: (commentId: string, body: string) => Promise<void>;
  activeCommentId: string | null;
  onCommentClick: (commentId: string) => void;
  pendingAnchor: { from: number; to: number } | null;
  onCancelCreate: () => void;
  onClose: () => void;
  canComment: boolean;
  currentUserId: string;
}

export function CommentSidebar({
  comments,
  onCreateComment,
  onReply,
  onResolve,
  onUnresolve,
  onEdit,
  activeCommentId,
  onCommentClick,
  pendingAnchor,
  onCancelCreate,
  onClose,
  canComment,
  currentUserId,
}: CommentSidebarProps) {
  const [newBody, setNewBody] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showResolved, setShowResolved] = useState(false);
  const activeRef = useRef<HTMLDivElement>(null);
  const createInputRef = useRef<HTMLTextAreaElement>(null);

  const threads = groupIntoThreads(comments);
  const openThreads = threads
    .filter((t) => t.root.status === 'open')
    .sort((a, b) => new Date(a.root.created_at).getTime() - new Date(b.root.created_at).getTime());
  const resolvedThreads = threads.filter((t) => t.root.status === 'resolved');

  // Scroll to active comment when it changes
  useEffect(() => {
    if (activeCommentId && activeRef.current) {
      activeRef.current.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  }, [activeCommentId]);

  // Focus create input when pendingAnchor appears
  useEffect(() => {
    if (pendingAnchor && createInputRef.current) {
      createInputRef.current.focus();
    }
  }, [pendingAnchor]);

  async function handleSubmitCreate() {
    if (!pendingAnchor || !newBody.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      await onCreateComment(newBody.trim(), pendingAnchor.from, pendingAnchor.to);
      setNewBody('');
      onCancelCreate();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create comment');
    } finally {
      setSubmitting(false);
    }
  }

  function renderThread(thread: ThreadGroup) {
    const isActive = thread.root.id === activeCommentId;
    return (
      <div key={thread.root.id} ref={isActive ? activeRef : undefined}>
        <CommentThread
          root={thread.root}
          replies={thread.replies}
          onReply={(body) => onReply(thread.root.id, body)}
          onResolve={() => onResolve(thread.root.id)}
          onUnresolve={() => onUnresolve(thread.root.id)}
          onEdit={onEdit}
          isActive={isActive}
          onThreadClick={() => onCommentClick(thread.root.id)}
          currentUserId={currentUserId}
          canComment={canComment}
        />
      </div>
    );
  }

  return (
    <div className="comment-sidebar">
      <div className="comment-sidebar-header">
        <span className="comment-sidebar-title">Comments ({openThreads.length})</span>
        <button className="comment-sidebar-close" onClick={onClose}>
          &times;
        </button>
      </div>

      {/* Create comment form */}
      {pendingAnchor && canComment && (
        <div className="comment-create-form">
          <textarea
            ref={createInputRef}
            value={newBody}
            onChange={(e) => setNewBody(e.target.value)}
            placeholder="Add a comment..."
            rows={3}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                handleSubmitCreate();
              }
              if (e.key === 'Escape') {
                e.preventDefault();
                onCancelCreate();
              }
            }}
          />
          {error && <div className="comment-error">{error}</div>}
          <div className="comment-form-actions">
            <button className="comment-action-btn" onClick={onCancelCreate}>
              Cancel
            </button>
            <button
              className="btn-primary comment-submit-btn"
              onClick={handleSubmitCreate}
              disabled={submitting || !newBody.trim()}
            >
              Comment
            </button>
          </div>
        </div>
      )}

      {/* Open threads */}
      <div className="comment-list">{openThreads.map((t) => renderThread(t))}</div>

      {/* Resolved section */}
      {resolvedThreads.length > 0 && (
        <div className="comment-resolved-section">
          <button
            className="comment-resolved-toggle"
            onClick={() => setShowResolved(!showResolved)}
          >
            {showResolved ? '\u25be' : '\u25b8'} Resolved ({resolvedThreads.length})
          </button>
          {showResolved && (
            <div className="comment-list">{resolvedThreads.map((t) => renderThread(t))}</div>
          )}
        </div>
      )}
    </div>
  );
}
