import type { UserProfile } from './api';

const COLORS = [
  '#e06c75',
  '#e5c07b',
  '#98c379',
  '#56b6c2',
  '#61afef',
  '#c678dd',
  '#d19a66',
  '#be5046',
  '#7ec699',
  '#f472b6',
  '#a78bfa',
  '#fb923c',
];

export interface UserIdentity {
  name: string;
  color: string;
  isAgent?: boolean;
  agentStatus?: string | null;
}

function hashCode(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash |= 0;
  }
  return hash;
}

export function getUserIdentity(user: UserProfile): UserIdentity {
  const colorIndex = Math.abs(hashCode(user.id)) % COLORS.length;
  return {
    name: user.display_name,
    color: COLORS[colorIndex],
  };
}
