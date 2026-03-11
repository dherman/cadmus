# PR 3: React Editor Foundation — Implementation Plan

## Prerequisites

- [ ] PR 1 merged (monorepo structure and `@cadmus/doc-schema` package exist)
- [ ] Node.js >= 18, pnpm >= 9

## Steps

### Step 1: Initialize the Web Package

- [ ] Create `web/package.json`:
  - `"name": "@cadmus/web"`, `"private": true`
  - Scripts: `"dev": "vite"`, `"build": "vite build"`, `"preview": "vite preview"`, `"typecheck": "tsc --noEmit"`
- [ ] Install dependencies:
  - Production: `react`, `react-dom`, `@tiptap/react`, `@tiptap/pm`
  - Dev: `vite`, `@vitejs/plugin-react`, `typescript`, `@types/react`, `@types/react-dom`
  - Workspace: `@cadmus/doc-schema` (via `"@cadmus/doc-schema": "workspace:*"`)
- [ ] Create `web/tsconfig.json` extending the root base config, with `"jsx": "react-jsx"` and appropriate `include`/`exclude` paths
- [ ] Create `web/vite.config.ts`:

  ```typescript
  import { defineConfig } from 'vite';
  import react from '@vitejs/plugin-react';

  export default defineConfig({
    plugins: [react()],
    server: {
      port: 5173,
    },
  });
  ```

### Step 2: Create the Entry Point

- [ ] Create `web/index.html` — standard HTML5 boilerplate with a `<div id="root">` and `<script type="module" src="/src/main.tsx">`. Set `<title>Cadmus</title>`
- [ ] Create `web/src/main.tsx`:

  ```tsx
  import { StrictMode } from 'react';
  import { createRoot } from 'react-dom/client';
  import { App } from './App';
  import './editor.css';

  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  ```

### Step 3: Implement the Editor Component

- [ ] Create `web/src/Editor.tsx`:
  - Import `useEditor`, `EditorContent` from `@tiptap/react`
  - Import `extensions` from `@cadmus/doc-schema`
  - Define the component:

    ```tsx
    import { useEditor, EditorContent } from '@tiptap/react';
    import { extensions } from '@cadmus/doc-schema';

    const INITIAL_CONTENT = `<h1>Welcome to Cadmus</h1>
    <p>Start editing this document. Try out <strong>bold</strong>,
    <em>italic</em>, and <code>inline code</code>.</p>`;

    export function Editor() {
      const editor = useEditor({
        extensions,
        content: INITIAL_CONTENT,
      });

      if (!editor) return null;

      return (
        <div className="editor-wrapper">
          <EditorContent editor={editor} />
        </div>
      );
    }
    ```

- [ ] Export the `editor` instance via a ref or context so the Toolbar (PR 5) and Collaboration (PR 4) components can access it

### Step 4: Create the App Shell

- [ ] Create `web/src/App.tsx`:

  ```tsx
  import { Editor } from './Editor';

  export function App() {
    return (
      <div className="app">
        <header className="app-header">
          <h1>Cadmus</h1>
        </header>
        <main className="app-main">
          <Editor />
        </main>
      </div>
    );
  }
  ```

### Step 5: Add Editor Styles

- [ ] Create `web/src/editor.css` with:
  - [ ] Global reset/normalization (minimal)
  - [ ] `.app` layout: full viewport, flex column
  - [ ] `.app-header`: simple top bar with the app name
  - [ ] `.editor-wrapper`: centered container with max-width (~720px), comfortable padding
  - [ ] `.ProseMirror` styles:
    - `outline: none` (remove browser default)
    - `min-height: 80vh`
    - Heading sizes (`h1` through `h6`)
    - `blockquote`: left border, padding, muted color
    - `pre > code`: monospace font, background color, padding, border-radius
    - `ul`, `ol`: proper list styling with indentation
    - `hr`: subtle horizontal line
    - `img`: max-width 100%, centered
    - `a`: colored, underlined
    - `code` (inline): background tint, padding, border-radius, monospace font
  - [ ] `.ProseMirror:focus` or `.ProseMirror-focused`: subtle outline or border change

### Step 6: Create a Rich Test Fixture

- [ ] Create `web/src/fixtures/sample-document.ts`:
  - Export an HTML string (or ProseMirror JSON) containing every node and mark type from the schema
  - This fixture is used for visual QA and can be swapped in as `content` during development to verify all types render correctly

### Step 7: Update Root Scripts

- [ ] Update root `package.json` scripts to include the web package:
  - `"dev:web": "pnpm -F @cadmus/web dev"`
  - `"build:web": "pnpm -F @cadmus/web build"`

## Verification

- [ ] `pnpm -F @cadmus/web dev` starts Vite and the editor loads in the browser
- [ ] Typing in the editor works. All keyboard shortcuts (Ctrl+B for bold, etc.) function
- [ ] Loading the sample fixture document displays all node types with correct styling
- [ ] `pnpm -F @cadmus/web build` produces a production build without errors
- [ ] `pnpm -F @cadmus/web typecheck` passes

## Files Created/Modified

```
web/package.json              (new)
web/tsconfig.json             (new)
web/vite.config.ts            (new)
web/index.html                (new)
web/src/main.tsx              (new)
web/src/App.tsx               (new)
web/src/Editor.tsx            (new)
web/src/editor.css            (new)
web/src/fixtures/sample-document.ts (new)
package.json                  (modified — add web scripts)
```
