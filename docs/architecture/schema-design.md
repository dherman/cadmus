# Schema Design

## Overview

The ProseMirror schema is a long-lived commitment: every document stored in the CRDT encodes its structure in terms of this schema. Changes to existing node/mark types require versioned migrations. This document defines the launch schema and the rules for evolving it.

## Design Principles

1. **Markdown round-trip fidelity.** Every construct in the schema must have a clean, deterministic markdown representation. If we can't serialize it to markdown and parse it back without loss, it doesn't belong in the schema.
2. **Minimal launch surface.** Start with CommonMark coverage. Defer constructs that don't round-trip cleanly or that add complexity without clear user demand.
3. **Schema versioning from day one.** Every schema change gets a version bump and a migration function, even if the migration is a no-op.

## Launch Schema (Version 1)

### Nodes

| Node | Group | Content | Marks | Notes |
|------|-------|---------|-------|-------|
| `doc` | (root) | `block+` | — | Root node. Document must contain ≥1 block. |
| `paragraph` | block | `inline*` | `_` (all) | Default block. Empty docs contain one empty paragraph. |
| `heading` | block | `inline*` | `_` (all) | Attrs: `level` (1–6). Serialize as ATX (`# Heading`). |
| `codeBlock` | block | `text*` | `''` (none) | Attrs: `language` (string, nullable). Serialize as fenced (`` ``` ``). |
| `blockquote` | block | `block+` | — | Serialize with `>` prefix. Must contain ≥1 block. |
| `bulletList` | block | `listItem+` | — | Serialize with `- ` prefix. |
| `orderedList` | block | `listItem+` | — | Attrs: `start` (number, default 1). Serialize as `1. `, `2. `, etc. |
| `listItem` | — | `paragraph block*` | — | Must start with paragraph. Can contain nested blocks (lists, code, etc.). |
| `horizontalRule` | block | (leaf) | — | Serialize as `---`. |
| `hardBreak` | inline | (leaf) | — | Serialize as `\` at end of line. |
| `image` | block | (leaf) | — | Attrs: `src`, `alt`, `title`. Block-level (not inline). Serialize as `![alt](src "title")`. |
| `text` | inline | — | `_` (all) | Base text node. |

### Marks

| Mark | Excludes | Serialize | Notes |
|------|----------|-----------|-------|
| `bold` | — | `**text**` | Never `__text__`. |
| `italic` | — | `*text*` | Never `_text_`. |
| `code` | `_` (all) | `` `text` `` | Excludes all other marks (matches markdown behavior). |
| `link` | — | `[text](href "title")` | Attrs: `href`, `title` (nullable). Non-inclusive (doesn't extend on typing). |
| `strike` | — | `~~text~~` | GFM extension. Requires `gfm: true` in MarkedJS config. |

### Excluded from launch

- **Underline** — no markdown representation. Would serialize as `<u>text</u>` (raw HTML) or be silently dropped. Creates lossy round-trips.
- **Tables** — planned for soon after launch (v2). Markdown table support is limited (one paragraph per cell, no colspan). We'll constrain the editor to match markdown's capabilities.
- **Task lists** — planned for v2. GFM `- [ ]` / `- [x]` syntax.
- **Math/LaTeX, footnotes, embeds** — deferred until user demand.

## Canonical Markdown Style

The serializer produces deterministic output. CLI users and agents should expect this format from checkouts and match it for minimal diffs on push.

| Construct | Canonical Form |
|-----------|---------------|
| Bold | `**text**` |
| Italic | `*text*` |
| Inline code | `` `text` `` |
| Heading | `# ATX style` (with space after `#`) |
| Code block | Triple-backtick fenced (never indented) |
| Bullet list | `- item` (hyphen, never `*` or `+`) |
| Ordered list | `1. item`, `2. item`, etc. |
| Blockquote | `> text` |
| Hard break | `\` at end of line (not two trailing spaces) |
| Horizontal rule | `---` |
| Image | `![alt](src "title")` |
| Link | `[text](url "title")` |
| Indentation | 2 spaces for nesting |

## Schema Versioning and Migration

Each schema version is identified by an integer (`SCHEMA_VERSION` in `packages/doc-schema/src/extensions.ts`). The version is stored alongside each document in the database.

### Migration rules

- **Adding a new node/mark type** (e.g., tables): safe. Existing documents don't contain the new type. Bump version, write a no-op migration function.
- **Adding an attribute to an existing type** (e.g., `width` on images): requires walking the document tree to add default values. Bump version, write migration.
- **Removing a node/mark type**: hardest case. Must decide what happens to existing content (convert to another type? strip it?). Bump version, write migration that transforms or removes instances.
- **Never** modify an attribute's semantics without a version bump.

### Migration execution

On document load, the server checks the stored schema version. If it's behind the current version, it runs migrations sequentially (v1→v2, v2→v3, etc.) before initializing the Yrs document. Migrations operate on ProseMirror JSON (the Tiptap document format).

## Shared Schema Package

The schema definition lives in `packages/doc-schema/` and is imported by both the web client and the sidecar. This is the single source of truth — the schema is shared code, not duplicated code. See the [Node Sidecar](node-sidecar.md) document for why this matters.
