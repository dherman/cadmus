/**
 * Compute the ProseMirror Step sequence that transforms old_doc into new_doc.
 *
 * This is the core of the CLI push merge strategy. The Rust server sends two
 * document states (the checkout-time version and the pushed version), and this
 * module returns the Steps needed to get from one to the other.
 *
 * The Steps are then translated into Yrs operations on the Rust side.
 *
 * TODO: Implement using prosemirror-recreate-transform or equivalent.
 * This is a placeholder that returns the Step JSON structure.
 */

import { Editor } from '@tiptap/core';
import { createExtensions } from '@cadmus/doc-schema';
import { Node as ProseMirrorNode } from '@tiptap/pm/model';
import type { JSONContent } from '@tiptap/core';
export function diff(oldDoc: JSONContent, newDoc: JSONContent): object[] {
  // Create a headless editor to get the schema
  const editor = new Editor({
    extensions: createExtensions({ disableHistory: true }),
    content: oldDoc,
  });

  const schema = editor.schema;

  // Reconstruct ProseMirror Node instances from JSON
  const _oldNode = ProseMirrorNode.fromJSON(schema, oldDoc);
  const _newNode = ProseMirrorNode.fromJSON(schema, newDoc);

  // TODO: Use prosemirror-recreate-transform to compute Steps.
  //
  // The implementation will look roughly like:
  //
  //   import { recreateTransform } from 'prosemirror-recreate-transform'
  //   const transform = recreateTransform(oldNode, newNode)
  //   const steps = transform.steps.map(step => step.toJSON())
  //
  // For now, return an empty array. The merge endpoint should detect
  // this and fall back to a full document replace if no steps are computed.

  const steps: object[] = [];

  editor.destroy();

  return steps;
}
