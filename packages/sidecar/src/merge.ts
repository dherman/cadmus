/**
 * Three-way merge for ProseMirror documents.
 *
 * Given a base document, a current document (with concurrent edits), and a
 * new document (from the CLI push), produce the Steps needed to transform
 * the current document into the merged result.
 *
 * Strategy:
 * 1. Compute "their" steps: base → current (what the browser changed)
 * 2. Compute "our" steps: base → new (what the CLI changed)
 * 3. Rebase "our" steps over "their" steps using ProseMirror's Mapping
 * 4. Apply the rebased "our" steps to the current document
 * 5. Return the steps that transform current → merged
 *
 * This ensures that positions in the returned steps are relative to the
 * current CRDT state, not the base version.
 */

import { Editor } from '@tiptap/core';
import { createExtensions } from '@cadmus/doc-schema';
import { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { recreateTransform } from '@fellow/prosemirror-recreate-transform';
import type { JSONContent } from '@tiptap/core';

export function merge(
  baseDoc: JSONContent,
  currentDoc: JSONContent,
  newDoc: JSONContent,
): object[] {
  const editor = new Editor({
    extensions: createExtensions({ disableHistory: true }),
    content: baseDoc,
  });

  const schema = editor.schema;
  const baseNode = ProseMirrorNode.fromJSON(schema, baseDoc);
  const currentNode = ProseMirrorNode.fromJSON(schema, currentDoc);
  const newNode = ProseMirrorNode.fromJSON(schema, newDoc);

  // If the current doc equals the base doc (no concurrent edits),
  // just diff base → new directly
  if (currentNode.eq(baseNode)) {
    const transform = recreateTransform(currentNode, newNode);
    const steps = transform.steps.map((step) => step.toJSON());
    editor.destroy();
    return steps;
  }

  // Compute "their" changes: base → current
  const theirTransform = recreateTransform(baseNode, currentNode);

  // Compute "our" changes: base → new
  const ourTransform = recreateTransform(baseNode, newNode);

  // Rebase "our" steps over "their" steps.
  // The mapping from "their" transform tells us how positions shifted
  // due to concurrent edits. We map each of "our" steps through this
  // mapping to get positions relative to the current document.
  const theirMapping = theirTransform.mapping;

  // Apply rebased "our" steps to the current document
  let doc = currentNode;
  const appliedSteps: object[] = [];

  for (const step of ourTransform.steps) {
    // Map the step's positions through the concurrent edit mapping
    const mapped = step.map(theirMapping);
    if (!mapped) continue;

    const result = mapped.apply(doc);
    if (result.doc) {
      appliedSteps.push(mapped.toJSON());
      doc = result.doc;
    }
    // If the step fails to apply (conflict), skip it silently.
    // The dry-run preview will show the user what the merge looks like.
  }

  editor.destroy();
  return appliedSteps;
}
