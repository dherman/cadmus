import { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router';
import { Editor } from './Editor';
import { Presence } from './Presence';
import { useCollaboration } from './useCollaboration';
import { useAuth } from './auth/AuthContext';

export function EditorPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { getWsToken, user } = useAuth();
  const [wsToken, setWsToken] = useState<string | null>(null);
  const [tokenError, setTokenError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getWsToken()
      .then((token) => {
        if (!cancelled) setWsToken(token);
      })
      .catch((err) => {
        if (!cancelled) setTokenError(err instanceof Error ? err.message : 'Failed to get token');
      });
    return () => {
      cancelled = true;
    };
  }, [id, getWsToken]);

  if (tokenError) {
    return (
      <div className="app">
        <p>Failed to connect: {tokenError}</p>
      </div>
    );
  }

  if (!wsToken) {
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
    />
  );
}

function EditorPageInner({
  docId,
  wsToken,
  navigate,
  user,
  getWsToken,
}: {
  docId: string;
  wsToken: string;
  navigate: ReturnType<typeof useNavigate>;
  user: ReturnType<typeof useAuth>['user'];
  getWsToken: () => Promise<string>;
}) {
  const { ydoc, provider, connectionStatus } = useCollaboration(docId, wsToken);

  // Handle ws-token expiry (close code 4401)
  useEffect(() => {
    if (!provider) return;

    const ws = provider.ws;
    if (!ws) return;

    const handleClose = async (event: CloseEvent) => {
      if (event.code === 4401) {
        try {
          const newToken = await getWsToken();
          provider.url = `${provider.url.split('?')[0]}?token=${encodeURIComponent(newToken)}`;
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
      </header>
      <main className="app-main">
        {ydoc && provider && <Editor ydoc={ydoc} provider={provider} user={user} />}
      </main>
    </div>
  );
}
