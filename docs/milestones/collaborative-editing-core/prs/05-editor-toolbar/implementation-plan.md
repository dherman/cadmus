# PR 5: Editor Toolbar — Implementation Plan

## Prerequisites

- [x] PR 3 merged (Editor component exists and renders Tiptap editor)
- [x] Can be developed in parallel with PR 2 (Rust server) since it only depends on the frontend

## Steps

### Step 1: Create the Toolbar Component

- [x] Create `web/src/Toolbar.tsx`
- [x] Accept `editor: Editor | null` as a prop
- [x] If `editor` is null, render nothing
- [x] Subscribe to editor transaction events to track active states
- [x] Use `useCallback` for button click handlers to avoid re-creating functions
- [x] Render button groups:

  ```tsx
  export function Toolbar({ editor }: { editor: Editor | null }) {
    if (!editor) return null;

    return (
      <div className="toolbar" role="toolbar" aria-label="Formatting">
        <div className="toolbar-group">
          <ToolbarButton
            label="Bold"
            isActive={editor.isActive('bold')}
            onClick={() => editor.chain().focus().toggleBold().run()}
            disabled={!editor.can().toggleBold()}
          />
          {/* ... more mark buttons */}
        </div>
        <div className="toolbar-separator" />
        <div className="toolbar-group">{/* Block buttons */}</div>
      </div>
    );
  }
  ```

### Step 2: Create the ToolbarButton Sub-component

- [x] Create `ToolbarButton` (within `Toolbar.tsx` or as a separate file):
  ```tsx
  function ToolbarButton({
    label,
    isActive,
    onClick,
    disabled,
  }: {
    label: string;
    isActive: boolean;
    onClick: () => void;
    disabled?: boolean;
  }) {
    return (
      <button
        className={`toolbar-btn ${isActive ? 'is-active' : ''}`}
        onClick={onClick}
        disabled={disabled}
        title={label}
        type="button"
      >
        {label}
      </button>
    );
  }
  ```

### Step 3: Implement All Toolbar Buttons

- [x] **Mark buttons:** Bold, Italic, Strikethrough, Code — each calls `editor.chain().focus().toggle{Mark}().run()`
- [x] **Link button:** Separate handler:
  ```typescript
  const handleLink = useCallback(() => {
    if (editor.isActive('link')) {
      editor.chain().focus().unsetLink().run();
      return;
    }
    const url = window.prompt('Enter URL:');
    if (url) {
      editor.chain().focus().setLink({ href: url }).run();
    }
  }, [editor]);
  ```
- [x] **Block buttons:** Heading 1, 2, 3, Bullet List, Ordered List, Blockquote, Code Block — each calls the appropriate `toggle` command
- [x] **Horizontal Rule:** `editor.chain().focus().setHorizontalRule().run()`. No active state

### Step 4: Force Re-render on Editor State Changes

- [x] Ensure the toolbar updates when the selection changes (to reflect active states). Tiptap's `useEditor` already handles this when the editor instance is passed through React state. If additional granularity is needed, use:
  ```typescript
  const [, forceUpdate] = useReducer((x) => x + 1, 0);
  useEffect(() => {
    editor.on('transaction', forceUpdate);
    return () => {
      editor.off('transaction', forceUpdate);
    };
  }, [editor]);
  ```

### Step 5: Integrate Toolbar into the App

- [x] Modify `web/src/App.tsx` (or `Editor.tsx`):
  - Render `<Toolbar editor={editor} />` above the `<EditorContent />`
  - The editor instance needs to be accessible by both components. If the editor is created inside `Editor.tsx`, either lift it to the parent or expose it via a ref/context
  - Recommended approach: create the editor in a parent component or custom hook, and pass it to both `Toolbar` and `EditorContent`

### Step 6: Add Toolbar Styles

- [x] Add to `web/src/editor.css` (or create `web/src/toolbar.css`):
  - [x] `.toolbar`: flex row, border-bottom, padding, sticky top (so it stays visible while scrolling)
  - [x] `.toolbar-group`: flex row, gap between buttons
  - [x] `.toolbar-separator`: vertical line between groups
  - [x] `.toolbar-btn`: padding, border-radius, no border, cursor pointer, background transparent
  - [x] `.toolbar-btn:hover`: subtle background
  - [x] `.toolbar-btn.is-active`: distinct background color (e.g., light blue or gray)
  - [x] `.toolbar-btn:disabled`: opacity 0.4, cursor not-allowed

### Step 7: Accessibility

- [x] Add `role="toolbar"` and `aria-label="Formatting"` to the toolbar container
- [x] Add `aria-pressed={isActive}` to toggle buttons
- [x] Add `title` attributes with the button name + keyboard shortcut (e.g., `"Bold (Ctrl+B)"`)

## Verification

- [x] Each toolbar button applies the correct formatting when clicked
- [x] Active state highlights correctly as the cursor moves through formatted text
- [x] Clicking Bold on selected text toggles it (applies and removes)
- [x] Link button prompts for URL and applies/removes links
- [x] Toolbar does not interfere with keyboard shortcuts
- [x] Buttons are correctly disabled when their action is not available (e.g., Bold disabled inside a code block)

## Files Created/Modified

```
web/src/Toolbar.tsx           (new)
web/src/Editor.tsx            (modified — expose editor instance)
web/src/App.tsx               (modified — render Toolbar)
web/src/editor.css            (modified — toolbar styles)
```
