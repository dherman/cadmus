# PR 3: React Editor Foundation — Design

## Purpose

Build the minimal React frontend that renders a Tiptap editor using the shared schema from `@cadmus/doc-schema`. This PR produces a working single-user editor — collaboration is wired up in PR 4. The goal is to validate that the schema renders correctly, the editor is functional, and the component architecture is ready for collaboration extensions.

## Architecture

```
web/
  src/
    App.tsx           # Root component, renders Editor
    Editor.tsx        # Tiptap editor wrapper
    main.tsx          # Vite entry point
  index.html
  vite.config.ts
  package.json
```

The frontend is a standard Vite + React application. It's minimal on purpose — no routing, no state management library, no component library. The editor is the entire application at this stage.

## Key Design Decisions

### Vite as Build Tool

Vite is the standard choice for new React projects. It provides fast HMR for development and efficient bundling for production. The `@vitejs/plugin-react` plugin handles JSX transforms.

### Editor Component Design

The `Editor` component wraps Tiptap's `useEditor` hook. It receives the extension array from `@cadmus/doc-schema` and renders the editor content area via Tiptap's `EditorContent` component.

The component is structured to accept collaboration extensions as props in the future, but for this PR it operates in standalone (non-collaborative) mode:

```tsx
// Simplified structure
function Editor() {
  const editor = useEditor({
    extensions: [...schemaExtensions],
    content: '<p>Start editing...</p>',
  });

  return <EditorContent editor={editor} />;
}
```

The `content` prop provides initial content for standalone mode. When collaboration is added in PR 4, this is replaced by the Yjs document state.

### Tiptap Version: v3

We use Tiptap v3 (currently the latest stable line). The `@tiptap/extension-markdown` package (used in the schema) was introduced in v3.7.0. All `@tiptap/*` packages should be pinned to the same minor version to avoid compatibility issues.

### Styling Approach

Minimal CSS for the editor, using a dedicated `editor.css` file. The styling goals for this PR are basic readability, not visual polish:

- The editor fills the viewport with comfortable padding.
- Prose elements (headings, lists, code blocks, blockquotes) have reasonable default styling.
- The `ProseMirror-focused` class shows a subtle focus indicator.
- Code blocks get a monospace font and background color.

We use a plain CSS file rather than Tailwind or CSS-in-JS. The editor's content area needs semantic element styling (`h1`, `h2`, `ul`, `blockquote`, etc.), which is more naturally expressed as element selectors than utility classes.

### No Routing

The app has one route: the editor. Document selection, dashboards, and multi-document support come in Milestone 2. For now, the app loads and displays a single editor instance.

## Content Rendering Verification

This PR should verify that all schema node types render correctly in the editor. The initial content (or a test fixture) should exercise:

- Headings (h1–h6)
- Paragraphs with inline marks (bold, italic, strike, code, links)
- Bullet lists and ordered lists (including nested)
- Code blocks (with and without language annotation)
- Blockquotes (including nested blocks)
- Images (block-level)
- Horizontal rules
- Hard breaks

## Dependencies

| Package                        | Purpose                              |
| ------------------------------ | ------------------------------------ |
| `react`, `react-dom`           | UI framework                         |
| `@tiptap/react`                | React bindings for Tiptap            |
| `@tiptap/pm`                   | ProseMirror peer dependency          |
| `@cadmus/doc-schema`           | Shared schema (workspace dependency) |
| `vite`, `@vitejs/plugin-react` | Build tooling                        |
| `typescript`                   | Type checking                        |
