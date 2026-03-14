import * as Y from 'yjs';
import { WebsocketProvider } from 'y-websocket';

// Base URL for the WebSocket endpoint, without trailing slash.
// The provider will connect to: ${WS_BASE_URL}/${docId}/ws
const WS_BASE_URL = import.meta.env.VITE_WS_URL ?? 'ws://localhost:8080/api/docs';

export function createCollaborationProvider(docId: string, wsToken: string) {
  const ydoc = new Y.Doc();
  // y-websocket appends /${roomname} to the server URL, so we pass the docId/ws
  // path with the token as a query parameter to hit /api/docs/{id}/ws?token=...
  const provider = new WebsocketProvider(
    WS_BASE_URL,
    `${docId}/ws?token=${encodeURIComponent(wsToken)}`,
    ydoc,
  );
  return { ydoc, provider };
}

export function destroyCollaborationProvider(provider: WebsocketProvider) {
  provider.disconnect();
  provider.destroy();
}
