# PR 5: CLI Push & Dry-Run — Implementation Plan

## Prerequisites

- [ ] PR 2 (Content Push Endpoint) is merged
- [ ] PR 3 (Step → Yrs Translation) is merged
- [ ] PR 4 (CLI Auth & Checkout) is merged

## Steps

### 1. Implement push command

- [x] In `packages/cli/src/main.ts`, implement the push action:

```typescript
program
  .command('push <doc-id> <file>')
  .description('Push local markdown changes to the server')
  .option('--dry-run', 'Preview changes without applying')
  .option('--force', 'Force push even with large diffs')
  .action(async (docId, file, options) => {
    const client = await getAuthenticatedClient();

    // Resolve doc ID
    const fullId = await resolveDocId(client, docId);

    // Read checkout metadata
    const fileDir = path.dirname(path.resolve(file));
    const meta = loadCheckoutMetadata(fileDir, fullId);
    if (!meta) {
      console.error(
        chalk.red('Error:') +
          ` No checkout metadata found for ${fullId}.` +
          ` Run 'cadmus checkout ${docId}' first.`,
      );
      process.exit(1);
    }

    // Read the local file
    if (!fs.existsSync(file)) {
      console.error(chalk.red('Error:') + ` File not found: ${file}`);
      process.exit(1);
    }
    const content = fs.readFileSync(file, 'utf-8');

    // Push
    const mode = options.dryRun ? 'Previewing' : 'Pushing';
    const spinner = ora(`${mode} changes...`).start();

    const result = await client.pushContent(
      fullId,
      {
        base_version: meta.version,
        format: 'markdown',
        content,
      },
      options.dryRun,
    );

    spinner.stop();

    if (options.dryRun) {
      displayDryRunResult(result, meta);
    } else {
      displayPushResult(result, meta);

      // Update checkout metadata with new version
      saveCheckoutMetadata(fileDir, {
        ...meta,
        version: result.version,
        checked_out_at: new Date().toISOString(),
      });
    }
  });
```

### 2. Implement dry-run display

- [x] Add `displayDryRunResult()`:

```typescript
function displayDryRunResult(result: PushResponse, meta: CheckoutMetadata) {
  console.log(`Preview of changes to "${meta.title}":\n`);

  if (result.diff) {
    // Colorize unified diff
    const lines = result.diff.split('\n');
    for (const line of lines) {
      if (line.startsWith('+')) {
        console.log(chalk.green(line));
      } else if (line.startsWith('-')) {
        console.log(chalk.red(line));
      } else if (line.startsWith('@@')) {
        console.log(chalk.cyan(line));
      } else {
        console.log(line);
      }
    }
  }

  console.log();
  displayChangeSummary(result.changes_summary);
  console.log();
  console.log('To apply these changes, run without --dry-run.');
}
```

### 3. Implement push result display

- [x] Add `displayPushResult()`:

```typescript
function displayPushResult(result: PushResponse, meta: CheckoutMetadata) {
  console.log(chalk.green('✓') + ` Pushed changes to "${meta.title}"`);
  console.log(`  New version: ${result.version}`);
  displayChangeSummary(result.changes_summary);
  console.log('  Checkout metadata updated.');
}
```

### 4. Implement change summary formatting

- [x] Add `displayChangeSummary()`:

```typescript
function displayChangeSummary(summary: ChangeSummary) {
  const parts = [];
  if (summary.nodes_added > 0) parts.push(`${summary.nodes_added} additions`);
  if (summary.nodes_removed > 0) parts.push(`${summary.nodes_removed} removals`);
  if (summary.nodes_modified > 0) parts.push(`${summary.nodes_modified} modifications`);

  console.log(`  Changes: ${summary.steps_applied} steps applied (${parts.join(', ')})`);
}
```

### 5. Implement auto-detection of document ID

- [x] When `doc-id` argument looks like a file path (contains `/` or `.`), try auto-detection:

