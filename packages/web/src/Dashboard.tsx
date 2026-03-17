import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { listDocuments, createDocument, DocumentSummary } from './api';
import { useAuth } from './auth/AuthContext';
import { ShareDialog } from './ShareDialog';

const ROLE_LABELS: Record<string, string> = {
  edit: 'Editor',
  comment: 'Commenter',
  read: 'Viewer',
};

export function Dashboard() {
  const [docs, setDocs] = useState<DocumentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [sharingDocId, setSharingDocId] = useState<string | null>(null);
  const navigate = useNavigate();
  const { user, logout } = useAuth();

  useEffect(() => {
    listDocuments()
      .then(setDocs)
      .finally(() => setLoading(false));
  }, []);

  const handleCreate = async () => {
    const doc = await createDocument('Untitled');
    navigate(`/docs/${doc.id}`);
  };

  const sharingDoc = sharingDocId ? docs.find((d) => d.id === sharingDocId) : null;

  return (
    <div className="dashboard">
      <header className="dashboard-header">
        <h1>Cadmus</h1>
        <div className="dashboard-header-actions">
          <button onClick={handleCreate} className="btn-primary">
            New Document
          </button>
          <span className="dashboard-user">{user?.display_name}</span>
          <button onClick={logout} className="btn-logout">
            Log out
          </button>
        </div>
      </header>
      <main className="dashboard-content">
        {loading ? (
          <p className="dashboard-loading">Loading...</p>
        ) : docs.length === 0 ? (
          <div className="dashboard-empty">
            <p>No documents yet.</p>
            <p>Create your first document to get started.</p>
          </div>
        ) : (
          <div className="document-list">
            {docs.map((doc) => (
              <div key={doc.id} className="document-card-wrapper">
                <button className="document-card" onClick={() => navigate(`/docs/${doc.id}`)}>
                  <div className="document-card-header">
                    <h3>{doc.title}</h3>
                    <span className={`role-badge role-${doc.role}`}>
                      {ROLE_LABELS[doc.role] || doc.role}
                    </span>
                  </div>
                  <p className="document-meta">
                    Updated {new Date(doc.updated_at).toLocaleDateString()}
                  </p>
                </button>
                {doc.is_owner && (
                  <button
                    className="btn-share"
                    onClick={(e) => {
                      e.stopPropagation();
                      setSharingDocId(doc.id);
                    }}
                  >
                    Share
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </main>

      {sharingDoc && (
        <ShareDialog
          docId={sharingDoc.id}
          docTitle={sharingDoc.title}
          onClose={() => setSharingDocId(null)}
        />
      )}
    </div>
  );
}
