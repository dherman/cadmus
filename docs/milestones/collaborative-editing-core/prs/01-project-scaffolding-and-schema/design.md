# PR 1: Project Scaffolding & Shared Schema Package — Design

## Purpose

Establish the monorepo structure, tooling, and the shared `doc-schema` package that both the frontend and (in future milestones) the Node sidecar will consume. This PR produces no running application, but it creates the foundation that every subsequent PR builds on.

## Design Decisions

### Monorepo with pnpm Workspaces

The project uses a pnpm workspace monorepo. This is the simplest approach for sharing a TypeScript package (`doc-schema`) between the web frontend and the sidecar, while keeping the Rust server in the same repository for atomic commits across the stack.

pnpm was chosen over npm/yarn for its strict dependency resolution (no phantom dependencies) and efficient disk usage via content-addressable storage. The workspace layout:

```
cadmus/
  packages/
    doc-schema/       # Shared schema — consumed by web/ and sidecar/
  web/                # React frontend (future PR)
  server/             # Rust server (future PR)
  package.json        # Workspace root
  pnpm-workspace.yaml
```

### doc-schema Package Design

The `doc-schema` package exports a configured array of Tiptap extensions that defines the document schema. It is the single source of truth for what nodes and marks exist, how they serialize to markdown, and what the schema version is.

The package exposes:

- `extensions` — the Tiptap extension array, ready to pass to `new Editor({ extensions })`.
- `SCHEMA_VERSION` — an integer (starting at 1) stored alongside documents for migration tracking.
- Type re-exports from `@tiptap/pm/model` for consumers that need ProseMirror types (`Schema`, `Node`, `Mark`).

The extensions array is built from Tiptap's `StarterKit` with modifications to match the launch schema defined in the [Schema Design](../../../../architecture/schema-design.md) doc:

- StarterKit provides: `Document`, `Paragraph`, `Text`, `Heading`, `CodeBlock`, `Blockquote`, `BulletList`, `OrderedList`, `ListItem`, `HorizontalRule`, `HardBreak`, `Bold`, `Italic`, `Strike`, `Code`.
- StarterKit's `Underline` is excluded (no clean markdown round-trip).
- `Image` is added separately (`@tiptap/extension-image`) as a block-level node.
- `Link` is added separately (`@tiptap/extension-link`) with `autolink: true` and `openOnClick: false`.
- `Markdown` extension (`@tiptap/extension-markdown`) is added for serialization/parsing support.

### Why Not Include Collaboration Extensions Here

The collaboration extensions (`@tiptap/extension-collaboration`, `@tiptap/extension-collaboration-cursor`) are intentionally excluded from `doc-schema`. They are editor-runtime concerns, not schema concerns. The sidecar needs the schema but not collaboration — bundling them together would pull in `yjs` as a dependency of the sidecar unnecessarily.

### TypeScript Configuration

The package uses TypeScript with `"moduleResolution": "bundler"` and emits both ESM and CJS (via `tsup` or `unbuild`) so it can be consumed by the Vite-based frontend and the Node sidecar alike.

## Schema Mapping Reference

This table maps from the architecture spec to the specific Tiptap extensions configured in this PR:

| Schema Spec | Tiptap Extension | Source | Config Notes |
|-------------|-----------------|--------|--------------|
| `doc` | `Document` | StarterKit | — |
| `paragraph` | `Paragraph` | StarterKit | — |
| `heading` | `Heading` | StarterKit | `levels: [1, 2, 3, 4, 5, 6]` |
| `codeBlock` | `CodeBlock` | StarterKit | `languageClassPrefix: 'language-'` |
| `blockquote` | `Blockquote` | StarterKit | — |
| `bulletList` | `BulletList` | StarterKit | — |
| `orderedList` | `OrderedList` | StarterKit | — |
| `listItem` | `ListItem` | StarterKit | — |
| `horizontalRule` | `HorizontalRule` | StarterKit | — |
| `hardBreak` | `HardBreak` | StarterKit | — |
| `text` | `Text` | StarterKit | — |
| `bold` | `Bold` | StarterKit | — |
| `italic` | `Italic` | StarterKit | — |
| `strike` | `Strike` | StarterKit | — |
| `code` | `Code` | StarterKit | — |
| `image` | `Image` | `@tiptap/extension-image` | `inline: false` (block-level) |
| `link` | `Link` | `@tiptap/extension-link` | `openOnClick: false`, `autolink: true` |

## Testing Strategy

- Unit tests verifying the extension array produces a valid ProseMirror schema.
- Snapshot test of the schema spec (node names, mark names, content expressions) to catch accidental schema changes.
- Round-trip test: construct a ProseMirror document programmatically with all node/mark types, serialize to JSON, parse back, and assert equality.
