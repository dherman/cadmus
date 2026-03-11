#!/usr/bin/env node

/**
 * Cadmus CLI
 *
 * Commands:
 *   cadmus auth login              Authenticate and store credentials
 *   cadmus docs list               List accessible documents
 *   cadmus checkout <doc-id>       Download document as markdown
 *   cadmus push <doc-id> <file>    Push local changes back to server
 *   cadmus comment <doc-id>        Add a comment to a document
 */

import { Command } from 'commander';

const program = new Command();

program
  .name('cadmus')
  .description('Cadmus CLI — collaborative document editing from the command line')
  .version('0.1.0');

// --- Auth ---

program
  .command('auth')
  .description('Authentication commands')
  .command('login')
  .description('Authenticate with the Cadmus server')
  .option('-s, --server <url>', 'Server URL', 'http://localhost:8080')
  .action(async (options) => {
    // TODO: Prompt for credentials, exchange for token, store in ~/.config/cadmus/credentials
    console.log(`Authenticating with ${options.server}...`);
    console.log('Not yet implemented');
  });

// --- Documents ---

const docs = program.command('docs').description('Document management');

docs
  .command('list')
  .description('List accessible documents')
  .action(async () => {
    // TODO: GET /api/docs, display as table
    console.log('Not yet implemented');
  });

// --- Checkout ---

program
  .command('checkout <doc-id>')
  .description('Download a document as markdown')
  .option('-o, --output <path>', 'Output file path')
  .action(async (docId, _options) => {
    // TODO:
    // 1. GET /api/docs/{id}/content?format=markdown
    // 2. Write markdown to output file
    // 3. Record version in .cadmus/{doc-id}.json
    console.log(`Checking out document ${docId}...`);
    console.log('Not yet implemented');
  });

// --- Push ---

program
  .command('push <doc-id> <file>')
  .description('Push local markdown changes to the server')
  .option('--dry-run', 'Preview changes without applying')
  .option('--force', 'Force push even with large diffs')
  .action(async (docId, file, options) => {
    // TODO:
    // 1. Read .cadmus/{doc-id}.json for base_version
    // 2. Read the local markdown file
    // 3. POST /api/docs/{id}/content (with dry_run flag if set)
    // 4. Display result (diff preview or applied changes)
    const mode = options.dryRun ? '(dry run)' : '';
    console.log(`Pushing ${file} to document ${docId} ${mode}...`);
    console.log('Not yet implemented');
  });

// --- Comments ---

program
  .command('comment <doc-id>')
  .description('Add a comment to a document')
  .option('--lines <range>', 'Line range (e.g., 45-52)')
  .option('-m, --message <text>', 'Comment text')
  .action(async (docId, _options) => {
    // TODO:
    // 1. Parse line range to character offsets
    // 2. POST /api/docs/{id}/comments
    console.log(`Adding comment to document ${docId}...`);
    console.log('Not yet implemented');
  });

program.parse();
