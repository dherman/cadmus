# PR 2: Comment Anchoring — Implementation Plan

## Prerequisites

- [x] PR 1 (Comments Table & CRUD API) is merged

## Steps

### 1. Implement ProseMirror ↔ Yrs position mapping

- [x] Add position mapping functions to `packages/server/src/documents/anchors.rs`:

```rust
use yrs::types::xml::XmlFragmentRef;
use yrs::{ReadTxn, RelativePosition, TransactionMut};

/// Convert a ProseMirror character offset to a Yrs RelativePosition.
/// Walks the Yrs XML fragment tree, counting positions in the same way
/// ProseMirror counts positions (nodes add 1 for open/close tags, text
/// characters add 1 each).
pub fn pm_offset_to_relative_position(
    txn: &impl ReadTxn,
    xml_fragment: &XmlFragmentRef,
    pm_offset: u32,
) -> Option<RelativePosition> {
    // Implementation: traverse the XML tree depth-first,
    // counting positions ProseMirror-style until reaching pm_offset,
    // then create a RelativePosition at the corresponding Yrs location.
}

/// Convert a Yrs RelativePosition back to a ProseMirror character offset.
pub fn relative_position_to_pm_offset(
    txn: &impl ReadTxn,
    xml_fragment: &XmlFragmentRef,
    rel_pos: &RelativePosition,
) -> Option<u32> {
    // Implementation: resolve the RelativePosition to an absolute Yrs position,
    // then walk the XML tree to convert back to ProseMirror offset.
}
```

- [x] The ProseMirror position model counts: +1 for each character of text, +1 for each node open tag, +1 for each node close tag (except the root doc). The Yrs XML model uses a similar but not identical counting. The mapping function must account for the differences — particularly around XmlElement boundaries and XmlText nodes.

### 2. Update the create_comment handler

- [x] Modify `create_comment` in `packages/server/src/documents/api.rs`:

```rust
async fn create_comment(
    State(state): State<AppState>,
    Path(doc_id): Path<Uuid>,
    user: AuthUser,
    Json(req): Json<CreateCommentRequest>,
) -> Result<Json<CommentResponse>, AppError> {
    // 1. Check Comment permission (already done in PR 1)
    require_permission(&state.pool, doc_id, user.id, Permission::Comment).await?;

    // 2. Validate body non-empty (already done in PR 1)

    // 3. Convert anchors if provided
    let (anchor_start, anchor_end) = if let (Some(from), Some(to)) = (req.anchor_from, req.anchor_to) {
        let session = state.session_manager.get_or_load(doc_id).await?;
        let doc = session.doc();
        let txn = doc.transact();
        let xml = txn.get_xml_fragment("prosemirror").unwrap();

        let start = pm_offset_to_relative_position(&txn, &xml, from as u32)
            .ok_or(AppError::BadRequest("Invalid anchor_from position".into()))?;
        let end = pm_offset_to_relative_position(&txn, &xml, to as u32)
            .ok_or(AppError::BadRequest("Invalid anchor_to position".into()))?;

        (Some(start.encode_v1()), Some(end.encode_v1()))
    } else {
        (None, None)
    };

    // 4. Insert comment with binary anchors
    let comment = db::create_comment(
        &state.pool, doc_id, user.id, &req.body, anchor_start, anchor_end,
    ).await?;

    // 5. Return with original offsets
    Ok(Json(CommentResponse {
        anchor_from: req.anchor_from.map(|v| v as u32),
        anchor_to: req.anchor_to.map(|v| v as u32),
        // ... other fields
    }))
}
```

### 3. Update the list_comments handler

- [x] Modify `list_comments` in `packages/server/src/documents/api.rs` to resolve anchors:

