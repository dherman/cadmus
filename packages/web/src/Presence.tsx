import { useEffect, useState } from 'react';
import type { WebsocketProvider } from 'y-websocket';

interface PresenceUser {
  name: string;
  color: string;
}

export function Presence({ provider }: { provider: WebsocketProvider }) {
  const [users, setUsers] = useState<PresenceUser[]>([]);

  useEffect(() => {
    const update = () => {
      const states = Array.from(provider.awareness.getStates().values());
      setUsers(states.filter((s) => s.user).map((s) => s.user as PresenceUser));
    };

    provider.awareness.on('change', update);
    update();

    return () => {
      provider.awareness.off('change', update);
    };
  }, [provider]);

  return (
    <div className="presence" aria-label="Connected users">
      {users.map((user, i) => (
        <div key={i} className="presence-user" title={user.name}>
          <span className="presence-dot" style={{ backgroundColor: user.color }} />
          <span className="presence-name">{user.name}</span>
        </div>
      ))}
    </div>
  );
}
