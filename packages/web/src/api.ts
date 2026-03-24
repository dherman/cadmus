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
  role: 'read' | 'comment' | 'edit';
  is_owner: boolean;
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
  if (!res.ok) {
    if (res.status === 403) throw new Error('You don\u2019t have access to this document');
    throw new Error('Document not found');
  }
  return res.json();
}

// --- Sharing / Permissions API ---

export interface PermissionEntry {
  user_id: string;
  email: string;
  display_name: string;
  role: string;
  is_owner: boolean;
}

export async function listPermissions(docId: string): Promise<PermissionEntry[]> {
  const res = await authFetch(`${API_BASE}/api/docs/${encodeURIComponent(docId)}/permissions`);
  if (!res.ok) throw new Error('Failed to fetch permissions');
  return res.json();
}

export async function addPermission(docId: string, email: string, role: string): Promise<void> {
  const res = await authFetch(`${API_BASE}/api/docs/${encodeURIComponent(docId)}/permissions`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, role }),
  });
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Failed to add permission');
  }
}

export async function updatePermission(docId: string, userId: string, role: string): Promise<void> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/permissions/${encodeURIComponent(userId)}`,
    {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ role }),
    },
  );
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Failed to update permission');
  }
}

export async function removePermission(docId: string, userId: string): Promise<void> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/permissions/${encodeURIComponent(userId)}`,
    { method: 'DELETE' },
  );
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Failed to remove permission');
  }
}

// --- Comments API ---

export interface CommentAuthor {
  id: string;
  display_name: string;
  email: string;
}

export interface Comment {
  id: string;
  document_id: string;
  author: CommentAuthor;
  parent_id: string | null;
  anchor_from: number | null;
  anchor_to: number | null;
  body: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export async function listComments(
  docId: string,
  status?: 'open' | 'resolved' | 'all',
): Promise<Comment[]> {
  const params = status ? `?status=${status}` : '';
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments${params}`,
  );
  if (!res.ok) throw new Error('Failed to list comments');
  return res.json();
}

export async function createComment(
  docId: string,
  body: string,
  anchorFrom?: number,
  anchorTo?: number,
): Promise<Comment> {
  const res = await authFetch(`${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      body,
      anchor_from: anchorFrom,
      anchor_to: anchorTo,
    }),
  });
  if (!res.ok) throw new Error('Failed to create comment');
  return res.json();
}

export async function replyToComment(
  docId: string,
  commentId: string,
  body: string,
): Promise<Comment> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments/${encodeURIComponent(commentId)}/replies`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ body }),
    },
  );
  if (!res.ok) throw new Error('Failed to reply to comment');
  return res.json();
}

export async function editComment(
  docId: string,
  commentId: string,
  body: string,
): Promise<Comment> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments/${encodeURIComponent(commentId)}`,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ body }),
    },
  );
  if (!res.ok) throw new Error('Failed to edit comment');
  return res.json();
}

export async function resolveComment(docId: string, commentId: string): Promise<Comment> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments/${encodeURIComponent(commentId)}/resolve`,
    { method: 'POST' },
  );
  if (!res.ok) throw new Error('Failed to resolve comment');
  return res.json();
}

export async function unresolveComment(docId: string, commentId: string): Promise<Comment> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(docId)}/comments/${encodeURIComponent(commentId)}/unresolve`,
    { method: 'POST' },
  );
  if (!res.ok) throw new Error('Failed to unresolve comment');
  return res.json();
}

// --- Document Content API ---

export interface DocumentContent {
  format: 'markdown' | 'json';
  content: string | object;
}

export async function fetchDocumentContent(
  id: string,
  format: 'markdown' | 'json' = 'json',
): Promise<DocumentContent> {
  const res = await authFetch(
    `${API_BASE}/api/docs/${encodeURIComponent(id)}/content?format=${format}`,
  );
  if (!res.ok) {
    const body = await res.json();
    throw new Error(body.error || 'Failed to fetch document content');
  }
  return res.json();
}
