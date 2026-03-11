# PR 1: Project Scaffolding & Shared Schema Package — Implementation Plan

## Prerequisites

- [ ] Node.js >= 18 installed
- [ ] pnpm >= 9 installed
- [ ] Rust toolchain installed (for future PRs, but ensure `server/` directory is ready)

## Steps

### Step 1: Initialize Monorepo Root

- [ ] Create `package.json` at the repo root with `"private": true` and workspace scripts (`build`, `lint`, `test`, `typecheck`)
- [ ] Create `pnpm-workspace.yaml`:
  ```yaml
  packages:
    - "packages/*"
    - "web"
  ```
- [ ] Create root `.gitignore` covering `node_modules/`, `dist/`, `target/` (Rust), `.env`, etc.
- [ ] Create root `tsconfig.base.json` with shared compiler options (`strict: true`, `moduleResolution: "bundler"`, `target: "ES2022"`)
- [ ] Install shared dev dependencies at root: `typescript`, `tsup` (for building packages), `vitest` (test runner), `eslint`, `prettier`

### Step 2: Create `packages/doc-schema/`

- [ ] Create `packages/doc-schema/package.json`:
  - `"name": "@cadmus/doc-schema"`
  - `"main": "./dist/index.cjs"`, `"module": "./dist/index.js"`, `"types": "./dist/index.d.ts"`
  - `"exports"` field with proper ESM/CJS conditions
  - `"scripts": { "build": "tsup", "test": "vitest run" }`
- [ ] Create `packages/doc-schema/tsconfig.json` extending the root base config
- [ ] Create `packages/doc-schema/tsup.config.ts` with `entry: ['src/index.ts']`, `format: ['esm', 'cjs']`, `dts: true`
- [ ] Install dependencies:
  - Production: `@tiptap/core`, `@tiptap/starter-kit`, `@tiptap/extension-image`, `@tiptap/extension-link`, `@tiptap/extension-markdown`, `@tiptap/pm`
  - Dev: `vitest`

### Step 3: Implement the Schema

- [ ] Create `packages/doc-schema/src/extensions.ts`:
  - Import `StarterKit` from `@tiptap/starter-kit`
  - Import `Image` from `@tiptap/extension-image`
  - Import `Link` from `@tiptap/extension-link`
  - Import `Markdown` from `@tiptap/extension-markdown`
  - Export `const SCHEMA_VERSION = 1`
  - Export `const extensions` as a configured array:
    ```typescript
    export const extensions = [
      StarterKit.configure({
        // All defaults are fine for launch schema.
        // Explicitly list what we're getting for clarity.
      }),
      Image.configure({
        inline: false, // Block-level images
      }),
      Link.configure({
        openOnClick: false,
        autolink: true,
      }),
      Markdown,
    ];
    ```
- [ ] Create `packages/doc-schema/src/index.ts`:
  - Re-export `extensions` and `SCHEMA_VERSION` from `./extensions`
  - Re-export relevant ProseMirror types: `Schema`, `Node`, `Mark` from `@tiptap/pm/model`

### Step 4: Write Tests

- [ ] Create `packages/doc-schema/src/__tests__/schema.test.ts` with the following tests:
  - [ ] **"produces a valid ProseMirror schema"** — Instantiate a headless Tiptap `Editor` with the extensions, extract `editor.schema`, assert it has the expected node names (`doc`, `paragraph`, `heading`, `codeBlock`, `blockquote`, `bulletList`, `orderedList`, `listItem`, `horizontalRule`, `hardBreak`, `image`, `text`) and mark names (`bold`, `italic`, `strike`, `code`, `link`)
  - [ ] **"schema does not include underline"** — Assert `editor.schema.marks.underline` is undefined
  - [ ] **"image is block-level"** — Assert `editor.schema.nodes.image.isBlock === true`
  - [ ] **"SCHEMA_VERSION is 1"** — Trivial but establishes the contract
  - [ ] **"schema spec snapshot"** — Serialize the schema spec (node names + content expressions, mark names + exclude sets) to a stable JSON and snapshot it. This catches accidental schema changes
  - [ ] **"round-trip ProseMirror JSON"** — Construct a document node using `schema.nodeFromJSON(...)` with all node/mark types present, convert back to JSON with `node.toJSON()`, and assert deep equality

### Step 5: Create Stub Directories for Future PRs

- [ ] Create `server/` directory with a minimal `Cargo.toml` and `src/main.rs` (just a `fn main() {}` placeholder). This ensures the monorepo structure is established
- [ ] Create `web/` directory with a `package.json` stub (will be fleshed out in PR 3)

### Step 6: CI Configuration

- [ ] Create `.github/workflows/ci.yml` (or equivalent) with:
  - [ ] **Lint:** `pnpm lint` at root
  - [ ] **Typecheck:** `pnpm typecheck` at root
  - [ ] **Test:** `pnpm test` at root (runs vitest across all packages)
  - [ ] **Build:** `pnpm build` at root (builds doc-schema)
  - [ ] Rust: `cargo check` and `cargo test` in `server/`

## Verification

- [ ] `pnpm install` succeeds from a clean state
- [ ] `pnpm -F @cadmus/doc-schema build` produces `dist/` with `.js`, `.cjs`, and `.d.ts` files
- [ ] `pnpm -F @cadmus/doc-schema test` passes all tests
- [ ] The schema snapshot test is committed and would catch any future accidental schema changes

## Files Created/Modified

```
package.json                          (new)
pnpm-workspace.yaml                   (new)
tsconfig.base.json                    (new)
.gitignore                            (new)
.github/workflows/ci.yml              (new)
packages/doc-schema/package.json      (new)
packages/doc-schema/tsconfig.json     (new)
packages/doc-schema/tsup.config.ts    (new)
packages/doc-schema/src/extensions.ts (new)
packages/doc-schema/src/index.ts      (new)
packages/doc-schema/src/__tests__/schema.test.ts (new)
server/Cargo.toml                     (new, stub)
server/src/main.rs                    (new, stub)
web/package.json                      (new, stub)
```
