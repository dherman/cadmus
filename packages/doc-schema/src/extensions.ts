/**
 * Cadmus Document Schema — Single Source of Truth
 *
 * This file defines the Tiptap extension set that constitutes the document schema.
 * It is imported by:
 *   - The web client (browser editor)
 *   - The Node sidecar (server-side markdown serialization)
 *
 * IMPORTANT: Any change to extensions or their configuration here affects both
 * the editor behavior and the markdown serialization output. Bump SCHEMA_VERSION
 * and add a migration function when modifying existing node/mark types.
 */

import StarterKit from '@tiptap/starter-kit';
import Image from '@tiptap/extension-image';
import { Markdown } from '@tiptap/markdown';
import type { Extensions } from '@tiptap/core';

/**
 * Schema version. Stored alongside each document in the database.
 * Bump this when changing existing node/mark types or their attributes.
 * Adding new node types is safe but should still bump the version.
 */
export const SCHEMA_VERSION = 1;

/**
 * Create the configured extension array for the editor and sidecar.
 *
 * Options allow the caller to customize behavior per-environment:
 * - The web client may enable undo/redo (or disable it when using Yjs collaboration)
 * - The sidecar doesn't need undo/redo at all
 */
export function createExtensions(
  options: {
    /** Disable built-in undo/redo — required when using Yjs collaboration */
    disableHistory?: boolean;
  } = {},
): Extensions {
  return [
    StarterKit.configure({
      // Disable underline — no clean markdown representation
      underline: false,

      // Disable history when collaborative editing is active (Yjs has its own)
      undoRedo: options.disableHistory ? false : undefined,

      heading: {
        levels: [1, 2, 3, 4, 5, 6],
      },

      codeBlock: {
        languageClassPrefix: 'language-',
      },

      // Link is included in StarterKit v3
      link: {
        openOnClick: false, // Don't navigate on click — editor, not reader
        autolink: true, // Auto-detect URLs as you type
      },
    }),

    Image.configure({
      inline: false, // Block-level images, not inline
      allowBase64: false, // Require URLs — no embedded data URIs
    }),

    Markdown.configure({
      indentation: {
        style: 'space',
        size: 2,
      },
      // MarkedJS options for parsing
      markedOptions: {
        gfm: true, // Enable GFM for strikethrough, tables (when added)
        breaks: false, // Don't convert \n to <br>
      },
    }),
  ];
}
