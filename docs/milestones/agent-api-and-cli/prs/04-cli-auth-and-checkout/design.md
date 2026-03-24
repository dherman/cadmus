# PR 4: CLI Auth & Checkout

## Purpose

Implement the CLI's authentication flow and document checkout command. After this PR, a user can run `cadmus auth login` to authenticate, `cadmus docs list` to see their documents, and `cadmus checkout <doc-id>` to download a document as markdown with version tracking. This PR builds the CLI's foundation — HTTP client, credential storage, config management — that PR 5 builds on for push.

## Commands

### `cadmus auth login`

```bash
$ cadmus auth login --server http://localhost:8080
Email: alice@example.com
Password: ********
✓ Authenticated as Alice (alice@example.com)
  Credentials saved to ~/.config/cadmus/credentials
```

Interactive login flow:

1. Prompt for email and password (using `inquirer` or `readline`).
2. Call `POST /api/auth/login` with the credentials.
3. Store the access token, refresh token, and server URL in `~/.config/cadmus/credentials`.
4. Confirm success with the user's display name.

Agent token login (non-interactive):

```bash
$ cadmus auth login --server http://localhost:8080 --token cadmus_abc123...
✓ Authenticated with agent token
  Credentials saved to ~/.config/cadmus/credentials
```

When `--token` is provided, skip the interactive prompt. Store the agent token directly — no refresh flow needed.

### `cadmus auth status`

```bash
$ cadmus auth status
✓ Logged in as Alice (alice@example.com)
  Server: http://localhost:8080
```

Reads credentials and calls `GET /api/auth/me` to verify they're still valid. Shows "Not logged in" if no credentials or if the token is expired/invalid.

### `cadmus docs list`

```bash
$ cadmus docs list
ID                                    Title                    Updated
────────────────────────────────────  ───────────────────────  ─────────────
550e8400-e29b-41d4-a716-446655440000  Design Spec              2 hours ago
6ba7b810-9dad-11d1-80b4-00c04fd430c8  Meeting Notes            3 days ago
```

Calls `GET /api/docs`, formats the result as a table. Uses relative timestamps for readability.

### `cadmus checkout <doc-id>`

```bash
$ cadmus checkout 550e8400 -o ./design-spec.md
✓ Checked out "Design Spec" (version v_abc123)
  Written to ./design-spec.md
  Metadata saved to .cadmus/550e8400-e29b-41d4-a716-446655440000.json
```

1. Resolve short IDs: if the provided ID is a prefix, match against the full document list. Error if ambiguous.
2. Call `GET /api/docs/{id}/content?format=markdown`.
3. Write the markdown content to the output file (default: `./<title-slugified>.md`).
4. Create `.cadmus/<doc-id>.json` with checkout metadata.

## Credential Storage

Credentials are stored in `~/.config/cadmus/credentials` as JSON:

```json
{
  "server": "http://localhost:8080",
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "token_type": "jwt"
}
```

Or for agent token auth:

```json
{
  "server": "http://localhost:8080",
  "access_token": "cadmus_abc123...",
  "token_type": "agent"
}
```

File permissions are set to `0600` (owner read/write only).

The CLI's HTTP client automatically:

- Reads credentials from this file.
- Adds `Authorization: Bearer <token>` to all requests.
- For JWT auth: attempts token refresh via `POST /api/auth/refresh` if a request returns 401, then retries.
- For agent token auth: no refresh — a 401 means the token is expired/revoked.

## Checkout Metadata

The `.cadmus/` directory is created relative to the output file's directory:

```
project/
  design-spec.md
  .cadmus/
    550e8400-e29b-41d4-a716-446655440000.json
```

Metadata file format:

```json
{
  "doc_id": "550e8400-e29b-41d4-a716-446655440000",
  "version": "v_abc123",
  "title": "Design Spec",
  "checked_out_at": "2026-03-24T10:30:00Z",
  "file": "./design-spec.md",
  "server": "http://localhost:8080"
}
```

The `version` field is the `base_version` that PR 5's push command will use.

## HTTP Client Module

A thin wrapper around `fetch` (Node's built-in) that handles:

- Base URL construction from stored server config.
- Authorization header injection.
- JSON request/response serialization.
- Token refresh on 401 (for JWT auth).
- User-friendly error messages for common failures (network error, 403, 404).

```typescript
// api.ts
export class CadmusClient {
    constructor(private server: string, private getToken: () => Promise<string>) {}

    async get(path: string): Promise<any> { ... }
    async post(path: string, body: any): Promise<any> { ... }

    // High-level methods
    async listDocuments(): Promise<Document[]> { ... }
    async getDocumentContent(docId: string, format: string): Promise<ContentResponse> { ... }
    async pushContent(docId: string, body: PushRequest): Promise<PushResponse> { ... }
}
```

## Dependencies

New dependencies for the CLI package:

- `chalk` — colored terminal output
- `ora` — spinner for async operations

The CLI avoids heavy dependencies. Interactive prompts use Node's built-in `readline` module rather than adding `inquirer`.

## Error Handling

The CLI provides clear, actionable error messages:

| Scenario            | Output                                                                         |
| ------------------- | ------------------------------------------------------------------------------ |
| Not logged in       | `Error: Not logged in. Run 'cadmus auth login' first.`                         |
| Network error       | `Error: Cannot reach server at http://localhost:8080`                          |
| 401 (expired)       | `Error: Session expired. Run 'cadmus auth login' to re-authenticate.`          |
| 403 (no permission) | `Error: You don't have access to this document.`                               |
| 404 (doc not found) | `Error: Document not found: 550e8400...`                                       |
| Ambiguous short ID  | `Error: Ambiguous ID prefix '550e'. Did you mean: 550e8400... or 550e9911...?` |
| Output file exists  | Overwrite without prompting (checkout is idempotent)                           |

## What's Not Included

- Push command (PR 5)
- Comment command (deferred — endpoint exists but CLI UX needs design)
- `cadmus docs create` (can be added as a fast follow)
- Offline mode / retry queue
