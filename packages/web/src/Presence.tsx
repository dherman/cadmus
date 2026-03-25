import { useEffect, useState } from 'react';
import type { WebsocketProvider } from 'y-websocket';
import { BotIcon } from './BotIcon';

interface PresenceUser {
  name: string;
  color: string;
  isAgent?: boolean;
  agentStatus?: string | null;
}

export function Presence({ provider }: { provider: WebsocketProvider }) {
  const [users, setUsers] = useState<PresenceUser[]>([]);

  useEffect(() => {
    const update = () => {
      const states = Array.from(provider.awareness.getStates().values());
      setUsers(
        states
          .filter((s) => s.user)
          .map((s) => ({
            name: s.user.name as string,
            color: s.user.color as string,
            isAgent: s.user.isAgent as boolean | undefined,
            agentStatus: s.user.agentStatus as string | null | undefined,
          })),
      );
    };

    provider.awareness.on('change', update);
    update();

    return () => {
      provider.awareness.off('change', update);
    };
  }, [provider]);

  return (
    <div className="presence" aria-label="Connected users">
      {users.map((user, i) =>
        user.isAgent ? (
          <div key={i} className="presence-agent" title={user.name}>
            <BotIcon />
            <span className="agent-name">{user.name}</span>
            {user.agentStatus && <span className="agent-status">{user.agentStatus}</span>}
          </div>
        ) : (
          <div key={i} className="presence-user" title={user.name}>
            <span className="presence-dot" style={{ backgroundColor: user.color }} />
            <span className="presence-name">{user.name}</span>
          </div>
        ),
      )}
    </div>
  );
}
