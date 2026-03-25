/**
 * Credential storage (~/.config/cadmus/credentials)
 * and checkout metadata (.cadmus/<doc-id>.json)
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

export interface Credentials {
  server: string;
  access_token: string;
  refresh_token?: string;
  token_type: 'jwt' | 'agent';
}

export interface CheckoutMetadata {
  doc_id: string;
  version: string;
  title: string;
  checked_out_at: string;
  file: string;
  server: string;
}

export function getCredentialsPath(): string {
  return path.join(os.homedir(), '.config', 'cadmus', 'credentials');
}

export function loadCredentials(): Credentials | null {
  const credPath = getCredentialsPath();
  if (!fs.existsSync(credPath)) {
    return null;
  }
  const raw = fs.readFileSync(credPath, 'utf-8');
  return JSON.parse(raw) as Credentials;
}

export function saveCredentials(creds: Credentials): void {
  const credPath = getCredentialsPath();
  const dir = path.dirname(credPath);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(credPath, JSON.stringify(creds, null, 2), 'utf-8');
  fs.chmodSync(credPath, 0o600);
}

export function getMetadataDir(fileDir: string): string {
  return path.join(fileDir, '.cadmus');
}

export function loadCheckoutMetadata(fileDir: string, docId: string): CheckoutMetadata | null {
  const metaPath = path.join(getMetadataDir(fileDir), `${docId}.json`);
  if (!fs.existsSync(metaPath)) {
    return null;
  }
  const raw = fs.readFileSync(metaPath, 'utf-8');
  return JSON.parse(raw) as CheckoutMetadata;
}

export function saveCheckoutMetadata(fileDir: string, meta: CheckoutMetadata): void {
  const metaDir = getMetadataDir(fileDir);
  fs.mkdirSync(metaDir, { recursive: true });
  const metaPath = path.join(metaDir, `${meta.doc_id}.json`);
  fs.writeFileSync(metaPath, JSON.stringify(meta, null, 2), 'utf-8');
}
