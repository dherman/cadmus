import * as Y from 'yjs';
import { WebsocketProvider } from 'y-websocket';

// Base URL for the WebSocket endpoint, without trailing slash.
// The provider will connect to: ${WS_BASE_URL}/${docId}/ws
const WS_BASE_URL = import.meta.env.VITE_WS_URL ?? 'ws://localhost:8080/api/docs';

export function createCollaborationProvider(docId: string, wsToken: string) {
  const ydoc = new Y.Doc();
  // y-websocket builds the URL as: serverUrl + '/' + roomname + '?token=...'
  // The token is passed via `params` (not embedded in the roomname) so that the
  // BroadcastChannel name — which is serverUrl + '/' + roomname — is the same
  // across all tabs viewing the same document. This enables y-websocket's
  // cross-tab BroadcastChannel sync.
  const provider = new WebsocketProvider(WS_BASE_URL, `${docId}/ws`, ydoc, {
    params: { token: wsToken },
  });
  return { ydoc, provider };
}

export function destroyCollaborationProvider(provider: WebsocketProvider) {
  provider.disconnect();
  provider.destroy();
}
