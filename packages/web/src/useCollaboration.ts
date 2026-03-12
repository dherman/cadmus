import { useEffect, useState } from 'react';
import { createCollaborationProvider, destroyCollaborationProvider } from './collaboration';
import type * as Y from 'yjs';
import type { WebsocketProvider } from 'y-websocket';

export type ConnectionStatus = 'connected' | 'connecting' | 'disconnected';

export function useCollaboration(docId: string) {
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>('connecting');
  const [collab, setCollab] = useState<{ ydoc: Y.Doc; provider: WebsocketProvider } | null>(null);

  useEffect(() => {
    const { ydoc, provider } = createCollaborationProvider(docId);

    const handleStatus = ({ status }: { status: ConnectionStatus }) => {
      setConnectionStatus(status);
    };
    provider.on('status', handleStatus);
    setCollab({ ydoc, provider });

    return () => {
      provider.off('status', handleStatus);
      destroyCollaborationProvider(provider);
      setCollab(null);
      setConnectionStatus('connecting');
    };
  }, [docId]);

  return {
    ydoc: collab?.ydoc ?? null,
    provider: collab?.provider ?? null,
    connectionStatus,
    isConnected: connectionStatus === 'connected',
  };
}
