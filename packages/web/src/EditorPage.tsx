import { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router';
import { Editor } from './Editor';
import { Presence } from './Presence';
import { ShareDialog } from './ShareDialog';
import { useCollaboration } from './useCollaboration';
import { useAuth } from './auth/AuthContext';
import { getDocument, fetchDocumentContent, DocumentSummary } from './api';

export function EditorPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { getWsToken, user } = useAuth();
  const [wsToken, setWsToken] = useState<string | null>(null);
  const [tokenError, setTokenError] = useState<string | null>(null);
  const [doc, setDoc] = useState<DocumentSummary | null>(null);
  const [docError, setDocError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const [token, docData] = await Promise.all([getWsToken(), getDocument(id!)]);
        if (!cancelled) {
          setWsToken(token);
          setDoc(docData);
        }
      } catch (err) {
        if (!cancelled) {
          const msg = err instanceof Error ? err.message : 'Failed to load';
          if (msg.includes('access')) {
            setDocError(msg);
          } else {
            setTokenError(msg);
          }
        }
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, [id, getWsToken]);

  if (docError) {
    return (
      <div className="app">
        <header className="app-header">
          <button className="back-button" onClick={() => navigate('/')}>
            &larr; Documents
          </button>
          <h1>Cadmus</h1>
        </header>
        <main className="app-main">
          <div className="editor-error">
            <p>{docError}</p>
            <button className="btn-primary" onClick={() => navigate('/')}>
              Back to Dashboard
            </button>
          </div>
        </main>
      </div>
    );
  }

  if (tokenError) {
    return (
      <div className="app">
        <p>Failed to connect: {tokenError}</p>
      </div>
    );
  }

  if (!wsToken || !doc) {
    return (
      <div className="app">
        <p className="auth-loading">Connecting...</p>
      </div>
    );
  }

  return (
    <EditorPageInner
      docId={id!}
      wsToken={wsToken}
      navigate={navigate}
      user={user}
      getWsToken={getWsToken}
      doc={doc}
    />
  );
}

function downloadMarkdown(filename: string, content: string) {
  const blob = new Blob([content], { type: 'text/markdown' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

function slugify(title: string): string {
  const slug = title
    .toLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^a-z0-9-]/g, '');
  return slug || 'document';
}

function EditorPageInner({
  docId,
  wsToken,
  navigate,
  user,
  getWsToken,
  doc,
}: {
  docId: string;
  wsToken: string;
  navigate: ReturnType<typeof useNavigate>;
  user: ReturnType<typeof useAuth>['user'];
  getWsToken: () => Promise<string>;
  doc: DocumentSummary;
}) {
  const { ydoc, provider, connectionStatus } = useCollaboration(docId, wsToken);
  const [showShareDialog, setShowShareDialog] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);

  const isEditable = doc.role === 'edit';

  async function handleExport() {
    setExporting(true);
    setExportError(null);
    try {
      const result = await fetchDocumentContent(docId, 'markdown');
      downloadMarkdown(`${slugify(doc.title)}.md`, result.content as string);
    } catch (err) {
      setExportError(err instanceof Error ? err.message : 'Export failed');
    } finally {
      setExporting(false);
    }
  }

  // Handle ws-token expiry (close code 4401)
  useEffect(() => {
    if (!provider) return;

    const ws = provider.ws;
    if (!ws) return;

    const handleClose = async (event: CloseEvent) => {
      if (event.code === 4401) {
        try {
          const newToken = await getWsToken();
          (provider as unknown as { url: string }).url =
            `${provider.url.split('?')[0]}?token=${encodeURIComponent(newToken)}`;
          provider.connect();
        } catch {
          // If we can't get a new token, the user will see disconnected status
        }
      }
    };

    ws.addEventListener('close', handleClose);
    return () => {
      ws.removeEventListener('close', handleClose);
    };
  }, [provider, getWsToken]);

  return (
    <div className="app">
      <header className="app-header">
        <button className="back-button" onClick={() => navigate('/')}>
          &larr; Documents
        </button>
        <h1>Cadmus</h1>
        <span className={`status-dot ${connectionStatus}`} />
        {provider && <Presence provider={provider} />}
        {doc.is_owner && (
          <button className="btn-share" onClick={() => setShowShareDialog(true)}>
            Share
          </button>
        )}
        <button className="btn-export" onClick={handleExport} disabled={exporting}>
          {exporting ? 'Exporting\u2026' : 'Export'}
        </button>
        {exportError && <span className="export-error">{exportError}</span>}
      </header>
      <main className="app-main">
        {ydoc && provider && (
          <Editor ydoc={ydoc} provider={provider} user={user} editable={isEditable} />
        )}
      </main>

      {showShareDialog && (
        <ShareDialog docId={docId} docTitle={doc.title} onClose={() => setShowShareDialog(false)} />
      )}
    </div>
  );
}
