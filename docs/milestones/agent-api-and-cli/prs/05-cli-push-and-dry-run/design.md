# PR 5: CLI Push & Dry-Run

## Purpose

Implement the CLI's push command, completing the checkout→edit→push cycle. After this PR, a user or agent can check out a document, edit it locally with any tool, preview changes with `--dry-run`, and push changes back to the server with automatic three-way merge.

## Commands

### `cadmus push <doc-id> <file>`

```bash
$ cadmus push 550e8400 ./design-spec.md
✓ Pushed changes to "Design Spec"
  New version: v_def456
  Changes: 5 steps applied (2 additions, 1 modification)
  Checkout metadata updated.
```

Flow:

1. Read `.cadmus/<doc-id>.json` to get `base_version` and the server URL.
2. Read the local markdown file.
3. Call `POST /api/docs/{id}/content` with `base_version`, `format: "markdown"`, and the file content.
4. On success: display the change summary and update `.cadmus/<doc-id>.json` with the new version.
5. On failure: display a clear error message.

If no `.cadmus/` metadata exists for the document, error with a suggestion to run `cadmus checkout` first.

### `cadmus push --dry-run`

```bash
$ cadmus push 550e8400 ./design-spec.md --dry-run
Preview of changes to "Design Spec":

--- base (v_abc123)
+++ local (./design-spec.md)
@@ -5,7 +5,9 @@
 ## Overview

-This is the old paragraph.
+This is the updated paragraph with new information
+added by the agent.

 ## Implementation

5 steps would be applied (2 additions, 1 modification)
No conflicts detected.

To apply these changes, run without --dry-run.
```

Calls the same endpoint with `?dry_run=true`. Displays the unified diff and change summary without modifying the document. Does not update checkout metadata.

### `cadmus push --force`

The `--force` flag is accepted but reserved for future use (M7 change size limits). Currently it has no effect — all pushes are applied regardless of size.

## Auto-Detection of Document ID

When the user omits the doc-id, the CLI looks for a `.cadmus/` directory in the current directory or parents, and matches the file argument against checkout metadata:

```bash
# If .cadmus/ contains metadata linking design-spec.md to doc 550e8400:
$ cadmus push ./design-spec.md
# Equivalent to: cadmus push 550e8400 ./design-spec.md
```

If multiple `.cadmus/` entries match or none match, error with guidance.

## Post-Push Version Update

On successful push, the CLI:

1. Updates `.cadmus/<doc-id>.json` with the new version from the response.
2. This means the next push uses the latest version as its base, enabling sequential edit→push→edit→push cycles without re-checking out.

## Handling Stale Base Versions

If the document has been edited significantly since checkout, the three-way merge may produce unexpected results. The recommended workflow is:

1. Push with `--dry-run` first to preview the merge.
2. If the diff looks wrong, re-checkout to get the latest version, re-apply local changes, and push again.

The CLI does not implement automatic re-checkout or conflict resolution — the dry-run preview is the safety mechanism.

## Output Formatting

- Diffs use standard unified diff format with color (green for additions, red for deletions).
- Change summaries are one-line with counts.
- Spinners show during network calls (`Pushing changes...`).
- Success uses a green checkmark, errors use a red X.

## Error Cases

| Scenario                           | Output                                                                        |
| ---------------------------------- | ----------------------------------------------------------------------------- |
| No .cadmus/ metadata for doc       | `Error: No checkout metadata found. Run 'cadmus checkout 550e8400' first.`    |
| File not found                     | `Error: File not found: ./design-spec.md`                                     |
| Base version no longer exists      | `Error: Base version v_abc123 not found. Re-checkout with 'cadmus checkout'.` |
| Server unavailable                 | `Error: Cannot reach server at http://localhost:8080`                         |
| Permission denied                  | `Error: You don't have edit access to this document.`                         |
| Agent token lacks docs:write scope | `Error: Token does not have write permission.`                                |

## What's Not Included

- Automatic conflict resolution or re-checkout
- Interactive merge UI
- Change size warnings/limits (M7)
- Batch push (multiple documents at once)
