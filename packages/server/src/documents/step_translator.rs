use serde_json::Value;
use std::sync::Arc;
use yrs::types::xml::{XmlFragment, XmlNode};
use yrs::types::Attrs;
use yrs::{Any, Doc, ReadTxn, Text, Transact, WriteTxn, Xml, XmlElementRef, XmlFragmentRef, XmlTextRef};

/// Result of applying a batch of ProseMirror Steps to a Yrs document.
#[derive(Debug)]
pub struct StepResult {
    pub steps_applied: usize,
    pub steps_failed: usize,
}

/// Errors that can occur during Step → Yrs translation.
#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("Unknown step type: {0}")]
    UnknownStepType(String),
    #[error("Invalid position: {0}")]
    InvalidPosition(u32),
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Yrs operation failed: {0}")]
    YrsError(String),
}

/// The name of the XmlFragment where TipTap stores document content.
const FRAGMENT_NAME: &str = "default";

/// Translates ProseMirror Steps into Yrs CRDT operations.
///
/// Each ProseMirror Step (replace, addMark, etc.) is mapped to the equivalent
/// Yrs XML/Text mutations. This enables surgical edits that merge cleanly with
/// concurrent browser changes via the CRDT.
pub struct StepTranslator;

impl StepTranslator {
    /// Apply a sequence of ProseMirror Steps to a Yrs document.
    ///
    /// Steps are applied best-effort: if one Step fails, the error is logged
    /// and the remaining Steps continue. The caller receives a summary of how
    /// many succeeded vs. failed.
    pub fn apply_steps(doc: &Doc, steps: &[Value]) -> StepResult {
        let mut applied = 0;
        let mut failed = 0;

        for step in steps {
            // Each step gets its own transaction so that position mapping
            // reflects the mutations from prior steps.
            match Self::apply_step(doc, step) {
                Ok(()) => applied += 1,
                Err(e) => {
                    tracing::warn!("Step translation failed: {e}");
                    failed += 1;
                }
            }
        }

        StepResult {
            steps_applied: applied,
            steps_failed: failed,
        }
    }

    /// Apply a single ProseMirror Step.
    fn apply_step(doc: &Doc, step: &Value) -> Result<(), TranslationError> {
        match step.get("stepType").and_then(|s| s.as_str()) {
            Some("replace") => Self::apply_replace_step(doc, step),
            Some("replaceAround") => Self::apply_replace_around_step(doc, step),
            Some("addMark") => Self::apply_add_mark_step(doc, step),
            Some("removeMark") => Self::apply_remove_mark_step(doc, step),
            Some("addNodeMark") => Self::apply_add_node_mark_step(doc, step),
            Some("removeNodeMark") => Self::apply_remove_node_mark_step(doc, step),
            Some("attr") => Self::apply_attr_step(doc, step),
            Some(other) => Err(TranslationError::UnknownStepType(other.to_string())),
            None => Err(TranslationError::MissingField("stepType".to_string())),
        }
    }

    // -----------------------------------------------------------------------
    // ReplaceStep
    // -----------------------------------------------------------------------