```typescript
// If user runs: cadmus push ./design-spec.md
// Look for .cadmus/ directory and find matching metadata
function autoDetectDocId(filePath: string): string | null {
  const fileDir = path.dirname(path.resolve(filePath));
  const metaDir = path.join(fileDir, '.cadmus');
  if (!fs.existsSync(metaDir)) return null;

  const files = fs.readdirSync(metaDir).filter((f) => f.endsWith('.json'));
  for (const f of files) {
    const meta = JSON.parse(fs.readFileSync(path.join(metaDir, f), 'utf-8'));
    if (path.resolve(meta.file) === path.resolve(filePath)) {
      return meta.doc_id;
    }
  }
  return null;
}
```

- [x] Update the push command to use auto-detection when appropriate.

### 6. Add error handling for common push failures

- [x] Handle API errors with user-friendly messages:

```typescript
try {
    const result = await client.pushContent(...);
    // ...
} catch (err) {
    if (err instanceof ApiError) {
        switch (err.status) {
            case 404:
                console.error(
                    chalk.red('Error:') +
                    ` Base version ${meta.version} not found.` +
                    ` Re-checkout with 'cadmus checkout ${docId}'.`
                );
                break;
            case 403:
                console.error(chalk.red('Error:') + " You don't have edit access to this document.");
                break;
            case 422:
                console.error(chalk.red('Error:') + ' Failed to parse markdown: ' + err.body);
                break;
            default:
                console.error(chalk.red('Error:') + ` Server returned ${err.status}: ${err.body}`);
        }
    } else {
        console.error(chalk.red('Error:') + ` ${err.message}`);
    }
    process.exit(1);
}
```

### 7. Add pushContent method to API client

- [x] In `packages/cli/src/api.ts`, add:

```typescript
async pushContent(
    docId: string,
    body: { base_version: string; format: string; content: string },
    dryRun?: boolean,
): Promise<PushResponse> {
    const query = dryRun ? '?dry_run=true' : '';
    return this.post(`/api/docs/${encodeURIComponent(docId)}/content${query}`, body);
}
```

### 8. Test the full checkout→edit→push cycle

- [x] Start the dev stack: `pnpm dev`
- [x] Create a document and add content via the browser.
- [x] Checkout:

```bash
cd packages/cli
pnpm dev -- checkout <doc-id> -o ./test-doc.md
```

- [x] Edit the file locally (add a paragraph, change a heading).
- [x] Preview with dry-run:

```bash
pnpm dev -- push <doc-id> ./test-doc.md --dry-run
```

- [x] Verify the diff output shows the changes correctly.
- [x] Push for real:

```bash
pnpm dev -- push <doc-id> ./test-doc.md
```

- [x] Verify the browser shows the updated content.
- [x] Verify `.cadmus/<doc-id>.json` has the new version.
- [ ] Edit and push again (sequential push cycle).
- [ ] Verify the second push uses the updated base_version.

### 9. Test concurrent edit scenarios

- [ ] Open the document in the browser and make an edit to paragraph 1.
- [ ] Edit paragraph 3 in the local markdown file.
- [ ] Push the local changes.
- [ ] Verify both edits are preserved (three-way merge).

### 10. Build and format check

- [x] Run `pnpm -F @cadmus/cli build` (or verify tsx works) — compiles without errors.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [x] `cadmus push <doc-id> <file>` pushes changes successfully
- [x] Push response shows version and change summary
- [x] `--dry-run` shows colored unified diff without applying
- [x] `.cadmus/<doc-id>.json` is updated with new version after push
- [ ] Sequential push cycles work (push → edit → push)
- [ ] Missing checkout metadata shows helpful error
- [ ] Missing file shows helpful error
- [ ] Stale base_version shows helpful re-checkout suggestion
- [ ] Permission errors show clear message
- [ ] Concurrent edits to different regions merge cleanly
- [ ] Auto-detection of document ID works from `.cadmus/` metadata

## Files Modified

| File                       | Change                          |
| -------------------------- | ------------------------------- |
| `packages/cli/src/main.ts` | Add push command implementation |
| `packages/cli/src/api.ts`  | Add pushContent method          |
