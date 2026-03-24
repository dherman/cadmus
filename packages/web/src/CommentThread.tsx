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

function isEdited(comment: Comment): boolean {
  return new Date(comment.updated_at).getTime() - new Date(comment.created_at).getTime() > 1000;
}

interface CommentThreadProps {
  root: Comment;
  replies: Comment[];
  onReply: (body: string) => Promise<void>;
  onResolve: () => Promise<void>;
  onUnresolve: () => Promise<void>;
  onEdit: (commentId: string, body: string) => Promise<void>;
  isActive: boolean;
  onThreadClick: () => void;
  currentUserId: string;
  canComment: boolean;
}

function CommentBody({
  comment,
  currentUserId,
  onEdit,
}: {
  comment: Comment;
  currentUserId: string;
  onEdit: (commentId: string, body: string) => Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [editBody, setEditBody] = useState(comment.body);
  const [saving, setSaving] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (editing && textareaRef.current) {
      textareaRef.current.focus();
      textareaRef.current.setSelectionRange(editBody.length, editBody.length);
    }
  }, [editing]);

  async function handleSave() {
    if (!editBody.trim() || editBody.trim() === comment.body) {
      setEditing(false);
      return;
    }
    setSaving(true);
    try {
      await onEdit(comment.id, editBody.trim());
      setEditing(false);
    } finally {
      setSaving(false);
    }
  }

  function handleCancel() {
    setEditBody(comment.body);
    setEditing(false);
  }

  const isOwner = comment.author.id === currentUserId;

  return (
    <div>
      <div className="comment-header">
        <span className="comment-author">{comment.author.display_name}</span>
        <span className="comment-time">{timeAgo(comment.created_at)}</span>
        {isEdited(comment) && <span className="comment-edited">(edited)</span>}
        {isOwner && !editing && (
          <button
            className="comment-edit-btn"
            onClick={(e) => {
              e.stopPropagation();
              setEditBody(comment.body);
              setEditing(true);
            }}
          >
            Edit
          </button>
        )}
      </div>
      {editing ? (
        <div className="comment-edit-form">
          <textarea
            ref={textareaRef}
            className="comment-edit-textarea"
            value={editBody}
            onChange={(e) => setEditBody(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                handleSave();
              }
              if (e.key === 'Escape') {
                e.preventDefault();
                handleCancel();
              }
            }}
          />
          <div className="comment-edit-actions">
            <button className="comment-action-btn" onClick={handleCancel}>
              Cancel
            </button>
            <button
              className="btn-primary comment-submit-btn"
              onClick={handleSave}
              disabled={saving || !editBody.trim()}
            >
              Save
            </button>
          </div>
        </div>
      ) : (
        <div className="comment-body">{comment.body}</div>
      )}
    </div>
  );
}

export function CommentThread({
  root,
  replies,
  onReply,
  onResolve,
  onUnresolve,
  onEdit,
  isActive,
  onThreadClick,
  currentUserId,
  canComment,
}: CommentThreadProps) {
  const [showReplyForm, setShowReplyForm] = useState(false);
  const [replyBody, setReplyBody] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const replyRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (showReplyForm && replyRef.current) {
      replyRef.current.focus();
    }
  }, [showReplyForm]);

  async function handleSubmitReply() {
    if (!replyBody.trim()) return;
    setSubmitting(true);
    try {
      await onReply(replyBody.trim());
      setReplyBody('');
      setShowReplyForm(false);
    } finally {
      setSubmitting(false);
    }
  }

  const isResolved = root.status === 'resolved';

  return (
    <div
      className={`comment-thread${isActive ? ' active' : ''}${isResolved ? ' resolved' : ''}`}
      onClick={onThreadClick}
    >
      <div className="comment-thread-root">
        <CommentBody comment={root} currentUserId={currentUserId} onEdit={onEdit} />
      </div>

      {replies.length > 0 && (
        <div className="comment-thread-replies">
          {replies.map((reply) => (
            <div key={reply.id} className="comment-reply">
              <CommentBody comment={reply} currentUserId={currentUserId} onEdit={onEdit} />
            </div>
          ))}
        </div>
      )}

      {canComment && (
        <div className="comment-thread-actions">
          <button
            className="comment-action-btn"
            onClick={(e) => {
              e.stopPropagation();
              setShowReplyForm(!showReplyForm);
              setReplyBody('');
            }}
          >
            Reply
          </button>
          {isResolved ? (
            <button
              className="comment-action-btn"
              onClick={(e) => {
                e.stopPropagation();
                onUnresolve();
              }}
            >
              Reopen
            </button>
          ) : (
            <button
              className="comment-action-btn"
              onClick={(e) => {
                e.stopPropagation();
                onResolve();
              }}
            >
              Resolve
            </button>
          )}
        </div>
      )}

      {showReplyForm && (
        <div className="comment-reply-form">
          <textarea
            ref={replyRef}
            value={replyBody}
            onChange={(e) => setReplyBody(e.target.value)}
            placeholder="Write a reply..."
            rows={2}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                handleSubmitReply();
              }
              if (e.key === 'Escape') {
                e.preventDefault();
                setShowReplyForm(false);
                setReplyBody('');
              }
            }}
          />
          <div className="comment-form-actions">
            <button
              className="comment-action-btn"
              onClick={() => {
                setShowReplyForm(false);
                setReplyBody('');
              }}
            >
              Cancel
            </button>
            <button
              className="btn-primary comment-submit-btn"
              onClick={handleSubmitReply}
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
