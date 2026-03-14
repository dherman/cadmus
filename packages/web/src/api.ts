const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

// --- Auth types ---

export interface UserProfile {
  id: string;
  email: string;
  display_name: string;
}

export interface AuthResponse {
  user: UserProfile;
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export interface TokenResponse {
  access_token: string;
  expires_in: number;
}

export interface WsTokenResponse {
  ws_token: string;
  expires_in: number;
}

// --- Auth API functions ---

export async function registerUser(
  email: string,
  displayName: string,
  password: string,
): Promise<AuthResponse> {
  const res = await fetch(`${API_BASE}/api/auth/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, display_name: displayName, password }),
  });
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Registration failed');
  }
  return res.json();
}

export async function loginUser(email: string, password: string): Promise<AuthResponse> {
  const res = await fetch(`${API_BASE}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  });
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Login failed');
  }
  return res.json();
}

export async function refreshToken(token: string): Promise<TokenResponse> {
  const res = await fetch(`${API_BASE}/api/auth/refresh`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ refresh_token: token }),
  });
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Token refresh failed');
  }
  return res.json();
}

export async function fetchWsToken(accessToken: string): Promise<WsTokenResponse> {
  const res = await fetch(`${API_BASE}/api/auth/ws-token`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Failed to get WebSocket token');
  }
  return res.json();
}

export async function fetchMe(accessToken: string): Promise<UserProfile> {
  const res = await fetch(`${API_BASE}/api/auth/me`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!res.ok) throw new Error('Failed to fetch user profile');
  return res.json();
}

// --- Auth-aware fetch ---

let getAccessTokenFn: (() => Promise<string>) | null = null;

export function setAccessTokenProvider(fn: () => Promise<string>) {
  getAccessTokenFn = fn;
}

async function authFetch(url: string, options: RequestInit = {}): Promise<Response> {
  if (!getAccessTokenFn) throw new Error('Auth not initialized');
  const token = await getAccessTokenFn();
  return fetch(url, {
    ...options,
    headers: {
      ...options.headers,
      Authorization: `Bearer ${token}`,
    },
  });
}

// --- Document API ---

export interface DocumentSummary {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export async function listDocuments(): Promise<DocumentSummary[]> {
  const res = await authFetch(`${API_BASE}/api/docs`);
  if (!res.ok) throw new Error('Failed to fetch documents');
  return res.json();
}

export async function createDocument(title: string): Promise<DocumentSummary> {
  const res = await authFetch(`${API_BASE}/api/docs`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ title }),
  });
  if (!res.ok) throw new Error('Failed to create document');
  return res.json();
}

export async function getDocument(id: string): Promise<DocumentSummary> {
  const res = await authFetch(`${API_BASE}/api/docs/${encodeURIComponent(id)}`);
  if (!res.ok) throw new Error('Document not found');
  return res.json();
}
