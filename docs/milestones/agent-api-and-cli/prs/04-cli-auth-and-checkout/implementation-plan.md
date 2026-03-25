# PR 4: CLI Auth & Checkout — Implementation Plan

## Prerequisites

- [x] PR 1 (Agent Token Management) is merged

## Steps

### 1. Add CLI dependencies

- [x] Update `packages/cli/package.json` to add dependencies:

```json
{
  "dependencies": {
    "commander": "^12.0.0",
    "chalk": "^5.3.0",
    "ora": "^8.0.0"
  }
}
```

- [x] Run `pnpm install` from the workspace root.

### 2. Add TypeScript configuration

- [x] Create `packages/cli/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "esModuleInterop": true,
    "resolveJsonModule": true,
    "declaration": true
  },
  "include": ["src/**/*"]
}
```

### 3. Implement config module

- [x] Create `packages/cli/src/config.ts`:

```typescript
// Manages credential storage (~/.config/cadmus/credentials)
// and checkout metadata (.cadmus/<doc-id>.json)

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

export function getCredentialsPath(): string { ... }
export function loadCredentials(): Credentials | null { ... }
export function saveCredentials(creds: Credentials): void { ... }

export function getMetadataDir(fileDir: string): string { ... }
export function loadCheckoutMetadata(fileDir: string, docId: string): CheckoutMetadata | null { ... }
export function saveCheckoutMetadata(fileDir: string, meta: CheckoutMetadata): void { ... }
```

- [x] Use `os.homedir()` + `path.join` for `~/.config/cadmus/credentials`.
- [x] Use `fs.mkdirSync` with `{ recursive: true }` to create directories.
- [x] Set file permissions to `0o600` on credentials file via `fs.chmodSync`.

### 4. Implement API client module

- [x] Create `packages/cli/src/api.ts`:

```typescript
export class CadmusClient {
    constructor(
        private server: string,
        private getToken: () => Promise<string>,
    ) {}

    private async request(method: string, path: string, body?: any): Promise<any> {
        const url = `${this.server}${path}`;
        const headers: Record<string, string> = {
            'Authorization': `Bearer ${await this.getToken()}`,
        };
        if (body) {
            headers['Content-Type'] = 'application/json';
        }
        const res = await fetch(url, {
            method,
            headers,
            body: body ? JSON.stringify(body) : undefined,
        });
        if (!res.ok) {
            throw new ApiError(res.status, await res.text());
        }
        if (res.status === 204) return null;
        return res.json();
    }

    async get(path: string) { return this.request('GET', path); }
    async post(path: string, body: any) { return this.request('POST', path, body); }

    // High-level methods
    async login(email: string, password: string): Promise<LoginResponse> { ... }
    async getMe(): Promise<User> { ... }
    async listDocuments(): Promise<Document[]> { ... }
    async getDocumentContent(docId: string, format: string): Promise<ContentResponse> { ... }
    async pushContent(docId: string, body: PushRequest, dryRun?: boolean): Promise<PushResponse> { ... }
}

export class ApiError extends Error {
    constructor(public status: number, public body: string) {
        super(`HTTP ${status}: ${body}`);
    }
}
```

### 5. Implement auth commands

- [x] Create `packages/cli/src/auth.ts`:

```typescript
export async function loginCommand(options: { server: string; token?: string }) {
  if (options.token) {
    // Agent token login — store directly
    saveCredentials({
      server: options.server,
      access_token: options.token,
      token_type: 'agent',
    });
    console.log(chalk.green('✓') + ' Authenticated with agent token');
    return;
  }

  // Interactive login
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  const email = await question(rl, 'Email: ');
  const password = await questionHidden(rl, 'Password: ');
  rl.close();

  const spinner = ora('Authenticating...').start();
  const client = new CadmusClient(options.server, async () => '');
  const result = await client.login(email, password);
  spinner.stop();

  saveCredentials({
    server: options.server,
    access_token: result.access_token,
    refresh_token: result.refresh_token,
    token_type: 'jwt',
  });

  console.log(chalk.green('✓') + ` Authenticated as ${result.user.display_name}`);
  console.log(`  Credentials saved to ${getCredentialsPath()}`);
}
```

- [x] Implement password hiding in the terminal (disable echo during password input).
- [x] Implement `statusCommand()` that loads credentials and calls `GET /api/auth/me`.

### 6. Implement docs list command

- [x] In `packages/cli/src/main.ts`, implement the `docs list` action:

```typescript
docs
  .command('list')
  .description('List accessible documents')
  .action(async () => {
    const client = await getAuthenticatedClient();
    const spinner = ora('Loading documents...').start();
    const docs = await client.listDocuments();
    spinner.stop();

    if (docs.length === 0) {
      console.log('No documents found.');
      return;
    }

    // Format as table
    const rows = docs.map((d) => ({
      ID: d.id,
      Title: d.title,
      Updated: formatRelativeTime(d.updated_at),
    }));
    printTable(rows);
  });
```

