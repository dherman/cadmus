#!/usr/bin/env node

/**
 * Cadmus CLI
 *
 * Commands:
 *   cadmus auth login              Authenticate and store credentials
 *   cadmus auth status             Show current login state
 *   cadmus docs list               List accessible documents
 *   cadmus checkout <doc-id>       Download document as markdown
 *   cadmus push <doc-id> <file>    Push local changes back to server
 *   cadmus comment <doc-id>        Add a comment to a document
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { Command } from 'commander';
import chalk from 'chalk';
import ora from 'ora';
import { CadmusClient, ApiError, type DocumentSummary, type PushResponse } from './api.js';
import { loginCommand, statusCommand } from './auth.js';
import {
  loadCredentials,
  saveCheckoutMetadata,
  loadCheckoutMetadata,
  getMetadataDir,
  type CheckoutMetadata,
} from './config.js';

// --- Helpers ---

// When run via `pnpm -F @cadmus/cli dev`, cwd is packages/cli/ not the
// user's shell directory. INIT_CWD (set by pnpm/npm during lifecycle
// scripts) preserves the original directory. Fall back to process.cwd()
// for direct invocations (e.g. globally installed CLI).
const userCwd = process.env.INIT_CWD || process.cwd();

async function getAuthenticatedClient(): Promise<CadmusClient> {
  const creds = loadCredentials();
  if (!creds) {
    console.error(chalk.red('Error:') + " Not logged in. Run 'cadmus auth login' first.");
    return process.exit(1);
  }
  return new CadmusClient(creds.server, async () => creds.access_token);
}

function formatRelativeTime(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffMs = now - then;
  const diffSec = Math.floor(diffMs / 1000);

  if (diffSec < 60) return 'just now';
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin} minute${diffMin === 1 ? '' : 's'} ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr} hour${diffHr === 1 ? '' : 's'} ago`;
  const diffDay = Math.floor(diffHr / 24);
  if (diffDay < 30) return `${diffDay} day${diffDay === 1 ? '' : 's'} ago`;
  const diffMonth = Math.floor(diffDay / 30);
  return `${diffMonth} month${diffMonth === 1 ? '' : 's'} ago`;
}

function printTable(rows: Record<string, string>[]): void {
  if (rows.length === 0) return;
  const keys = Object.keys(rows[0]);
  const widths = keys.map((k) => Math.max(k.length, ...rows.map((r) => (r[k] ?? '').length)));

  // Header
  const header = keys.map((k, i) => k.padEnd(widths[i])).join('  ');
  const separator = widths.map((w) => '─'.repeat(w)).join('  ');
  console.log(header);
  console.log(separator);

  // Rows
  for (const row of rows) {
    const line = keys.map((k, i) => (row[k] ?? '').padEnd(widths[i])).join('  ');
    console.log(line);
  }
}

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/(^-|-$)/g, '');
}

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

async function resolveDocId(client: CadmusClient, shortId: string): Promise<string> {
  if (UUID_RE.test(shortId)) {
    return shortId;
  }

  const docs = await client.listDocuments();
  const matches = docs.filter((d) => d.id.toLowerCase().startsWith(shortId.toLowerCase()));

  if (matches.length === 0) {
    console.error(chalk.red('Error:') + ` Document not found: ${shortId}`);
    return process.exit(1);
  }
  if (matches.length > 1) {
    console.error(chalk.red('Error:') + ` Ambiguous ID prefix '${shortId}'. Did you mean:`);
    for (const m of matches) {
      console.error(`  ${m.id}  ${m.title}`);
    }
    return process.exit(1);
  }
  return matches[0].id;
}

function autoDetectDocId(filePath: string): string | null {
  const fileDir = path.dirname(path.resolve(filePath));
  const metaDir = getMetadataDir(fileDir);
  if (!fs.existsSync(metaDir)) return null;

  const files = fs.readdirSync(metaDir).filter((f) => f.endsWith('.json'));
  for (const f of files) {
    const meta = JSON.parse(fs.readFileSync(path.join(metaDir, f), 'utf-8')) as CheckoutMetadata;
    if (path.resolve(fileDir, meta.file) === path.resolve(filePath)) {
      return meta.doc_id;
    }
  }
  return null;
}

interface ChangeSummary {
  steps_applied: number;
  nodes_added: number;
  nodes_removed: number;
  nodes_modified: number;
}

function displayChangeSummary(summary: ChangeSummary): void {
  const parts = [];
  if (summary.nodes_added > 0) parts.push(`${summary.nodes_added} additions`);
  if (summary.nodes_removed > 0) parts.push(`${summary.nodes_removed} removals`);
  if (summary.nodes_modified > 0) parts.push(`${summary.nodes_modified} modifications`);

  console.log(`  Changes: ${summary.steps_applied} steps applied (${parts.join(', ')})`);
}

function displayDryRunResult(result: PushResponse, meta: CheckoutMetadata): void {
  console.log(`Preview of changes to "${meta.title}":\n`);

  if (result.diff) {
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
  if (result.changes_summary) {
    displayChangeSummary(result.changes_summary);
  }
  console.log();
  console.log('To apply these changes, run without --dry-run.');
}

function displayPushResult(result: PushResponse, meta: CheckoutMetadata): void {
  console.log(chalk.green('✓') + ` Pushed changes to "${meta.title}"`);
  console.log(`  New version: ${result.version}`);
  if (result.changes_summary) {
    displayChangeSummary(result.changes_summary);
  }
  console.log('  Checkout metadata updated.');
}

function handleError(err: unknown): never {
  if (err instanceof ApiError) {
    switch (err.status) {
      case 401:
        console.error(
          chalk.red('Error:') + " Session expired. Run 'cadmus auth login' to re-authenticate.",
        );
        break;
      case 403:
        console.error(chalk.red('Error:') + " You don't have access to this document.");
        break;
      case 404:
        console.error(chalk.red('Error:') + ' Document not found.');
        break;
      default:
        console.error(chalk.red('Error:') + ` ${err.message}`);
    }
  } else if (err instanceof Error) {
    console.error(chalk.red('Error:') + ` ${err.message}`);
  }
  return process.exit(1);
}

// --- Program ---

const program = new Command();

program
  .name('cadmus')
  .description('Cadmus CLI — collaborative document editing from the command line')
  .version('0.1.0');

// --- Auth ---

const auth = program.command('auth').description('Authentication commands');

auth
  .command('login')
  .description('Authenticate with the Cadmus server')
  .option('-s, --server <url>', 'Server URL', 'http://localhost:8080')
  .option('-t, --token <token>', 'Authenticate with an agent token')
  .action(async (options) => {
    try {
      await loginCommand(options);
    } catch (err) {
      handleError(err);
    }
  });

auth
  .command('status')
  .description('Show current authentication status')
  .action(async () => {
    try {
      await statusCommand();
    } catch (err) {
      handleError(err);
    }
  });

// --- Documents ---

const docs = program.command('docs').description('Document management');

docs
  .command('list')
  .description('List accessible documents')
  .action(async () => {
    try {
      const client = await getAuthenticatedClient();
      const spinner = ora('Loading documents...').start();
      const documents = await client.listDocuments();
      spinner.stop();

      if (documents.length === 0) {
        console.log('No documents found.');
        return;
      }

      const rows = documents.map((d: DocumentSummary) => ({
        ID: d.id,
        Title: d.title,
        Updated: formatRelativeTime(d.updated_at),
      }));
      printTable(rows);
    } catch (err) {
      handleError(err);
    }
  });

// --- Checkout ---

program
  .command('checkout <doc-id>')
  .description('Download a document as markdown')
  .option('-o, --output <path>', 'Output file path')
  .action(async (docId: string, _options: unknown, command: Command) => {
    try {
      const opts = command.opts<{ output?: string }>();
      const client = await getAuthenticatedClient();

      const spinner = ora('Resolving document...').start();
      const fullId = await resolveDocId(client, docId);

      spinner.text = 'Checking out...';
      const [content, doc] = await Promise.all([
        client.getDocumentContent(fullId, 'markdown'),
        client.getDocument(fullId),
      ]);
      spinner.stop();

      const outputPath = path.resolve(userCwd, opts.output || `./${slugify(doc.title)}.md`);

      fs.writeFileSync(outputPath, content.content, 'utf-8');

      const fileDir = path.dirname(outputPath);
      saveCheckoutMetadata(fileDir, {
        doc_id: fullId,
        version: content.version,
        title: doc.title,
        checked_out_at: new Date().toISOString(),
        file: path.basename(outputPath),
        server: client.server,
      });

      console.log(chalk.green('✓') + ` Checked out "${doc.title}" (version ${content.version})`);
      console.log(`  Written to ${outputPath}`);
      console.log(`  Metadata saved to ${path.join(fileDir, '.cadmus', fullId + '.json')}`);
    } catch (err) {
      handleError(err);
    }
  });

// --- Push ---

program
  .command('push [doc-id] [file]')
  .description('Push local markdown changes to the server')
  .option('--dry-run', 'Preview changes without applying')
  .option('--force', 'Force push even with large diffs')
  .action(
    async (
      docId: string | undefined,
      file: string | undefined,
      _options: unknown,
      command: Command,
    ) => {
      try {
        const opts = command.opts<{ dryRun?: boolean; force?: boolean }>();

        // Support single-argument form: cadmus push ./file.md (auto-detect doc ID)
        if (docId && !file) {
          // The single argument might be a file path — try auto-detection
          const filePath = path.resolve(userCwd, docId);
          if (fs.existsSync(filePath)) {
            const detected = autoDetectDocId(filePath);
            if (detected) {
              file = docId;
              docId = detected;
            } else {
              console.error(
                chalk.red('Error:') +
                  ` No checkout metadata found for ${filePath}.` +
                  ` Run 'cadmus checkout <doc-id>' first.`,
              );
              process.exit(1);
            }
          } else {
            console.error(chalk.red('Error:') + ` File not found: ${docId}`);
            process.exit(1);
          }
        }

        if (!docId || !file) {
          console.error(chalk.red('Error:') + ' Usage: cadmus push <doc-id> <file>');
          process.exit(1);
        }

        const client = await getAuthenticatedClient();

        // Resolve doc ID
        const fullId = await resolveDocId(client, docId);

        // Resolve file path relative to user's working directory
        const filePath = path.resolve(userCwd, file);

        // Read checkout metadata
        const fileDir = path.dirname(filePath);
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
        if (!fs.existsSync(filePath)) {
          console.error(chalk.red('Error:') + ` File not found: ${file}`);
          process.exit(1);
        }
        const content = fs.readFileSync(filePath, 'utf-8');

        // Push
        const mode = opts.dryRun ? 'Previewing' : 'Pushing';
        const spinner = ora(`${mode} changes...`).start();

        const result = await client.pushContent(
          fullId,
          {
            base_version: meta.version,
            format: 'markdown',
            content,
          },
          opts.dryRun,
        );

        spinner.stop();

        if (opts.dryRun) {
          displayDryRunResult(result, meta);
        } else {
          displayPushResult(result, meta);

          // Update checkout metadata with new version
          if (result.version) {
            saveCheckoutMetadata(fileDir, {
              ...meta,
              version: result.version,
              checked_out_at: new Date().toISOString(),
            });
          }
        }
      } catch (err) {
        if (err instanceof ApiError) {
          switch (err.status) {
            case 404:
              console.error(
                chalk.red('Error:') +
                  ' Base version not found.' +
                  ` Re-checkout with 'cadmus checkout ${docId}'.`,
              );
              break;
            case 403:
              console.error(chalk.red('Error:') + " You don't have edit access to this document.");
              break;
            case 422:
              console.error(chalk.red('Error:') + ' Failed to parse markdown: ' + err.body);
              break;
            default:
              handleError(err);
          }
          process.exit(1);
        }
        handleError(err as Error);
      }
    },
  );

// --- Comments (stub — deferred) ---

program
  .command('comment <doc-id>')
  .description('Add a comment to a document')
  .option('--lines <range>', 'Line range (e.g., 45-52)')
  .option('-m, --message <text>', 'Comment text')
  .action(async (_docId: string, _options) => {
    console.log('Not yet implemented');
    process.exit(1);
  });

// Strip leading "--" from argv that pnpm/npm inject when forwarding args
// (e.g. `pnpm dev -- checkout -o foo.md`), since Commander treats "--"
// as end-of-options and would ignore flags after it.
const argv = process.argv.slice(0);
const ddIndex = argv.indexOf('--');
if (ddIndex !== -1) {
  argv.splice(ddIndex, 1);
}

program.parse(argv);
