/**
 * Compute the ProseMirror Step sequence that transforms old_doc into new_doc.
 *
 * This is the core of the CLI push merge strategy. The Rust server sends two
 * document states (the checkout-time version and the pushed version), and this
 * module returns the Steps needed to get from one to the other.
 *
 * The Steps are then translated into Yrs operations on the Rust side.
 */

import { Editor } from '@tiptap/core';
import { createExtensions } from '@cadmus/doc-schema';
import { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { recreateTransform } from '@fellow/prosemirror-recreate-transform';
import type { JSONContent } from '@tiptap/core';

export function diff(oldDoc: JSONContent, newDoc: JSONContent): object[] {
  const editor = new Editor({
    extensions: createExtensions({ disableHistory: true }),
    content: oldDoc,
  });

  const schema = editor.schema;
  const oldNode = ProseMirrorNode.fromJSON(schema, oldDoc);
  const newNode = ProseMirrorNode.fromJSON(schema, newDoc);

  const transform = recreateTransform(oldNode, newNode);
  const steps = transform.steps.map((step) => step.toJSON());

  editor.destroy();
  return steps;
}