- [x] Implement `printTable(rows)` — a simple table formatter using fixed-width columns.
- [x] Implement `formatRelativeTime(iso: string)` — converts ISO timestamp to "2 hours ago", "3 days ago", etc.

### 7. Implement checkout command

- [x] In `packages/cli/src/main.ts`, implement the checkout action:

```typescript
program
  .command('checkout <doc-id>')
  .description('Download a document as markdown')
  .option('-o, --output <path>', 'Output file path')
  .action(async (docId, options) => {
    const client = await getAuthenticatedClient();

    // Resolve short IDs
    const fullId = await resolveDocId(client, docId);

    const spinner = ora('Checking out...').start();
    const content = await client.getDocumentContent(fullId, 'markdown');
    const doc = await client.getDocument(fullId);
    spinner.stop();

    // Determine output path
    const outputPath = options.output || `./${slugify(doc.title)}.md`;

    // Write markdown file
    fs.writeFileSync(outputPath, content.content, 'utf-8');

    // Save checkout metadata
    const fileDir = path.dirname(path.resolve(outputPath));
    saveCheckoutMetadata(fileDir, {
      doc_id: fullId,
      version: content.version,
      title: doc.title,
      checked_out_at: new Date().toISOString(),
      file: outputPath,
      server: client.server,
    });

    console.log(chalk.green('✓') + ` Checked out "${doc.title}" (version ${content.version})`);
    console.log(`  Written to ${outputPath}`);
    console.log(`  Metadata saved to .cadmus/${fullId}.json`);
  });
```

### 8. Implement document ID resolution

- [x] Add `resolveDocId(client, shortId)`:
  - If `shortId` is a full UUID, use it directly.
  - Otherwise, call `listDocuments()` and find IDs that start with the prefix.
  - If exactly one match, return it.
  - If zero matches, error with "Document not found".
  - If multiple matches, error with "Ambiguous ID prefix" and list the matches.

### 9. Implement helper utility: getAuthenticatedClient

- [x] Add a helper that loads credentials and constructs a `CadmusClient`:

```typescript
async function getAuthenticatedClient(): Promise<CadmusClient> {
  const creds = loadCredentials();
  if (!creds) {
    console.error(chalk.red('Error:') + " Not logged in. Run 'cadmus auth login' first.");
    process.exit(1);
  }
  return new CadmusClient(creds.server, async () => {
    // For JWT: attempt refresh if needed
    // For agent: return token directly
    return creds.access_token;
  });
}
```

### 10. Wire up all commands in main.ts

- [x] Rewrite `packages/cli/src/main.ts` to use the implementations from steps 5–9, replacing all TODO stubs.

### 11. Test the CLI manually

- [ ] Start the dev stack: `pnpm dev`
- [ ] Test login:

```bash
cd packages/cli && pnpm dev -- auth login --server http://localhost:8080
```

- [ ] Test docs list:

```bash
pnpm dev -- docs list
```

- [ ] Test checkout:

```bash
pnpm dev -- checkout <doc-id> -o ./test-doc.md
cat ./test-doc.md        # verify content
cat .cadmus/<doc-id>.json  # verify metadata
```

- [ ] Test agent token login:

```bash
# First create a token via curl
# Then:
pnpm dev -- auth login --server http://localhost:8080 --token cadmus_...
pnpm dev -- docs list   # should work with agent token
```

### 12. Build and format check

- [x] Run `pnpm -F @cadmus/cli build` (or verify tsx works) — compiles without errors.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [ ] `cadmus auth login` prompts for email/password and stores credentials
- [ ] `cadmus auth login --token` stores agent token directly
- [ ] `cadmus auth status` shows current login state
- [ ] `cadmus docs list` shows a formatted table of documents
- [ ] `cadmus checkout <id>` downloads markdown to the specified file
- [ ] `.cadmus/<doc-id>.json` is created with correct version and metadata
- [ ] Short document ID resolution works (prefix matching)
- [ ] Ambiguous short IDs show clear error with candidates
- [ ] Network errors show user-friendly messages
- [ ] 401 errors suggest re-authentication
- [ ] Credentials file has 0600 permissions

## Files Modified

| File                         | Change                                   |
| ---------------------------- | ---------------------------------------- |
| `packages/cli/package.json`  | Add chalk, ora dependencies              |
| `packages/cli/tsconfig.json` | New: TypeScript configuration            |
| `packages/cli/src/main.ts`   | Full rewrite with working commands       |
| `packages/cli/src/api.ts`    | New: HTTP client for Cadmus server       |
| `packages/cli/src/auth.ts`   | New: login flow and credential storage   |
| `packages/cli/src/config.ts` | New: config and metadata file management |