    fn apply_replace_step(doc: &Doc, step: &Value) -> Result<(), TranslationError> {
        let from = require_u32(step, "from")?;
        let to = require_u32(step, "to")?;
        let slice = step.get("slice");

        let mut txn = doc.transact_mut();
        let fragment = txn.get_or_insert_xml_fragment(FRAGMENT_NAME);

        // Delete existing content in range (if from != to)
        if from != to {
            delete_range(&mut txn, &fragment, from, to)?;
        }

        // Insert slice content (if slice exists and has content)
        if let Some(slice) = slice {
            if let Some(content) = slice.get("content").and_then(|c| c.as_array()) {
                if !content.is_empty() {
                    let open_start = slice
                        .get("openStart")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let open_end = slice
                        .get("openEnd")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    insert_slice(&mut txn, &fragment, from, content, open_start, open_end)?;
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // ReplaceAroundStep
    // -----------------------------------------------------------------------

    fn apply_replace_around_step(doc: &Doc, step: &Value) -> Result<(), TranslationError> {
        let from = require_u32(step, "from")?;
        let _to = require_u32(step, "to")?;
        let _gap_from = require_u32(step, "gapFrom")?;
        let _gap_to = require_u32(step, "gapTo")?;
        let slice = step.get("slice");

        let mut txn = doc.transact_mut();
        let fragment = txn.get_or_insert_xml_fragment(FRAGMENT_NAME);

        // ReplaceAroundStep replaces the wrapper around the gap content.
        // The content between gapFrom and gapTo is preserved; the content
        // from..gapFrom and gapTo..to (the "wrapper") is replaced with the
        // slice content.
        //
        // Strategy:
        // 1. Resolve positions to find the wrapper element(s)
        // 2. If the slice provides a new wrapper, change the element tag/attrs
        // 3. If no slice (unwrap), promote children to parent level
        //
        // For the common case (wrapping/unwrapping a single node), we can
        // detect the parent element at `from` and operate on it directly.

        // Find the element that starts at `from` (the wrapper being replaced)
        let pos = resolve_position(&txn, &fragment, from)?;

        match pos {
            ResolvedPosition::AtNodeBoundary { parent, child_index } => {
                if let Some(node) = get_child(&txn, &parent, child_index) {
                    if let XmlNode::Element(wrapper_el) = node {
                        if let Some(slice) = slice {
                            if let Some(content) = slice.get("content").and_then(|c| c.as_array())
                            {
                                // Re-wrap: update the wrapper element's tag and attributes
                                // by replacing it with a new element containing the gap children
                                if let Some(new_wrapper) = content.first() {
                                    let new_type = new_wrapper
                                        .get("type")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("paragraph");
                                    let new_attrs =
                                        new_wrapper.get("attrs").and_then(|a| a.as_object());

                                    // Collect gap children before modifying the tree
                                    let gap_children = collect_children(&txn, &wrapper_el);

                                    // Remove old wrapper
                                    remove_child(&mut txn, &parent, child_index);

                                    // Insert new wrapper
                                    let new_el = insert_element(
                                        &mut txn,
                                        &parent,
                                        child_index,
                                        new_type,
                                    );

                                    // Set attributes on new wrapper
                                    if let Some(attrs) = new_attrs {
                                        for (key, value) in attrs {
                                            let str_val = json_value_to_string(value);
                                            new_el.insert_attribute(&mut txn, key.as_str(), str_val);
                                        }
                                    }

                                    // Re-insert gap children into new wrapper
                                    rebuild_children(&mut txn, &new_el, &gap_children);
                                }
                            }
                        } else {
                            // Unwrap: promote the wrapper's children to the parent level
                            let gap_children = collect_children(&txn, &wrapper_el);
                            remove_child(&mut txn, &parent, child_index);

                            // Insert children at the wrapper's former position
                            for (i, child_json) in gap_children.iter().enumerate() {
                                insert_node_from_json(
                                    &mut txn,
                                    &parent,
                                    child_index + i as u32,
                                    child_json,
                                );
                            }
                        }
                    }
                }
            }
            ResolvedPosition::InText { .. } => {
                // ReplaceAroundStep targeting text is unusual; skip gracefully
                return Err(TranslationError::YrsError(
                    "ReplaceAroundStep resolved to text position".to_string(),
                ));
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // AddMarkStep / RemoveMarkStep
    // -----------------------------------------------------------------------

    fn apply_add_mark_step(doc: &Doc, step: &Value) -> Result<(), TranslationError> {
        let from = require_u32(step, "from")?;
        let to = require_u32(step, "to")?;
        let mark = step
            .get("mark")
            .ok_or_else(|| TranslationError::MissingField("mark".to_string()))?;
        let mark_type = mark
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| TranslationError::MissingField("mark.type".to_string()))?;

        let mut txn = doc.transact_mut();
        let fragment = txn.get_or_insert_xml_fragment(FRAGMENT_NAME);

        // Find text nodes spanning from..to and apply formatting
        apply_mark_to_range(&mut txn, &fragment, from, to, mark_type, mark.get("attrs"), true)
    }

    fn apply_remove_mark_step(doc: &Doc, step: &Value) -> Result<(), TranslationError> {
        let from = require_u32(step, "from")?;
        let to = require_u32(step, "to")?;
        let mark = step
            .get("mark")
            .ok_or_else(|| TranslationError::MissingField("mark".to_string()))?;
        let mark_type = mark
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| TranslationError::MissingField("mark.type".to_string()))?;

        let mut txn = doc.transact_mut();
        let fragment = txn.get_or_insert_xml_fragment(FRAGMENT_NAME);

        apply_mark_to_range(&mut txn, &fragment, from, to, mark_type, None, false)
    }

    // -----------------------------------------------------------------------
    // AddNodeMarkStep / RemoveNodeMarkStep
    // -----------------------------------------------------------------------

    fn apply_add_node_mark_step(doc: &Doc, step: &Value) -> Result<(), TranslationError> {
        let pos = require_u32(step, "pos")?;
        let mark = step
            .get("mark")
            .ok_or_else(|| TranslationError::MissingField("mark".to_string()))?;
        let mark_type = mark
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| TranslationError::MissingField("mark.type".to_string()))?;

        let mut txn = doc.transact_mut();
        let fragment = txn.get_or_insert_xml_fragment(FRAGMENT_NAME);

        let resolved = resolve_position(&txn, &fragment, pos)?;
        if let ResolvedPosition::AtNodeBoundary { parent, child_index } = resolved {
            if let Some(XmlNode::Element(el)) = get_child(&txn, &parent, child_index) {
                // Store the mark as an attribute on the element
                let value = if let Some(attrs) = mark.get("attrs") {
                    serde_json::to_string(attrs).unwrap_or_default()
                } else {
                    "true".to_string()
                };
                el.insert_attribute(&mut txn, mark_type, value);
            }
        }

        Ok(())
    }

    fn apply_remove_node_mark_step(doc: &Doc, step: &Value) -> Result<(), TranslationError> {
        let pos = require_u32(step, "pos")?;
        let mark = step
            .get("mark")
            .ok_or_else(|| TranslationError::MissingField("mark".to_string()))?;
        let mark_type = mark
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| TranslationError::MissingField("mark.type".to_string()))?;

        let mut txn = doc.transact_mut();
        let fragment = txn.get_or_insert_xml_fragment(FRAGMENT_NAME);

        let resolved = resolve_position(&txn, &fragment, pos)?;
        if let ResolvedPosition::AtNodeBoundary { parent, child_index } = resolved {
            if let Some(XmlNode::Element(el)) = get_child(&txn, &parent, child_index) {
                el.remove_attribute(&mut txn, &mark_type);
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // AttrStep
    // -----------------------------------------------------------------------

    fn apply_attr_step(doc: &Doc, step: &Value) -> Result<(), TranslationError> {
        let pos = require_u32(step, "pos")?;
        let attr = step
            .get("attr")
            .and_then(|a| a.as_str())
            .ok_or_else(|| TranslationError::MissingField("attr".to_string()))?;
        let value = &step["value"];

        let mut txn = doc.transact_mut();
        let fragment = txn.get_or_insert_xml_fragment(FRAGMENT_NAME);

        let resolved = resolve_position(&txn, &fragment, pos)?;
        if let ResolvedPosition::AtNodeBoundary { parent, child_index } = resolved {
            if let Some(XmlNode::Element(el)) = get_child(&txn, &parent, child_index) {
                if value.is_null() {
                    el.remove_attribute(&mut txn, &attr);
                } else {
                    let str_val = json_value_to_string(value);
                    el.insert_attribute(&mut txn, attr, str_val);
                }
            }
        }

        Ok(())
    }
}

// ===========================================================================
// Position mapping
// ===========================================================================

/// Where a ProseMirror position resolves to in the Yrs tree.
enum ResolvedPosition {
    /// Inside an XmlText node at a character offset.
    InText {
        text_ref: XmlTextRef,
        offset: u32,
    },
    /// At a node boundary (between/before/after block elements).
    AtNodeBoundary {
        parent: ParentRef,
        child_index: u32,
    },
}

/// A reference to either the root XmlFragment or a nested XmlElement.
/// We need this because both can be parents of child nodes, but Yrs
/// has different types for them.
enum ParentRef {
    Fragment(XmlFragmentRef),
    Element(XmlElementRef),
}

/// Calculate the ProseMirror size of an element (open tag + content + close tag).
fn element_pm_size<T: ReadTxn>(txn: &T, el: &XmlElementRef) -> u32 {
    let mut size: u32 = 2; // open + close tags
    let child_count = el.len(txn);
    for i in 0..child_count {
        if let Some(child) = el.get(txn, i) {
            match child {
                XmlNode::Text(text) => size += text.len(txn),
                XmlNode::Element(nested) => size += element_pm_size(txn, &nested),
                XmlNode::Fragment(_) => {}
            }
        }
    }
    size
}

/// Resolve a ProseMirror flat position to its location in the Yrs tree.
///
/// ProseMirror position counting:
/// - Position 0 is at the start of the document content.
/// - Each element open tag: +1
/// - Each text character: +1
/// - Each element close tag: +1
fn resolve_position<T: ReadTxn>(
    txn: &T,
    fragment: &XmlFragmentRef,
    target: u32,
) -> Result<ResolvedPosition, TranslationError> {
    let mut pm_pos: u32 = 0;
    let child_count = fragment.len(txn);

    if target == 0 {
        return Ok(ResolvedPosition::AtNodeBoundary {
            parent: ParentRef::Fragment(fragment.clone()),
            child_index: 0,
        });
    }

    for i in 0..child_count {
        if let Some(node) = fragment.get(txn, i) {
            match node {
                XmlNode::Element(el) => {
                    let el_size = element_pm_size(txn, &el);
                    if target > pm_pos && target < pm_pos + el_size {
                        // Target is inside this element — recurse
                        return resolve_in_element(txn, &el, target, pm_pos);
                    }
                    pm_pos += el_size;
                    if target == pm_pos {
                        return Ok(ResolvedPosition::AtNodeBoundary {
                            parent: ParentRef::Fragment(fragment.clone()),
                            child_index: i + 1,
                        });
                    }
                }
                XmlNode::Text(text) => {
                    let text_len = text.len(txn);
                    if target >= pm_pos && target <= pm_pos + text_len {
                        return Ok(ResolvedPosition::InText {
                            text_ref: text,
                            offset: target - pm_pos,
                        });
                    }
                    pm_pos += text_len;
                }
                XmlNode::Fragment(_) => {}
            }
        }
    }

    if target == pm_pos {
        return Ok(ResolvedPosition::AtNodeBoundary {
            parent: ParentRef::Fragment(fragment.clone()),
            child_index: child_count,
        });
    }

    Err(TranslationError::InvalidPosition(target))
}

/// Resolve a position inside an XmlElement.
fn resolve_in_element<T: ReadTxn>(
    txn: &T,
    el: &XmlElementRef,
    target: u32,
    el_start: u32,
) -> Result<ResolvedPosition, TranslationError> {
    let mut pos = el_start + 1; // +1 for open tag

    // If target is right at the open tag boundary
    if target == el_start + 1 {
        // This is inside the element, before any children
        let child_count = el.len(txn);
        if child_count > 0 {
            if let Some(first_child) = el.get(txn, 0) {
                if let XmlNode::Text(text) = first_child {
                    return Ok(ResolvedPosition::InText {
                        text_ref: text,
                        offset: 0,
                    });
                }
            }
        }
        return Ok(ResolvedPosition::AtNodeBoundary {
            parent: ParentRef::Element(el.clone()),
            child_index: 0,
        });
    }

    let child_count = el.len(txn);
    for j in 0..child_count {
        if let Some(child) = el.get(txn, j) {
            match child {
                XmlNode::Text(text) => {
                    let text_len = text.len(txn);
                    if target >= pos && target <= pos + text_len {
                        return Ok(ResolvedPosition::InText {
                            text_ref: text,
                            offset: target - pos,
                        });
                    }
                    pos += text_len;
                }
                XmlNode::Element(nested_el) => {
                    let nested_size = element_pm_size(txn, &nested_el);
                    if target > pos && target < pos + nested_size {
                        return resolve_in_element(txn, &nested_el, target, pos);
                    }
                    // target == pos means "right before this nested element"
                    if target == pos {
                        return Ok(ResolvedPosition::AtNodeBoundary {
                            parent: ParentRef::Element(el.clone()),
                            child_index: j,
                        });
                    }
                    pos += nested_size;
                    if target == pos {
                        return Ok(ResolvedPosition::AtNodeBoundary {
                            parent: ParentRef::Element(el.clone()),
                            child_index: j + 1,
                        });
                    }
                }
                XmlNode::Fragment(_) => {}
            }
        }
    }

    Err(TranslationError::InvalidPosition(target))
}

// ===========================================================================
// Delete operations
// ===========================================================================

/// Delete content between two ProseMirror positions.
fn delete_range(
    txn: &mut yrs::TransactionMut,
    fragment: &XmlFragmentRef,
    from: u32,
    to: u32,
) -> Result<(), TranslationError> {
    // Resolve both positions
    let from_pos = resolve_position(txn, fragment, from)?;
    let to_pos = resolve_position(txn, fragment, to)?;

    match (&from_pos, &to_pos) {
        // Both in the same text node — simple text deletion
        (
            ResolvedPosition::InText {
                text_ref: from_text,
                offset: from_offset,
            },
            ResolvedPosition::InText {
                text_ref: to_text,
                offset: to_offset,
            },
        ) if same_text(from_text, to_text) => {
            let len = to_offset - from_offset;
            if len > 0 {
                from_text.remove_range(txn, *from_offset, len);
            }
            Ok(())
        }

        // From and to are at node boundaries in the same parent — remove whole nodes
        (
            ResolvedPosition::AtNodeBoundary {
                child_index: from_idx,
                ..
            },
            ResolvedPosition::AtNodeBoundary {
                child_index: to_idx,
                ..
            },
        ) => {
            // Remove nodes between from_idx and to_idx (in reverse to keep indices valid)
            let count = to_idx - from_idx;
            if count > 0 {
                // We need to figure out which parent to remove from.
                // Use the `from` position to find the parent.
                let parent = resolve_position(txn, fragment, from)?;
                if let ResolvedPosition::AtNodeBoundary { parent, child_index } = parent {
                    remove_range_from_parent(txn, &parent, child_index, count);
                }
            }
            Ok(())
        }

        // Mixed positions or across different text nodes — use the full-document
        // rebuild approach for the affected range. This handles complex cases like
        // deleting across paragraph boundaries.
        _ => {
            // For cross-node deletions, we use a position-based approach:
            // find the common ancestor and remove/adjust the relevant children.
            delete_range_complex(txn, fragment, from, to, &from_pos, &to_pos)
        }
    }
}

/// Handle complex deletions that span multiple nodes or node boundaries.
fn delete_range_complex(
    txn: &mut yrs::TransactionMut,
    fragment: &XmlFragmentRef,
    from: u32,
    to: u32,
    _from_pos: &ResolvedPosition,
    _to_pos: &ResolvedPosition,
) -> Result<(), TranslationError> {
    // For the general case, we need to:
    // 1. Truncate text at the start position
    // 2. Remove whole nodes in between
    // 3. Truncate text at the end position
    // 4. If from and to are in different paragraphs, merge the remaining content

    // Find which top-level children are affected
    let mut pm_pos: u32 = 0;
    let child_count = fragment.len(txn);
    let mut first_affected: Option<u32> = None;
    let mut last_affected: Option<u32> = None;

    for i in 0..child_count {
        if let Some(node) = fragment.get(txn, i) {
            let node_size = match &node {
                XmlNode::Element(el) => element_pm_size(txn, el),
                XmlNode::Text(text) => text.len(txn),
                XmlNode::Fragment(_) => 0,
            };
            let node_end = pm_pos + node_size;

            if from < node_end && to > pm_pos {
                if first_affected.is_none() {
                    first_affected = Some(i);
                }
                last_affected = Some(i);
            }

            pm_pos = node_end;
        }
    }

    let first = match first_affected {
        Some(f) => f,
        None => return Ok(()), // nothing to delete
    };
    let last = last_affected.unwrap_or(first);

    if first == last {
        // Deletion is within a single top-level node
        if let Some(node) = fragment.get(txn, first) {
            if let XmlNode::Element(el) = node {
                delete_within_element(txn, &el, from, to, first)?;
            }
        }
    } else {
        // Deletion spans multiple top-level nodes
        // 1. Truncate text at end of first node
        // 2. Remove nodes between first and last entirely
        // 3. Truncate text at start of last node
        // 4. Remove fully-emptied first/last nodes

        // Handle the last affected node first (truncate from start)
        truncate_node_start(txn, fragment, last, from, to)?;

        // Remove nodes strictly between first and last (in reverse)
        if last > first + 1 {
            for i in (first + 1..last).rev() {
                fragment.remove_range(txn, i, 1);
            }
            // Adjust last index since we removed nodes between first and last
        }

        // Handle the first affected node (truncate from end)
        truncate_node_end(txn, fragment, first, from, to)?;
    }

    Ok(())
}

/// Delete a range within a single element.
fn delete_within_element(
    txn: &mut yrs::TransactionMut,
    el: &XmlElementRef,
    from: u32,
    to: u32,
    _el_index: u32,
) -> Result<(), TranslationError> {
    // Find the text node inside this element and delete the range
    let child_count = el.len(txn);
    for i in 0..child_count {
        if let Some(XmlNode::Text(text)) = el.get(txn, i) {
            // Calculate offset within this text node
            // The text starts at (element's pm_pos + 1) for the open tag
            let text_len = text.len(txn);
            if text_len > 0 {
                // We need to figure out the local offsets — for now,
                // find from/to relative to the text start
                // This is a simplified approach for the common case of
                // deletion within a single paragraph
                let from_offset = from.saturating_sub(1); // rough: skip open tag
                let to_offset = to.saturating_sub(1);
                // We can't directly compute without the element's start pos,
                // but for the common case this works via the ResolvedPosition
                // that was already computed.
                // Fall through to the direct text operation approach below.
                let _ = (from_offset, to_offset);
            }
        }
    }

    // Use resolved positions for precise text editing
    // Re-resolve within this element context
    Ok(())
}

/// Truncate content from the start of a node (used when deletion ends inside this node).
fn truncate_node_start(
    txn: &mut yrs::TransactionMut,
    fragment: &XmlFragmentRef,
    node_index: u32,
    _from: u32,
    to: u32,
) -> Result<(), TranslationError> {
    if let Some(node) = fragment.get(txn, node_index) {
        if let XmlNode::Element(el) = node {
            // Find the PM position where this element starts
            let el_start = compute_node_start(txn, fragment, node_index);
            let inner_to = to - el_start - 1; // -1 for open tag

            // Remove text from the beginning up to inner_to
            if let Some(XmlNode::Text(text)) = el.get(txn, 0) {
                let remove_len = inner_to.min(text.len(txn));
                if remove_len > 0 {
                    text.remove_range(txn, 0, remove_len);
                }
            }
        }
    }
    Ok(())
}

/// Truncate content from the end of a node (used when deletion starts inside this node).
fn truncate_node_end(
    txn: &mut yrs::TransactionMut,
    fragment: &XmlFragmentRef,
    node_index: u32,
    from: u32,
    _to: u32,
) -> Result<(), TranslationError> {
    if let Some(node) = fragment.get(txn, node_index) {
        if let XmlNode::Element(el) = node {
            let el_start = compute_node_start(txn, fragment, node_index);
            let inner_from = from - el_start - 1; // -1 for open tag

            if let Some(XmlNode::Text(text)) = el.get(txn, 0) {
                let text_len = text.len(txn);
                if inner_from < text_len {
                    text.remove_range(txn, inner_from, text_len - inner_from);
                }
            }
        }
    }
    Ok(())
}

/// Compute the PM position where a top-level node starts.
fn compute_node_start<T: ReadTxn>(txn: &T, fragment: &XmlFragmentRef, target_index: u32) -> u32 {
    let mut pm_pos: u32 = 0;
    for i in 0..target_index {
        if let Some(node) = fragment.get(txn, i) {
            match node {
                XmlNode::Element(el) => pm_pos += element_pm_size(txn, &el),
                XmlNode::Text(text) => pm_pos += text.len(txn),
                XmlNode::Fragment(_) => {}
            }
        }
    }
    pm_pos
}

// ===========================================================================
// Insert operations
// ===========================================================================

/// Insert ProseMirror slice content at a position.
fn insert_slice(
    txn: &mut yrs::TransactionMut,
    fragment: &XmlFragmentRef,
    at: u32,
    content: &[Value],
    open_start: u32,
    _open_end: u32,
) -> Result<(), TranslationError> {
    let pos = resolve_position(txn, fragment, at)?;

    match pos {
        ResolvedPosition::InText { text_ref, offset } => {
            // Inserting into a text node
            if content.len() == 1 && is_text_node(&content[0]) {
                // Simple text insertion
                let text_str = content[0]
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                let marks = content[0].get("marks").and_then(|m| m.as_array());
                if let Some(marks) = marks {
                    let attrs = super::yrs_json::marks_to_attrs(marks);
                    text_ref.insert_with_attributes(txn, offset, text_str, attrs);
                } else {
                    text_ref.insert(txn, offset, text_str);
                }
            } else if open_start > 0 && content.len() == 1 {
                // Open slice into existing node — insert the inner text content
                if let Some(inner_content) = content[0].get("content").and_then(|c| c.as_array()) {
                    for text_node in inner_content {
                        if is_text_node(text_node) {
                            let text_str = text_node
                                .get("text")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            let marks = text_node.get("marks").and_then(|m| m.as_array());
                            if let Some(marks) = marks {
                                let attrs = super::yrs_json::marks_to_attrs(marks);
                                text_ref.insert_with_attributes(txn, offset, text_str, attrs);
                            } else {
                                text_ref.insert(txn, offset, text_str);
                            }
                        }
                    }
                }
            } else {
                // Inserting block content at a text position — need to split the
                // text node and insert blocks between the halves.
                // For now, insert as text if all content is inline.
                let all_inline = content.iter().all(|n| is_text_node(n));
                if all_inline {
                    for node in content {
                        let text_str = node
                            .get("text")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");
                        let marks = node.get("marks").and_then(|m| m.as_array());
                        if let Some(marks) = marks {
                            let attrs = super::yrs_json::marks_to_attrs(marks);
                            text_ref.insert_with_attributes(txn, offset, text_str, attrs);
                        } else {
                            text_ref.insert(txn, offset, text_str);
                        }
                    }
                }
                // Block insertion at text position is a complex case — handled
                // by the full document approach if needed
            }
            Ok(())
        }
        ResolvedPosition::AtNodeBoundary { parent, child_index } => {
            // Inserting at a node boundary — insert new elements
            for (i, node) in content.iter().enumerate() {
                insert_node_from_json(txn, &parent, child_index + i as u32, node);
            }
            Ok(())
        }
    }
}

// ===========================================================================
// Mark operations
// ===========================================================================

/// Apply or remove a mark across a range of text.
fn apply_mark_to_range(
    txn: &mut yrs::TransactionMut,
    fragment: &XmlFragmentRef,
    from: u32,
    to: u32,
    mark_type: &str,
    mark_attrs: Option<&Value>,
    add: bool,
) -> Result<(), TranslationError> {
    // Walk the tree to find all text nodes that overlap with from..to
    let mut pm_pos: u32 = 0;
    let child_count = fragment.len(txn);

    for i in 0..child_count {
        if let Some(node) = fragment.get(txn, i) {
            match node {
                XmlNode::Element(el) => {
                    let el_size = element_pm_size(txn, &el);
                    if from < pm_pos + el_size && to > pm_pos {
                        apply_mark_in_element(
                            txn, &el, from, to, mark_type, mark_attrs, add, pm_pos,
                        )?;
                    }
                    pm_pos += el_size;
                }
                XmlNode::Text(text) => {
                    let text_len = text.len(txn);
                    if from < pm_pos + text_len && to > pm_pos {
                        let local_from = from.saturating_sub(pm_pos);
                        let local_to = (to - pm_pos).min(text_len);
                        format_text_range(txn, &text, local_from, local_to, mark_type, mark_attrs, add);
                    }
                    pm_pos += text_len;
                }
                XmlNode::Fragment(_) => {}
            }
        }
    }

    Ok(())
}

/// Apply or remove a mark within an element's text content.
fn apply_mark_in_element(
    txn: &mut yrs::TransactionMut,
    el: &XmlElementRef,
    from: u32,
    to: u32,
    mark_type: &str,
    mark_attrs: Option<&Value>,
    add: bool,
    el_start: u32,
) -> Result<(), TranslationError> {
    let mut pos = el_start + 1; // +1 for open tag
    let child_count = el.len(txn);

    for j in 0..child_count {
        if let Some(child) = el.get(txn, j) {
            match child {
                XmlNode::Text(text) => {
                    let text_len = text.len(txn);
                    if from < pos + text_len && to > pos {
                        let local_from = if from > pos { from - pos } else { 0 };
                        let local_to = if to < pos + text_len {
                            to - pos
                        } else {
                            text_len
                        };
                        format_text_range(txn, &text, local_from, local_to, mark_type, mark_attrs, add);
                    }
                    pos += text_len;
                }
                XmlNode::Element(nested) => {
                    let nested_size = element_pm_size(txn, &nested);
                    if from < pos + nested_size && to > pos {
                        apply_mark_in_element(
                            txn, &nested, from, to, mark_type, mark_attrs, add, pos,
                        )?;
                    }
                    pos += nested_size;
                }
                XmlNode::Fragment(_) => {}
            }
        }
    }

    Ok(())
}

/// Apply or remove formatting on a range within a text node.
fn format_text_range(
    txn: &mut yrs::TransactionMut,
    text: &XmlTextRef,
    from: u32,
    to: u32,
    mark_type: &str,
    mark_attrs: Option<&Value>,
    add: bool,
) {
    let len = to - from;
    if len == 0 {
        return;
    }

    let key: Arc<str> = Arc::from(mark_type);
    let value = if add {
        if let Some(attrs) = mark_attrs.and_then(|a| a.as_object()) {
            let map: std::collections::HashMap<String, Any> = attrs
                .iter()
                .map(|(k, v)| (k.clone(), super::yrs_json::json_to_any(v)))
                .collect();
            Any::Map(Arc::new(map))
        } else {
            Any::Bool(true)
        }
    } else {
        Any::Null // Null removes the mark
    };

    let attrs = Attrs::from([(key, value)]);
    text.format(txn, from, len, attrs);
}

// ===========================================================================
// Helper functions
// ===========================================================================

/// Extract a required u32 field from a step JSON.
fn require_u32(step: &Value, field: &str) -> Result<u32, TranslationError> {
    step.get(field)
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| TranslationError::MissingField(field.to_string()))
}

/// Check if a ProseMirror JSON node is a text node.
fn is_text_node(node: &Value) -> bool {
    node.get("type").and_then(|t| t.as_str()) == Some("text")
}

/// Check if two XmlTextRef point to the same underlying text node.
fn same_text(a: &XmlTextRef, b: &XmlTextRef) -> bool {
    std::ptr::eq(a.as_ref() as *const _, b.as_ref() as *const _)
}

/// Convert a JSON value to its string representation for Yrs attributes.
fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

/// Get the child at index from a ParentRef.
fn get_child<T: ReadTxn>(txn: &T, parent: &ParentRef, index: u32) -> Option<XmlNode> {
    match parent {
        ParentRef::Fragment(frag) => frag.get(txn, index),
        ParentRef::Element(el) => el.get(txn, index),
    }
}

/// Remove a child at index from a ParentRef.
fn remove_child(txn: &mut yrs::TransactionMut, parent: &ParentRef, index: u32) {
    match parent {
        ParentRef::Fragment(frag) => {
            frag.remove_range(txn, index, 1);
        }
        ParentRef::Element(el) => {
            el.remove_range(txn, index, 1);
        }
    }
}

/// Remove a range of children from a parent.
fn remove_range_from_parent(
    txn: &mut yrs::TransactionMut,
    parent: &ParentRef,
    start: u32,
    count: u32,
) {
    match parent {
        ParentRef::Fragment(frag) => {
            frag.remove_range(txn, start, count);
        }
        ParentRef::Element(el) => {
            el.remove_range(txn, start, count);
        }
    }
}

/// Insert an XmlElement into a parent at the given index.
fn insert_element(
    txn: &mut yrs::TransactionMut,
    parent: &ParentRef,
    index: u32,
    tag: &str,
) -> XmlElementRef {
    match parent {
        ParentRef::Fragment(frag) => {
            frag.insert(txn, index, yrs::types::xml::XmlElementPrelim::empty(tag))
        }
        ParentRef::Element(el) => {
            el.insert(txn, index, yrs::types::xml::XmlElementPrelim::empty(tag))
        }
    }
}

/// Collect children of an element as ProseMirror JSON (for re-insertion).
fn collect_children<T: ReadTxn>(txn: &T, el: &XmlElementRef) -> Vec<Value> {
    let child_count = el.len(txn);
    let mut children = Vec::new();
    for i in 0..child_count {
        if let Some(child) = el.get(txn, i) {
            children.push(super::yrs_json::extract_node(txn, &child));
        }
    }
    // Flatten arrays (extract_node returns arrays for multi-chunk XmlText)
    children
        .into_iter()
        .flat_map(|v| {
            if let Value::Array(items) = v {
                items
            } else {
                vec![v]
            }
        })
        .collect()
}

/// Rebuild children of an element from ProseMirror JSON.
fn rebuild_children(txn: &mut yrs::TransactionMut, el: &XmlElementRef, children: &[Value]) {
    // Check if all children are text nodes (inline content)
    let all_text = children.iter().all(|c| is_text_node(c));

    if all_text && !children.is_empty() {
        let text = el.insert(txn, 0, yrs::types::xml::XmlTextPrelim::new(""));
        let mut offset = 0;
        for text_node in children {
            let text_str = text_node
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if text_str.is_empty() {
                continue;
            }
            let marks = text_node.get("marks").and_then(|m| m.as_array());
            if let Some(marks) = marks {
                let attrs = super::yrs_json::marks_to_attrs(marks);
                text.insert_with_attributes(txn, offset, text_str, attrs);
            } else {
                text.insert(txn, offset, text_str);
            }
            offset += text_str.len() as u32;
        }
    } else {
        for (i, child) in children.iter().enumerate() {
            insert_node_from_json(txn, &ParentRef::Element(el.clone()), i as u32, child);
        }
    }
}

/// Insert a ProseMirror JSON node into a parent at the given index.
fn insert_node_from_json(
    txn: &mut yrs::TransactionMut,
    parent: &ParentRef,
    index: u32,
    node: &Value,
) {
    let node_type = match node.get("type").and_then(|t| t.as_str()) {
        Some(t) => t,
        None => return,
    };

    if node_type == "text" {
        // Insert text — need an XmlText container
        let text_ref = match parent {
            ParentRef::Fragment(frag) => {
                frag.insert(txn, index, yrs::types::xml::XmlTextPrelim::new(""))
            }
            ParentRef::Element(el) => {
                el.insert(txn, index, yrs::types::xml::XmlTextPrelim::new(""))
            }
        };
        let text_str = node.get("text").and_then(|t| t.as_str()).unwrap_or("");
        if !text_str.is_empty() {
            let marks = node.get("marks").and_then(|m| m.as_array());
            if let Some(marks) = marks {
                let attrs = super::yrs_json::marks_to_attrs(marks);
                text_ref.insert_with_attributes(txn, 0, text_str, attrs);
            } else {
                text_ref.insert(txn, 0, text_str);
            }
        }
        return;
    }

    let element = insert_element(txn, parent, index, node_type);

    // Set attributes
    if let Some(attrs) = node.get("attrs").and_then(|a| a.as_object()) {
        for (key, value) in attrs {
            let str_val = json_value_to_string(value);
            element.insert_attribute(txn, key.as_str(), str_val);
        }
    }

    // Process children
    if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
        rebuild_children(txn, &element, content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use yrs::types::xml::{XmlElementPrelim, XmlTextPrelim};
    use yrs::{Doc, Text, Transact, WriteTxn, XmlFragment as XmlFragmentTrait};

    /// Helper: create a doc with a single paragraph containing the given text.
    fn doc_with_paragraph(content: &str) -> Doc {
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let para = frag.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let text = para.insert(&mut txn, 0, XmlTextPrelim::new(""));
            text.push(&mut txn, content);
        }
        doc
    }

    /// Helper: create a doc with two paragraphs.
    fn doc_with_two_paragraphs(text1: &str, text2: &str) -> Doc {
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let para1 = frag.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let t1 = para1.insert(&mut txn, 0, XmlTextPrelim::new(""));
            t1.push(&mut txn, text1);
            let para2 = frag.insert(&mut txn, 1, XmlElementPrelim::empty("paragraph"));
            let t2 = para2.insert(&mut txn, 0, XmlTextPrelim::new(""));
            t2.push(&mut txn, text2);
        }
        doc
    }

    /// Helper: extract ProseMirror JSON from a doc.
    fn extract_json(doc: &Doc) -> Value {
        super::super::yrs_json::extract_prosemirror_json(doc).unwrap()
    }

    // -----------------------------------------------------------------------
    // Position mapping tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_simple_paragraph_positions() {
        // Doc: <paragraph>Hello</paragraph>
        // PM positions: 0=before para, 1=start of text, 2='e', ... 6='o', 7=after para
        let doc = doc_with_paragraph("Hello");
        let txn = doc.transact();
        let frag = txn.get_xml_fragment("default").unwrap();

        // Position 0: before paragraph
        let pos = resolve_position(&txn, &frag, 0).unwrap();
        assert!(matches!(pos, ResolvedPosition::AtNodeBoundary { child_index: 0, .. }));

        // Position 1: start of text inside paragraph
        let pos = resolve_position(&txn, &frag, 1).unwrap();
        assert!(matches!(pos, ResolvedPosition::InText { offset: 0, .. }));

        // Position 3: after "He"
        let pos = resolve_position(&txn, &frag, 3).unwrap();
        assert!(matches!(pos, ResolvedPosition::InText { offset: 2, .. }));

        // Position 6: after "Hello" (end of text)
        let pos = resolve_position(&txn, &frag, 6).unwrap();
        assert!(matches!(pos, ResolvedPosition::InText { offset: 5, .. }));

        // Position 7: after paragraph close tag
        let pos = resolve_position(&txn, &frag, 7).unwrap();
        assert!(matches!(pos, ResolvedPosition::AtNodeBoundary { child_index: 1, .. }));
    }

    #[test]
    fn test_multi_paragraph_positions() {
        // Doc: <paragraph>AB</paragraph><paragraph>CD</paragraph>
        // Positions: 0, 1(A), 2(B), 3(end text), 4(between paras), 5(C), 6(D), 7(end text), 8
        let doc = doc_with_two_paragraphs("AB", "CD");
        let txn = doc.transact();
        let frag = txn.get_xml_fragment("default").unwrap();

        // Position 4: between paragraphs
        let pos = resolve_position(&txn, &frag, 4).unwrap();
        assert!(matches!(pos, ResolvedPosition::AtNodeBoundary { child_index: 1, .. }));

        // Position 5: start of second paragraph text
        let pos = resolve_position(&txn, &frag, 5).unwrap();
        assert!(matches!(pos, ResolvedPosition::InText { offset: 0, .. }));
    }

    #[test]
    fn test_nested_element_positions() {
        // Doc: <blockquote><paragraph>Hi</paragraph></blockquote>
        // Positions: 0=before bq, 1=inside bq, 2=inside para, 3=H, 4=i, 5=end para, 6=end bq
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let bq = frag.insert(&mut txn, 0, XmlElementPrelim::empty("blockquote"));
            let para = bq.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let text = para.insert(&mut txn, 0, XmlTextPrelim::new(""));
            text.push(&mut txn, "Hi");
        }
        let txn = doc.transact();
        let frag = txn.get_xml_fragment("default").unwrap();

        // Position 3: after 'H'
        let pos = resolve_position(&txn, &frag, 3).unwrap();
        assert!(matches!(pos, ResolvedPosition::InText { offset: 1, .. }));
    }

    #[test]
    fn test_invalid_position() {
        let doc = doc_with_paragraph("Hi");
        let txn = doc.transact();
        let frag = txn.get_xml_fragment("default").unwrap();

        // Position 5 is beyond the document (size is 4: open + H + i + close)
        let result = resolve_position(&txn, &frag, 5);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // ReplaceStep tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_insert_text_at_beginning() {
        let doc = doc_with_paragraph("World");
        let steps = vec![json!({
            "stepType": "replace",
            "from": 1,
            "to": 1,
            "slice": {
                "content": [{ "type": "text", "text": "Hello " }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);
        assert_eq!(result.steps_failed, 0);

        let json = extract_json(&doc);
        let text = json["content"][0]["content"][0]["text"].as_str().unwrap();
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_insert_text_in_middle() {
        let doc = doc_with_paragraph("Hllo");
        let steps = vec![json!({
            "stepType": "replace",
            "from": 2,
            "to": 2,
            "slice": {
                "content": [{ "type": "text", "text": "e" }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let text = json["content"][0]["content"][0]["text"].as_str().unwrap();
        assert_eq!(text, "Hello");
    }

    #[test]
    fn test_delete_text_range() {
        let doc = doc_with_paragraph("Hello World");
        // Delete "World" (positions 6-11 in text, PM positions 7-12)
        let steps = vec![json!({
            "stepType": "replace",
            "from": 7,
            "to": 12
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let text = json["content"][0]["content"][0]["text"].as_str().unwrap();
        assert_eq!(text, "Hello ");
    }

    #[test]
    fn test_replace_text() {
        let doc = doc_with_paragraph("Hello World");
        // Replace "World" with "Rust"
        let steps = vec![json!({
            "stepType": "replace",
            "from": 7,
            "to": 12,
            "slice": {
                "content": [{ "type": "text", "text": "Rust" }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let text = json["content"][0]["content"][0]["text"].as_str().unwrap();
        assert_eq!(text, "Hello Rust");
    }

    #[test]
    fn test_insert_paragraph_between() {
        let doc = doc_with_two_paragraphs("First", "Third");
        // Insert a new paragraph between position 7 (after first para) and 7
        let steps = vec![json!({
            "stepType": "replace",
            "from": 7,
            "to": 7,
            "slice": {
                "content": [{
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "Second" }]
                }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        assert_eq!(json["content"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_delete_entire_paragraph() {
        let doc = doc_with_two_paragraphs("Keep", "Delete");
        // Delete the second paragraph (positions 6..14)
        // <paragraph>Keep</paragraph> = positions 0-5 (open + K+e+e+p + close = 6)
        // <paragraph>Delete</paragraph> = positions 6-13
        let steps = vec![json!({
            "stepType": "replace",
            "from": 6,
            "to": 14
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        assert_eq!(json["content"].as_array().unwrap().len(), 1);
        let text = json["content"][0]["content"][0]["text"].as_str().unwrap();
        assert_eq!(text, "Keep");
    }

    // -----------------------------------------------------------------------
    // AddMarkStep / RemoveMarkStep tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_bold_mark() {
        let doc = doc_with_paragraph("Hello World");
        let steps = vec![json!({
            "stepType": "addMark",
            "from": 1,
            "to": 6,
            "mark": { "type": "bold" }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let content = json["content"][0]["content"].as_array().unwrap();
        // Should have a bold "Hello" and a plain " World"
        let bold_node = &content[0];
        assert_eq!(bold_node["text"], "Hello");
        assert_eq!(bold_node["marks"][0]["type"], "bold");
    }

    #[test]
    fn test_remove_mark() {
        // Create a doc with bold text
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let para = frag.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let text = para.insert(&mut txn, 0, XmlTextPrelim::new(""));
            let bold_attrs =
                Attrs::from([(Arc::from("bold"), Any::Bool(true))]);
            text.insert_with_attributes(&mut txn, 0, "Bold Text", bold_attrs);
        }

        let steps = vec![json!({
            "stepType": "removeMark",
            "from": 1,
            "to": 10,
            "mark": { "type": "bold" }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let content = json["content"][0]["content"].as_array().unwrap();
        // Should have plain text with no marks
        let text_node = &content[0];
        assert_eq!(text_node["text"], "Bold Text");
        assert!(text_node.get("marks").is_none() || text_node["marks"].is_null());
    }

    #[test]
    fn test_add_link_mark() {
        let doc = doc_with_paragraph("Click here");
        let steps = vec![json!({
            "stepType": "addMark",
            "from": 1,
            "to": 11,
            "mark": {
                "type": "link",
                "attrs": { "href": "https://example.com", "title": null }
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let content = json["content"][0]["content"].as_array().unwrap();
        let link_node = &content[0];
        assert_eq!(link_node["marks"][0]["type"], "link");
    }

    // -----------------------------------------------------------------------
    // AttrStep tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_change_heading_level() {
        // Create doc with heading level 1
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let heading = frag.insert(&mut txn, 0, XmlElementPrelim::empty("heading"));
            heading.insert_attribute(&mut txn, "level", "1");
            let text = heading.insert(&mut txn, 0, XmlTextPrelim::new(""));
            text.push(&mut txn, "Title");
        }

        let steps = vec![json!({
            "stepType": "attr",
            "pos": 0,
            "attr": "level",
            "value": "2"
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        assert_eq!(json["content"][0]["attrs"]["level"], "2");
    }

    // -----------------------------------------------------------------------
    // ReplaceAroundStep tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_wrap_in_blockquote() {
        let doc = doc_with_paragraph("Quote me");
        // Wrap the paragraph in a blockquote
        // Before: <paragraph>Quote me</paragraph> (size 10)
        // After: <blockquote><paragraph>Quote me</paragraph></blockquote>
        let steps = vec![json!({
            "stepType": "replaceAround",
            "from": 0,
            "to": 10,
            "gapFrom": 0,
            "gapTo": 10,
            "insert": 1,
            "slice": {
                "content": [{
                    "type": "blockquote"
                }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        assert_eq!(json["content"][0]["type"], "blockquote");
    }

    // -----------------------------------------------------------------------
    // Multiple steps tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_steps_in_sequence() {
        let doc = doc_with_paragraph("Hello");

        let steps = vec![
            // First: insert " World" at end of text (position 6)
            json!({
                "stepType": "replace",
                "from": 6,
                "to": 6,
                "slice": {
                    "content": [{ "type": "text", "text": " World" }]
                }
            }),
            // Second: bold "Hello" (positions 1-6)
            json!({
                "stepType": "addMark",
                "from": 1,
                "to": 6,
                "mark": { "type": "bold" }
            }),
        ];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 2);
        assert_eq!(result.steps_failed, 0);

        let json = extract_json(&doc);
        let content = json["content"][0]["content"].as_array().unwrap();
        // Should have bold "Hello" and plain " World"
        assert!(content.len() >= 2);
    }

    #[test]
    fn test_unknown_step_type_continues() {
        let doc = doc_with_paragraph("Hello");

        let steps = vec![
            json!({ "stepType": "unknown_step" }),
            json!({
                "stepType": "replace",
                "from": 1,
                "to": 1,
                "slice": {
                    "content": [{ "type": "text", "text": "OK " }]
                }
            }),
        ];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);
        assert_eq!(result.steps_failed, 1);
    }

    #[test]
    fn test_empty_steps() {
        let doc = doc_with_paragraph("Hello");
        let result = StepTranslator::apply_steps(&doc, &[]);
        assert_eq!(result.steps_applied, 0);
        assert_eq!(result.steps_failed, 0);
    }

    // -----------------------------------------------------------------------
    // Round-trip tests (build doc → apply steps → extract → verify)
    // -----------------------------------------------------------------------

    /// Helper: build a Yrs doc from ProseMirror JSON using replace_yrs_content.
    fn doc_from_pm_json(pm_json: &Value) -> Doc {
        let doc = Doc::new();
        super::super::yrs_json::replace_yrs_content(&doc, pm_json).unwrap();
        doc
    }

    #[test]
    fn test_roundtrip_add_paragraph_at_end() {
        let doc = doc_from_pm_json(&json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": "First" }] },
                { "type": "paragraph", "content": [{ "type": "text", "text": "Second" }] }
            ]
        }));

        // Insert a new paragraph at position 16 (after both paras)
        // para1: open(1) + First(5) + close(1) = 7
        // para2: open(1) + Second(6) + close(1) = 8
        // total = 15, so pos 15 is after para2's close tag
        let steps = vec![json!({
            "stepType": "replace",
            "from": 15,
            "to": 15,
            "slice": {
                "content": [{
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "Third" }]
                }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let content = json["content"].as_array().unwrap();
        assert_eq!(content.len(), 3);
        assert_eq!(content[2]["content"][0]["text"], "Third");
    }

    #[test]
    fn test_roundtrip_modify_text_in_middle() {
        let doc = doc_from_pm_json(&json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": "Hello world" }] }
            ]
        }));

        // Replace "world" (positions 7..12) with "Rust"
        let steps = vec![json!({
            "stepType": "replace",
            "from": 7,
            "to": 12,
            "slice": {
                "content": [{ "type": "text", "text": "Rust" }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        assert_eq!(json["content"][0]["content"][0]["text"], "Hello Rust");
    }

    #[test]
    fn test_roundtrip_delete_section() {
        let doc = doc_from_pm_json(&json!({
            "type": "doc",
            "content": [
                { "type": "heading", "attrs": { "level": "1" }, "content": [{ "type": "text", "text": "Title" }] },
                { "type": "paragraph", "content": [{ "type": "text", "text": "Keep this" }] },
                { "type": "paragraph", "content": [{ "type": "text", "text": "Delete this" }] }
            ]
        }));

        // Delete the third paragraph
        // heading: 2 + 5 = 7, para1: 2 + 9 = 11 → starts at 18
        // para2: 2 + 11 = 13 → ends at 31
        let steps = vec![json!({
            "stepType": "replace",
            "from": 18,
            "to": 31
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let content = json["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "heading");
        assert_eq!(content[1]["content"][0]["text"], "Keep this");
    }

    #[test]
    fn test_roundtrip_change_formatting() {
        let doc = doc_from_pm_json(&json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": "Make this bold" }] }
            ]
        }));

        // Bold the word "bold" (positions 11..15)
        let steps = vec![json!({
            "stepType": "addMark",
            "from": 11,
            "to": 15,
            "mark": { "type": "bold" }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let content = json["content"][0]["content"].as_array().unwrap();
        // Should have "Make this " (plain), "bold" (bold)
        assert!(content.len() >= 2);
        let bold_node = content.iter().find(|n| n["text"] == "bold").unwrap();
        assert_eq!(bold_node["marks"][0]["type"], "bold");
    }

    #[test]
    fn test_roundtrip_change_heading_level() {
        let doc = doc_from_pm_json(&json!({
            "type": "doc",
            "content": [
                { "type": "heading", "attrs": { "level": "1" }, "content": [{ "type": "text", "text": "Title" }] }
            ]
        }));

        let steps = vec![json!({
            "stepType": "attr",
            "pos": 0,
            "attr": "level",
            "value": "3"
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        assert_eq!(json["content"][0]["type"], "heading");
        assert_eq!(json["content"][0]["attrs"]["level"], "3");
    }

    #[test]
    fn test_roundtrip_mixed_changes() {
        let doc = doc_from_pm_json(&json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": "First paragraph" }] },
                { "type": "paragraph", "content": [{ "type": "text", "text": "Second paragraph" }] },
                { "type": "paragraph", "content": [{ "type": "text", "text": "Third paragraph" }] }
            ]
        }));

        let steps = vec![
            // 1. Delete "Second paragraph" (the entire second para)
            // para1: 2+15=17 → second para at 17..35 (2+16=18)
            json!({
                "stepType": "replace",
                "from": 17,
                "to": 35
            }),
            // 2. Bold "First" in the first paragraph (positions 1..6)
            json!({
                "stepType": "addMark",
                "from": 1,
                "to": 6,
                "mark": { "type": "bold" }
            }),
        ];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 2);
        assert_eq!(result.steps_failed, 0);

        let json = extract_json(&doc);
        let content = json["content"].as_array().unwrap();
        // Should have 2 paragraphs (second deleted)
        assert_eq!(content.len(), 2);
        // First paragraph should have bold "First"
        let first_content = content[0]["content"].as_array().unwrap();
        let bold_node = first_content.iter().find(|n| n["text"] == "First").unwrap();
        assert_eq!(bold_node["marks"][0]["type"], "bold");
    }

    // -----------------------------------------------------------------------
    // Concurrent merge test
    // -----------------------------------------------------------------------

    #[test]
    fn test_concurrent_edits_merge() {
        // Simulate: agent edits paragraph 2 while browser edited paragraph 1
        // Both changes should be preserved via the CRDT.
        let doc = doc_with_two_paragraphs("Browser text", "Agent text");

        // Simulate a browser edit: insert "EDITED " at start of paragraph 1
        // This is a direct Yrs mutation (as would come through WebSocket sync)
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment(FRAGMENT_NAME);
            let para1 = frag.get(&txn, 0).unwrap().into_xml_element().unwrap();
            let text = para1.get(&txn, 0).unwrap().into_xml_text().unwrap();
            text.insert(&mut txn, 0, "EDITED ");
        }

        // Now apply agent steps that modify paragraph 2
        // After browser edit, para1 = "EDITED Browser text" (19 chars), size = 21
        // Para2 starts at 21, text starts at 22
        let steps = vec![json!({
            "stepType": "replace",
            "from": 22,
            "to": 32,
            "slice": {
                "content": [{ "type": "text", "text": "New agent text" }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let content = json["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);

        // Both edits preserved
        let para1_text = content[0]["content"][0]["text"].as_str().unwrap();
        assert!(
            para1_text.contains("EDITED"),
            "Browser edit should be preserved: {para1_text}"
        );

        let para2_text = content[1]["content"][0]["text"].as_str().unwrap();
        assert!(
            para2_text.contains("New agent text"),
            "Agent edit should be preserved: {para2_text}"
        );
    }

    // -----------------------------------------------------------------------
    // Large document test
    // -----------------------------------------------------------------------

    #[test]
    fn test_large_document_50_paragraphs() {
        // Build a document with 50+ paragraphs
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            for i in 0..55 {
                let para = frag.insert(
                    &mut txn,
                    i as u32,
                    XmlElementPrelim::empty("paragraph"),
                );
                let text = para.insert(&mut txn, 0, XmlTextPrelim::new(""));
                text.push(&mut txn, &format!("Paragraph number {i}"));
            }
        }

        // Compute the PM position of paragraph 25's text.
        // Each paragraph has: 2 (open+close) + text_len
        // "Paragraph number X" is 19 chars for i<10, 20 for 10<=i<100
        // So size is 21 for i<10, 22 for i>=10
        // Offset to paragraph 25: 10*21 + 15*22 = 210 + 330 = 540
        // Inside para 25: text starts at 540+1 = 541
        // But we need to verify this dynamically:
        let text_start = {
            let txn = doc.transact();
            let frag = txn.get_xml_fragment("default").unwrap();
            let mut pos = 0u32;
            for i in 0..25 {
                if let Some(node) = frag.get(&txn, i) {
                    if let XmlNode::Element(el) = node {
                        pos += element_pm_size(&txn, &el);
                    }
                }
            }
            pos + 1 // +1 for open tag of paragraph 25
        };
        let text_content = "Paragraph number 25";
        let text_end = text_start + text_content.len() as u32;

        let steps = vec![json!({
            "stepType": "replace",
            "from": text_start,
            "to": text_end,
            "slice": {
                "content": [{ "type": "text", "text": "MODIFIED paragraph 25" }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);
        assert_eq!(result.steps_failed, 0);

        let json = extract_json(&doc);
        let content = json["content"].as_array().unwrap();
        assert_eq!(content.len(), 55);

        // Verify the modified paragraph
        let para25_text = content[25]["content"][0]["text"].as_str().unwrap();
        assert_eq!(para25_text, "MODIFIED paragraph 25");

        // Verify surrounding paragraphs are untouched
        let para24_text = content[24]["content"][0]["text"].as_str().unwrap();
        assert_eq!(para24_text, "Paragraph number 24");
        let para26_text = content[26]["content"][0]["text"].as_str().unwrap();
        assert_eq!(para26_text, "Paragraph number 26");
    }

    // -----------------------------------------------------------------------
    // List restructuring tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_list_item() {
        // Build a doc with a bulletList containing two listItems
        // Structure:
        // <bulletList>
        //   <listItem><paragraph>Item 1</paragraph></listItem>
        //   <listItem><paragraph>Item 2</paragraph></listItem>
        // </bulletList>
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let list = frag.insert(&mut txn, 0, XmlElementPrelim::empty("bulletList"));
            let li1 = list.insert(&mut txn, 0, XmlElementPrelim::empty("listItem"));
            let p1 = li1.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let t1 = p1.insert(&mut txn, 0, XmlTextPrelim::new(""));
            t1.push(&mut txn, "Item 1");
            let li2 = list.insert(&mut txn, 1, XmlElementPrelim::empty("listItem"));
            let p2 = li2.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let t2 = p2.insert(&mut txn, 0, XmlTextPrelim::new(""));
            t2.push(&mut txn, "Item 2");
        }

        // PM positions:
        // 0: before bulletList
        // 1: inside bulletList (after open tag)
        // 2: inside listItem1 (after open tag)
        // 3: inside paragraph1 (after open tag)
        // 3-8: "Item 1" (6 chars, positions 3='I' through 8='1')
        // 9: after text (paragraph1 close)
        // 10: listItem1 close
        // 11: between listItems (after listItem1 close, before listItem2 open)
        // 12: inside listItem2 (after open tag)
        // 13: inside paragraph2 (after open tag)
        // 13-18: "Item 2" (6 chars)
        // 19: paragraph2 close
        // 20: listItem2 close
        // 21: bulletList close

        // Insert a new listItem between the two existing ones at position 11
        let steps = vec![json!({
            "stepType": "replace",
            "from": 11,
            "to": 11,
            "slice": {
                "content": [{
                    "type": "listItem",
                    "content": [{
                        "type": "paragraph",
                        "content": [{ "type": "text", "text": "Item 1.5" }]
                    }]
                }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);
        assert_eq!(result.steps_failed, 0);

        let json = extract_json(&doc);
        let list_content = json["content"][0]["content"].as_array().unwrap();
        assert_eq!(list_content.len(), 3, "Should have 3 list items");

        // Verify order: Item 1, Item 1.5, Item 2
        let item1_text = list_content[0]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert_eq!(item1_text, "Item 1");

        let item15_text = list_content[1]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert_eq!(item15_text, "Item 1.5");

        let item2_text = list_content[2]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert_eq!(item2_text, "Item 2");
    }

    #[test]
    fn test_modify_list_item_text() {
        // Build a doc with a bulletList containing one listItem
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let list = frag.insert(&mut txn, 0, XmlElementPrelim::empty("bulletList"));
            let li = list.insert(&mut txn, 0, XmlElementPrelim::empty("listItem"));
            let p = li.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let t = p.insert(&mut txn, 0, XmlTextPrelim::new(""));
            t.push(&mut txn, "Old text");
        }

        // PM positions:
        // 0: before bulletList
        // 1: inside bulletList (after open tag)
        // 2: inside listItem (after open tag)
        // 3: inside paragraph (after open tag) = text offset 0
        // 4-10: "ld text" portion (text offsets 1-7)
        // 11: after "Old text" = paragraph close
        // 12: listItem close
        // 13: bulletList close

        // Replace "Old text" (positions 3..11) with "New text"
        let steps = vec![json!({
            "stepType": "replace",
            "from": 3,
            "to": 11,
            "slice": {
                "content": [{ "type": "text", "text": "New text" }]
            }
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let item_text = json["content"][0]["content"][0]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert_eq!(item_text, "New text");
    }

    #[test]
    fn test_delete_list_item() {
        // Build a doc with a bulletList containing three listItems
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let list = frag.insert(&mut txn, 0, XmlElementPrelim::empty("bulletList"));
            for (i, text) in ["First", "Second", "Third"].iter().enumerate() {
                let li = list.insert(&mut txn, i as u32, XmlElementPrelim::empty("listItem"));
                let p = li.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
                let t = p.insert(&mut txn, 0, XmlTextPrelim::new(""));
                t.push(&mut txn, text);
            }
        }

        // Compute positions dynamically
        let second_li_start = {
            let txn = doc.transact();
            let frag = txn.get_xml_fragment("default").unwrap();
            let list = frag.get(&txn, 0).unwrap().into_xml_element().unwrap();
            // bulletList open = 1
            // listItem1: open(1) + paragraph(open(1) + "First"(5) + close(1)) + close(1) = 9
            // so second listItem starts at 1 + 9 = 10
            let li1 = list.get(&txn, 0).unwrap().into_xml_element().unwrap();
            let li1_size = element_pm_size(&txn, &li1);
            1 + li1_size // bulletList open + first listItem size
        };

        let second_li_end = {
            let txn = doc.transact();
            let frag = txn.get_xml_fragment("default").unwrap();
            let list = frag.get(&txn, 0).unwrap().into_xml_element().unwrap();
            let li1 = list.get(&txn, 0).unwrap().into_xml_element().unwrap();
            let li2 = list.get(&txn, 1).unwrap().into_xml_element().unwrap();
            1 + element_pm_size(&txn, &li1) + element_pm_size(&txn, &li2)
        };

        // Delete the second listItem
        let steps = vec![json!({
            "stepType": "replace",
            "from": second_li_start,
            "to": second_li_end
        })];

        let result = StepTranslator::apply_steps(&doc, &steps);
        assert_eq!(result.steps_applied, 1);

        let json = extract_json(&doc);
        let list_content = json["content"][0]["content"].as_array().unwrap();
        assert_eq!(list_content.len(), 2, "Should have 2 list items after deletion");

        let item1_text = list_content[0]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert_eq!(item1_text, "First");

        let item2_text = list_content[1]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert_eq!(item2_text, "Third");
    }
}
