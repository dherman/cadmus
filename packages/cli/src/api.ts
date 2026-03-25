/**
 * HTTP client for the Cadmus server API.
 */

export class ApiError extends Error {
  constructor(
    public status: number,
    public body: string,
  ) {
    super(`HTTP ${status}: ${body}`);
    this.name = 'ApiError';
  }
}

export interface LoginResponse {
  user: { id: string; email: string; display_name: string };
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export interface UserProfile {
  id: string;
  email: string;
  display_name: string;
}

export interface DocumentSummary {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  role: string;
  is_owner: boolean;
}

export interface ContentResponse {
  version: string;
  format: string;
  content: string;
}

export interface PushRequest {
  base_version: string;
  format: string;
  content: string;
}

export interface PushResponse {
  version?: string;
  status: string;
  changes_summary?: {
    steps_applied: number;
    nodes_added: number;
    nodes_removed: number;
    nodes_modified: number;
  };
  diff?: string;
}

export class CadmusClient {
  public readonly server: string;
  private getToken: () => Promise<string>;

  constructor(server: string, getToken: () => Promise<string>) {
    this.server = server;
    this.getToken = getToken;
  }

  private async request(method: string, path: string, body?: unknown): Promise<unknown> {
    const url = `${this.server}${path}`;
    const token = await this.getToken();
    const headers: Record<string, string> = {};

    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }
    if (body) {
      headers['Content-Type'] = 'application/json';
    }

    let res: Response;
    try {
      res = await fetch(url, {
        method,
        headers,
        body: body ? JSON.stringify(body) : undefined,
      });
    } catch {
      throw new Error(`Cannot reach server at ${this.server}`);
    }

    if (!res.ok) {
      const text = await res.text();
      throw new ApiError(res.status, text);
    }
    if (res.status === 204) return null;
    return res.json();
  }

  async get(path: string): Promise<unknown> {
    return this.request('GET', path);
  }

  async post(path: string, body: unknown): Promise<unknown> {
    return this.request('POST', path, body);
  }

  // --- High-level methods ---

  async login(email: string, password: string): Promise<LoginResponse> {
    return (await this.post('/api/auth/login', {
      email,
      password,
    })) as LoginResponse;
  }

  async getMe(): Promise<UserProfile> {
    return (await this.get('/api/auth/me')) as UserProfile;
  }

  async listDocuments(): Promise<DocumentSummary[]> {
    return (await this.get('/api/docs')) as DocumentSummary[];
  }

  async getDocument(docId: string): Promise<DocumentSummary> {
    return (await this.get(`/api/docs/${docId}`)) as DocumentSummary;
  }

  async getDocumentContent(docId: string, format: string): Promise<ContentResponse> {
    return (await this.get(`/api/docs/${docId}/content?format=${format}`)) as ContentResponse;
  }

  async pushContent(docId: string, body: PushRequest, dryRun?: boolean): Promise<PushResponse> {
    const query = dryRun ? '?dry_run=true' : '';
    return (await this.post(`/api/docs/${docId}/content${query}`, body)) as PushResponse;
  }
}