```rust
async fn list_comments(
    State(state): State<AppState>,
    Path(doc_id): Path<Uuid>,
    Query(params): Query<ListCommentsParams>,
    user: AuthUser,
) -> Result<Json<Vec<CommentResponse>>, AppError> {
    require_permission(&state.pool, doc_id, user.id, Permission::Read).await?;

    let comments = db::list_comments(&state.pool, doc_id, params.status.as_deref()).await?;

    // Load the Yrs doc for anchor resolution
    let session = state.session_manager.get_or_load(doc_id).await?;
    let doc = session.doc();
    let txn = doc.transact();
    let xml = txn.get_xml_fragment("prosemirror").unwrap();

    let responses: Vec<CommentResponse> = comments
        .into_iter()
        .map(|c| {
            let anchor_from = c.anchor_start.as_ref().and_then(|bytes| {
                let rp = RelativePosition::decode_v1(bytes).ok()?;
                relative_position_to_pm_offset(&txn, &xml, &rp)
            });
            let anchor_to = c.anchor_end.as_ref().and_then(|bytes| {
                let rp = RelativePosition::decode_v1(bytes).ok()?;
                relative_position_to_pm_offset(&txn, &xml, &rp)
            });

            CommentResponse {
                id: c.id,
                document_id: c.document_id,
                author: CommentAuthor {
                    id: c.author_id,
                    display_name: c.author_display_name,
                    email: c.author_email,
                },
                parent_id: c.parent_id,
                anchor_from,
                anchor_to,
                body: c.body,
                status: c.status,
                created_at: c.created_at,
                updated_at: c.updated_at,
            }
        })
        .collect();

    Ok(Json(responses))
}
```

### 4. Add anchor fields to the frontend Comment type

- [x] Update the `Comment` interface in `packages/web/src/api.ts`:

```typescript
export interface Comment {
  id: string;
  document_id: string;
  author: { id: string; display_name: string; email: string };
  parent_id: string | null;
  anchor_from: number | null;
  anchor_to: number | null;
  body: string;
  status: string;
  created_at: string;
  updated_at: string;
}
```

- [x] Update `createComment` to pass `anchor_from` and `anchor_to` in the request body (already done in PR 1, verified).

### 5. Write tests for position mapping

- [x] Add unit tests for `pm_offset_to_sticky_bytes` and `sticky_bytes_to_pm_offset`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_paragraph_offset() {
        // Create a Yrs doc with a simple paragraph
        // Verify that ProseMirror offset maps correctly to RelativePosition
        // and back
    }

    #[test]
    fn test_offset_survives_insert_before() {
        // Create a doc, get a RelativePosition for a position
        // Insert text before that position
        // Resolve the RelativePosition — should point to the same content
    }

    #[test]
    fn test_offset_after_deletion() {
        // Create a doc, anchor to some text
        // Delete the anchored text
        // Resolve — should return nearest valid position
    }

    #[test]
    fn test_nested_content_offset() {
        // Create a doc with nested lists/blockquotes
        // Verify offset mapping accounts for node boundaries
    }
}
```

### 6. Integration test: anchor survives concurrent edit

- [ ] Manually test: open a document in two tabs, create a comment anchored to a word in the second paragraph. In the first tab, insert a new paragraph above. Refresh the second tab and verify the comment's anchor still points to the correct word.

### 7. Build and verify

- [x] Run `cargo build` — compiles without errors.
- [x] Run `cargo test` — all tests pass, including new anchor mapping tests.
- [x] Run `pnpm -F @cadmus/web build` — TypeScript compiles.
- [x] Run `pnpm run format:check` — no formatting issues.

## Verification

- [x] Creating a comment with `anchor_from`/`anchor_to` stores binary StickyIndex blobs in the database
- [x] Listing comments returns resolved absolute offsets
- [x] Resolved offsets match the original offsets when no edits have occurred
- [x] Resolved offsets track correctly after text is inserted before the anchor
- [x] Resolved offsets track correctly after text is inserted after the anchor
- [x] Deleting anchored text doesn't crash — returns nearest valid position or null
- [x] Comments without anchors (replies) return null for anchor fields
- [x] Position mapping handles nested content (lists, blockquotes) correctly

## Files Modified

| File                                        | Change                                             |
| ------------------------------------------- | -------------------------------------------------- |
| `packages/server/src/documents/anchors.rs`  | New: ProseMirror ↔ Yrs position mapping functions  |
| `packages/server/src/documents/mod.rs`      | Register `anchors` module                          |
| `packages/server/src/documents/comments.rs` | Add `anchor_from`/`anchor_to` to `CommentResponse` |
| `packages/server/src/documents/api.rs`      | Update create/list handlers with anchor logic      |
| `packages/web/src/api.ts`                   | Add `anchor_from`/`anchor_to` to `Comment` type    |
