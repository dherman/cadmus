import { useParams, useNavigate } from 'react-router';
import { Editor } from './Editor';
import { Presence } from './Presence';
import { useCollaboration } from './useCollaboration';

export function EditorPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { ydoc, provider, connectionStatus } = useCollaboration(id!);

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
        {ydoc && provider && <Editor ydoc={ydoc} provider={provider} />}
      </main>
    </div>
  );
}
