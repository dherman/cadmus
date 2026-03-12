import { useCallback, useEffect, useReducer } from 'react';
import type { Editor } from '@tiptap/core';

// Tiptap v3 extension commands (toggleBold, setLink, etc.) are typed via
// module augmentation in each extension package. Since we depend on
// @tiptap/starter-kit (which bundles them), the augmented types aren't
// picked up by TS. We cast chain()/can() to `any` as a standard workaround.
/* eslint-disable @typescript-eslint/no-explicit-any */

const isMac =
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
const mod = isMac ? '\u2318' : 'Ctrl';

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
      aria-pressed={isActive}
    >
      {label}
    </button>
  );
}

export function Toolbar({ editor }: { editor: Editor | null }) {
  const [, forceUpdate] = useReducer((x: number) => x + 1, 0);

  useEffect(() => {
    if (!editor) return;
    editor.on('transaction', forceUpdate);
    return () => {
      editor.off('transaction', forceUpdate);
    };
  }, [editor]);

  const handleLink = useCallback(() => {
    if (!editor) return;
    if (editor.isActive('link')) {
      (editor.chain().focus() as any).unsetLink().run();
      return;
    }
    const url = window.prompt('Enter URL:');
    if (url) {
      (editor.chain().focus() as any).setLink({ href: url }).run();
    }
  }, [editor]);

  if (!editor) return null;

  const can = editor.can() as any;

  return (
    <div className="toolbar" role="toolbar" aria-label="Formatting">
      <div className="toolbar-group">
        <ToolbarButton
          label={`Bold (${mod}+B)`}
          isActive={editor.isActive('bold')}
          onClick={() => (editor.chain().focus() as any).toggleBold().run()}
          disabled={!can.toggleBold()}
        />
        <ToolbarButton
          label={`Italic (${mod}+I)`}
          isActive={editor.isActive('italic')}
          onClick={() => (editor.chain().focus() as any).toggleItalic().run()}
          disabled={!can.toggleItalic()}
        />
        <ToolbarButton
          label={`Strikethrough (${mod}+Shift+S)`}
          isActive={editor.isActive('strike')}
          onClick={() => (editor.chain().focus() as any).toggleStrike().run()}
          disabled={!can.toggleStrike()}
        />
        <ToolbarButton
          label={`Code (${mod}+E)`}
          isActive={editor.isActive('code')}
          onClick={() => (editor.chain().focus() as any).toggleCode().run()}
          disabled={!can.toggleCode()}
        />
        <ToolbarButton
          label="Link"
          isActive={editor.isActive('link')}
          onClick={handleLink}
        />
      </div>
      <div className="toolbar-separator" />
      <div className="toolbar-group">
        <ToolbarButton
          label="H1"
          isActive={editor.isActive('heading', { level: 1 })}
          onClick={() =>
            (editor.chain().focus() as any).toggleHeading({ level: 1 }).run()
          }
          disabled={!can.toggleHeading({ level: 1 })}
        />
        <ToolbarButton
          label="H2"
          isActive={editor.isActive('heading', { level: 2 })}
          onClick={() =>
            (editor.chain().focus() as any).toggleHeading({ level: 2 }).run()
          }
          disabled={!can.toggleHeading({ level: 2 })}
        />
        <ToolbarButton
          label="H3"
          isActive={editor.isActive('heading', { level: 3 })}
          onClick={() =>
            (editor.chain().focus() as any).toggleHeading({ level: 3 }).run()
          }
          disabled={!can.toggleHeading({ level: 3 })}
        />
      </div>
      <div className="toolbar-separator" />
      <div className="toolbar-group">
        <ToolbarButton
          label="Bullet List"
          isActive={editor.isActive('bulletList')}
          onClick={() =>
            (editor.chain().focus() as any).toggleBulletList().run()
          }
          disabled={!can.toggleBulletList()}
        />
        <ToolbarButton
          label="Ordered List"
          isActive={editor.isActive('orderedList')}
          onClick={() =>
            (editor.chain().focus() as any).toggleOrderedList().run()
          }
          disabled={!can.toggleOrderedList()}
        />
        <ToolbarButton
          label="Blockquote"
          isActive={editor.isActive('blockquote')}
          onClick={() =>
            (editor.chain().focus() as any).toggleBlockquote().run()
          }
          disabled={!can.toggleBlockquote()}
        />
        <ToolbarButton
          label="Code Block"
          isActive={editor.isActive('codeBlock')}
          onClick={() =>
            (editor.chain().focus() as any).toggleCodeBlock().run()
          }
          disabled={!can.toggleCodeBlock()}
        />
        <ToolbarButton
          label="Horizontal Rule"
          isActive={false}
          onClick={() =>
            (editor.chain().focus() as any).setHorizontalRule().run()
          }
        />
      </div>
    </div>
  );
}
