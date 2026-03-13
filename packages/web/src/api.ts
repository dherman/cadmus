const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

export interface DocumentSummary {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export async function listDocuments(): Promise<DocumentSummary[]> {
  const res = await fetch(`${API_BASE}/api/docs`);
  if (!res.ok) throw new Error('Failed to fetch documents');
  return res.json();
}

export async function createDocument(title: string): Promise<DocumentSummary> {
  const res = await fetch(`${API_BASE}/api/docs`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ title }),
  });
  if (!res.ok) throw new Error('Failed to create document');
  return res.json();
}

export async function getDocument(id: string): Promise<DocumentSummary> {
  const res = await fetch(`${API_BASE}/api/docs/${encodeURIComponent(id)}`);
  if (!res.ok) throw new Error('Document not found');
  return res.json();
}
