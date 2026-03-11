# PR 5: Editor Toolbar — Design

## Purpose

Add a formatting toolbar to the editor with buttons for all marks and block types in the launch schema. The toolbar gives users a discoverable way to apply formatting beyond keyboard shortcuts and validates that the schema's editing commands work correctly through the Tiptap API.

## Scope

The toolbar covers all mark and node types in the launch schema. It does not include document-level actions (save, export, share) — those belong to later milestones.

### Toolbar Buttons

**Marks (inline formatting):**

| Button        | Action                  | Keyboard Shortcut | Active State                              |
| ------------- | ----------------------- | ----------------- | ----------------------------------------- |
| Bold          | `toggleBold`            | Ctrl+B            | Highlighted when cursor is in bold text   |
| Italic        | `toggleItalic`          | Ctrl+I            | Highlighted when cursor is in italic text |
| Strikethrough | `toggleStrike`          | Ctrl+Shift+X      | Highlighted when cursor is in struck text |
| Code          | `toggleCode`            | Ctrl+E            | Highlighted when cursor is in inline code |
| Link          | `setLink` / `unsetLink` | Ctrl+K            | Highlighted when cursor is on a link      |

**Blocks (structural formatting):**

| Button          | Action                        | Notes                                                                 |
| --------------- | ----------------------------- | --------------------------------------------------------------------- |
| Heading 1       | `toggleHeading({ level: 1 })` | Active when cursor is in h1                                           |
| Heading 2       | `toggleHeading({ level: 2 })` | Active when cursor is in h2                                           |
| Heading 3       | `toggleHeading({ level: 3 })` | h4–h6 omitted from toolbar (accessible via markdown shortcuts `####`) |
| Bullet List     | `toggleBulletList`            | Active when cursor is in a bullet list                                |
| Ordered List    | `toggleOrderedList`           | Active when cursor is in an ordered list                              |
| Blockquote      | `toggleBlockquote`            | Active when cursor is in a blockquote                                 |
| Code Block      | `toggleCodeBlock`             | Active when cursor is in a code block                                 |
| Horizontal Rule | `setHorizontalRule`           | Insert only, no toggle state                                          |

**Not in toolbar (accessible via other means):**

- Image insertion: deferred to a more complete media handling story.
- Hard break: entered via Shift+Enter.
- Headings 4–6: accessible via `####`, `#####`, `######` markdown input rules.

## Key Design Decisions

### Active State Tracking

Each button reflects whether its corresponding mark or node is active at the current cursor position. Tiptap provides `editor.isActive('bold')`, `editor.isActive('heading', { level: 1 })`, etc. The toolbar re-renders on every `selectionUpdate` and `transaction` event from the editor.

To avoid excessive re-renders, the toolbar subscribes to editor events via `editor.on('transaction', ...)` and reads active states in batch, updating a single state object.

### Link Button Behavior

The Link button is more complex than toggle buttons. When clicked:

- If the cursor is on an existing link: remove the link (`unsetLink`).
- If text is selected and not a link: prompt for a URL (via a small inline popover or `window.prompt` for this milestone) and apply `setLink({ href })`.
- If no text is selected: do nothing (links require selected text as anchor).

A full link editing popover (with URL preview, edit, open-in-new-tab) is a polish item. For this milestone, `window.prompt` is acceptable.

### Toolbar Positioning

The toolbar is a fixed bar between the header and the editor content area. It does not float or follow the cursor. This is the simplest implementation and works well for a document editor (as opposed to a block editor where floating toolbars are more common).

### Styling

Toolbar buttons are plain HTML `<button>` elements styled with CSS. Active buttons have a distinct background color. Disabled buttons (e.g., when the editor is not focused) are dimmed. Icons can be added later; for this PR, text labels or simple Unicode symbols are sufficient.

## Component API

```tsx
interface ToolbarProps {
  editor: Editor | null;
}

function Toolbar({ editor }: ToolbarProps) {
  // Reads editor state, renders buttons, calls editor.chain().focus()... commands.
}
```

The `editor` instance is passed as a prop from the parent component. The toolbar is a controlled component — it doesn't own any state beyond what it reads from the editor.
