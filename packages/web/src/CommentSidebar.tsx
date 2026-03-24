import { useState, useRef, useEffect } from 'react';
import type { Comment } from './api';

function timeAgo(dateStr: string): string {
  const seconds = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000);
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

interface CommentSidebarProps {
  comments: Comment[];
  onCreateComment: (body: string, anchorFrom: number, anchorTo: number) => Promise<void>;
  onReply: (commentId: string, body: string) => Promise<void>;
  onResolve: (commentId: string) => Promise<void>;
  onUnresolve: (commentId: string) => Promise<void>;
  activeCommentId: string | null;
  onCommentClick: (commentId: string) => void;
  pendingAnchor: { from: number; to: number } | null;
  onCancelCreate: () => void;
  onClose: () => void;
  canComment: boolean;
}

export function CommentSidebar({
  comments,
  onCreateComment,
  onReply,
  onResolve,
  onUnresolve,
  activeCommentId,
  onCommentClick,
  pendingAnchor,
  onCancelCreate,
  onClose,
  canComment,
}: CommentSidebarProps) {
  const [newBody, setNewBody] = useState('');
  const [replyingTo, setReplyingTo] = useState<string | null>(null);
  const [replyBody, setReplyBody] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showResolved, setShowResolved] = useState(false);
  const activeRef = useRef<HTMLDivElement>(null);
  const createInputRef = useRef<HTMLTextAreaElement>(null);

  // Top-level comments only (no parent), sorted by created_at
  const topLevel = comments
    .filter((c) => c.parent_id === null)
    .sort((a, b) => a.created_at.localeCompare(b.created_at));

  const openComments = topLevel.filter((c) => c.status === 'open');
  const resolvedComments = topLevel.filter((c) => c.status === 'resolved');

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

  async function handleSubmitReply(commentId: string) {
    if (!replyBody.trim()) return;
    setSubmitting(true);
    try {
      await onReply(commentId, replyBody.trim());
      setReplyBody('');
      setReplyingTo(null);
    } finally {
      setSubmitting(false);
    }
  }

  function renderCommentCard(comment: Comment, isActive: boolean) {
    const replies = comments
      .filter((c) => c.parent_id === comment.id)
      .sort((a, b) => a.created_at.localeCompare(b.created_at));

    return (
      <div
        key={comment.id}
        ref={isActive ? activeRef : undefined}
        className={`comment-card${isActive ? ' active' : ''}`}
        onClick={() => onCommentClick(comment.id)}
      >
        <div className="comment-header">
          <span className="comment-author">{comment.author.display_name}</span>
          <span className="comment-time">{timeAgo(comment.created_at)}</span>
        </div>
        <div className="comment-body">{comment.body}</div>

        {/* Replies */}
        {replies.length > 0 && (
          <div className="comment-replies">
            {replies.map((reply) => (
              <div key={reply.id} className="comment-reply">
                <div className="comment-header">
                  <span className="comment-author">{reply.author.display_name}</span>
                  <span className="comment-time">{timeAgo(reply.created_at)}</span>
                </div>
                <div className="comment-body">{reply.body}</div>
              </div>
            ))}
          </div>
        )}

        {/* Actions */}
        {canComment && (
          <div className="comment-actions">
            <button
              className="comment-action-btn"
              onClick={(e) => {
                e.stopPropagation();
                setReplyingTo(replyingTo === comment.id ? null : comment.id);
                setReplyBody('');
              }}
            >
              Reply
            </button>
            {comment.status === 'open' ? (
              <button
                className="comment-action-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  onResolve(comment.id);
                }}
              >
                Resolve
              </button>
            ) : (
              <button
                className="comment-action-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  onUnresolve(comment.id);
                }}
              >
                Unresolve
              </button>
            )}
          </div>
        )}

        {/* Reply form */}
        {replyingTo === comment.id && (
          <div className="comment-reply-form">
            <textarea
              value={replyBody}
              onChange={(e) => setReplyBody(e.target.value)}
              placeholder="Write a reply..."
              rows={2}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                  handleSubmitReply(comment.id);
                }
              }}
            />
            <div className="comment-form-actions">
              <button
                className="comment-action-btn"
                onClick={() => {
                  setReplyingTo(null);
                  setReplyBody('');
                }}
              >
                Cancel
              </button>
              <button
                className="btn-primary comment-submit-btn"
                onClick={() => handleSubmitReply(comment.id)}
                disabled={submitting || !replyBody.trim()}
              >
                Reply
              </button>
            </div>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="comment-sidebar">
      <div className="comment-sidebar-header">
        <span className="comment-sidebar-title">Comments ({openComments.length})</span>
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
                handleSubmitCreate();
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

      {/* Open comments */}
      <div className="comment-list">
        {openComments.map((c) => renderCommentCard(c, c.id === activeCommentId))}
      </div>

      {/* Resolved section */}
      {resolvedComments.length > 0 && (
        <div className="comment-resolved-section">
          <button
            className="comment-resolved-toggle"
            onClick={() => setShowResolved(!showResolved)}
          >
            {showResolved ? '▾' : '▸'} Resolved ({resolvedComments.length})
          </button>
          {showResolved && (
            <div className="comment-list">
              {resolvedComments.map((c) => renderCommentCard(c, c.id === activeCommentId))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
