import { useEffect, useState } from 'react';
import {
  listPermissions,
  addPermission,
  updatePermission,
  removePermission,
  PermissionEntry,
} from './api';

interface ShareDialogProps {
  docId: string;
  docTitle: string;
  onClose: () => void;
}

export function ShareDialog({ docId, docTitle, onClose }: ShareDialogProps) {
  const [permissions, setPermissions] = useState<PermissionEntry[]>([]);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState('comment');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  async function loadPermissions() {
    try {
      const perms = await listPermissions(docId);
      setPermissions(perms);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load permissions');
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadPermissions();
  }, [docId]);

  async function handleInvite(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const email = inviteEmail.trim();
    if (!email) return;

    try {
      await addPermission(docId, email, inviteRole);
      setInviteEmail('');
      await loadPermissions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to invite user');
    }
  }

  async function handleRoleChange(userId: string, newRole: string, previousRole: string) {
    setError(null);
    // Optimistically update
    setPermissions((prev) => prev.map((p) => (p.user_id === userId ? { ...p, role: newRole } : p)));
    try {
      await updatePermission(docId, userId, newRole);
    } catch (err) {
      // Revert on error
      setPermissions((prev) =>
        prev.map((p) => (p.user_id === userId ? { ...p, role: previousRole } : p)),
      );
      setError(err instanceof Error ? err.message : 'Failed to update role');
    }
  }

  async function handleRemove(userId: string) {
    setError(null);
    try {
      await removePermission(docId, userId);
      setPermissions((prev) => prev.filter((p) => p.user_id !== userId));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to remove user');
    }
  }

  return (
    <div className="share-dialog-overlay" onClick={onClose}>
      <div className="share-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="share-dialog-header">
          <h2>Share &ldquo;{docTitle}&rdquo;</h2>
          <button className="share-dialog-close" onClick={onClose}>
            &times;
          </button>
        </div>

        {error && <div className="share-dialog-error">{error}</div>}

        <div className="share-dialog-list">
          {loading ? (
            <p className="share-dialog-loading">Loading...</p>
          ) : (
            permissions.map((perm) => (
              <div key={perm.user_id} className="share-dialog-row">
                <div className="share-dialog-user">
                  <span className="share-dialog-name">{perm.display_name || perm.email}</span>
                  <span className="share-dialog-email">{perm.email}</span>
                </div>
                {perm.is_owner ? (
                  <span className="share-dialog-owner-badge">Owner</span>
                ) : (
                  <div className="share-dialog-actions">
                    <select
                      value={perm.role}
                      onChange={(e) => handleRoleChange(perm.user_id, e.target.value, perm.role)}
                      className="share-dialog-select"
                    >
                      <option value="read">Viewer</option>
                      <option value="comment">Commenter</option>
                      <option value="edit">Editor</option>
                    </select>
                    <button
                      className="share-dialog-remove"
                      onClick={() => handleRemove(perm.user_id)}
                    >
                      Remove
                    </button>
                  </div>
                )}
              </div>
            ))
          )}
        </div>

        <form className="share-dialog-invite" onSubmit={handleInvite}>
          <input
            type="email"
            placeholder="email@example.com"
            value={inviteEmail}
            onChange={(e) => setInviteEmail(e.target.value)}
            className="share-dialog-input"
          />
          <select
            value={inviteRole}
            onChange={(e) => setInviteRole(e.target.value)}
            className="share-dialog-select"
          >
            <option value="read">Viewer</option>
            <option value="comment">Commenter</option>
            <option value="edit">Editor</option>
          </select>
          <button type="submit" className="btn-primary share-dialog-invite-btn">
            Invite
          </button>
        </form>
      </div>
    </div>
  );
}
