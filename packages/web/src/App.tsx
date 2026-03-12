import { Editor } from './Editor';
import { useCollaboration } from './useCollaboration';
import { DEFAULT_DOC_ID } from './collaboration';

export function App() {
  const { ydoc, provider, connectionStatus } = useCollaboration(DEFAULT_DOC_ID);

  return (
    <div className="app">
      <header className="app-header">
        <h1>Cadmus</h1>
        <span className={`status-dot ${connectionStatus}`} />
      </header>
      <main className="app-main">
        {ydoc && provider && <Editor ydoc={ydoc} provider={provider} />}
      </main>
    </div>
  );
}
