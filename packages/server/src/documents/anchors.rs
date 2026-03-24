use yrs::branch::BranchPtr;
use yrs::types::xml::{XmlFragment, XmlNode};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Assoc, Doc, IndexedSequence, ReadTxn, Text, Transact, WriteTxn, XmlFragmentRef};

use crate::errors::AppError;

/// The name of the XmlFragment where TipTap stores document content.
const FRAGMENT_NAME: &str = "default";

/// Convert a ProseMirror character offset to a Yrs StickyIndex (binary).
///
/// ProseMirror positions count +1 for each node open tag (excluding the root
/// doc), +1 for each text character, and +1 for each node close tag (excluding
/// the root doc). This function walks the Yrs XML fragment tree using that
/// same counting scheme and creates a StickyIndex at the corresponding
/// location.
pub fn pm_offset_to_sticky_bytes(doc: &Doc, pm_offset: u32) -> Result<Vec<u8>, AppError> {
    let mut txn = doc.transact_mut();
    let fragment = txn
        .get_or_insert_xml_fragment(FRAGMENT_NAME);

    // Walk the fragment to find which XmlText node and local offset
    // corresponds to the given PM offset.
    let result = walk_fragment_to_offset(&txn, &fragment, pm_offset)?;

    match result {
        OffsetTarget::InText { text_ref, local_offset } => {
            // At the end of text, use Assoc::Before (pointing back to the last char).
            // At other positions, use Assoc::After (pointing forward).
            let text_len = text_ref.len(&txn);
            let assoc = if local_offset >= text_len {
                Assoc::Before
            } else {
                Assoc::After
            };
            let sticky = text_ref
                .sticky_index(&mut txn, local_offset, assoc)
                .ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "Cannot create sticky index at text offset {}",
                        local_offset
                    ))
                })?;
            Ok(sticky.encode_v1())
        }
        OffsetTarget::AtNodeBoundary { fragment_ref, child_index } => {
            // Position is at a node boundary (between/before/after block nodes).
            // Use the fragment's own sticky_index at the child_index.
            let sticky = fragment_ref
                .sticky_index(&mut txn, child_index, Assoc::Before)
                .ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "Cannot create sticky index at node boundary {}",
                        child_index
                    ))
                })?;
            Ok(sticky.encode_v1())
        }
    }
}

/// Convert a Yrs StickyIndex (binary) back to a ProseMirror character offset.
pub fn sticky_bytes_to_pm_offset(doc: &Doc, bytes: &[u8]) -> Option<u32> {
    let sticky = yrs::StickyIndex::decode_v1(bytes).ok()?;
    let txn = doc.transact();

    let fragment = txn.get_xml_fragment(FRAGMENT_NAME)?;

    // Walk the tree, accumulating PM positions, to find what PM offset
    // corresponds to the resolved absolute index.
    //
    // The StickyIndex resolves to an index within whichever branch (XmlText
    // or XmlFragment/XmlElement) it was created on. We need to identify which
    // branch it points to and convert that back to a PM offset.
    //
    // Strategy: walk the entire tree counting PM offsets, and for each text
    // node check if the resolved StickyIndex's branch matches. If so, compute
    // the PM offset from the accumulated count + local offset.
    resolve_pm_offset(&txn, &fragment, &sticky)
}

/// Internal: where a PM offset lands in the Yrs tree.
enum OffsetTarget<'a> {
    /// The offset lands inside an XmlText node.
    InText {
        text_ref: yrs::XmlTextRef,
        local_offset: u32,
    },
    /// The offset lands at a node boundary (e.g., right before/after a block element).
    AtNodeBoundary {
        fragment_ref: &'a XmlFragmentRef,
        child_index: u32,
    },
}

/// Walk the XML fragment tree depth-first, counting ProseMirror-style positions.
///
/// ProseMirror position model (from the root doc's perspective):
/// - Position 0 is at the very start of the document content (inside the doc node).
/// - Each child element of the doc (e.g., a paragraph) adds +1 for its opening tag.
/// - Text characters each add +1.
/// - Each child element adds +1 for its closing tag.
/// - Nested elements (e.g., list items inside a list) follow the same pattern recursively.
fn walk_fragment_to_offset<'a, T: ReadTxn>(
    txn: &T,
    fragment: &'a XmlFragmentRef,
    target: u32,
) -> Result<OffsetTarget<'a>, AppError> {
    let mut pm_pos: u32 = 0;
    let child_count = fragment.len(txn);

    // Position 0 (or any position at the very start) is a node boundary.
    if target == 0 {
        return Ok(OffsetTarget::AtNodeBoundary {
            fragment_ref: fragment,
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
                        return find_in_element(txn, &el, target, pm_pos);
                    }
                    pm_pos += el_size;

                    // Position right after this element's close tag = between blocks
                    if target == pm_pos {
                        return Ok(OffsetTarget::AtNodeBoundary {
                            fragment_ref: fragment,
                            child_index: i + 1,
                        });
                    }
                }
                XmlNode::Text(text) => {
                    let text_len = text.len(txn);
                    if target >= pm_pos && target <= pm_pos + text_len {
                        return Ok(OffsetTarget::InText {
                            text_ref: text,
                            local_offset: target - pm_pos,
                        });
                    }
                    pm_pos += text_len;
                }
                XmlNode::Fragment(_) => {}
            }
        }
    }

    // If target == pm_pos, it's at the very end of the document
    if target == pm_pos {
        return Ok(OffsetTarget::AtNodeBoundary {
            fragment_ref: fragment,
            child_index: child_count,
        });
    }

    Err(AppError::BadRequest(format!(
        "Position {} is beyond the document (size {})",
        target, pm_pos
    )))
}

/// Recursively find a PM offset within a nested XmlElement.
/// `pm_pos` is the PM position of this element's open tag.
fn find_in_element<T: ReadTxn>(
    txn: &T,
    el: &yrs::XmlElementRef,
    target: u32,
    pm_pos: u32,
) -> Result<OffsetTarget<'static>, AppError> {
    // After open tag
    let mut pos = pm_pos + 1;

    let child_count = el.len(txn);
    for j in 0..child_count {
        if let Some(child) = el.get(txn, j) {
            match child {
                XmlNode::Text(text) => {
                    let text_len = text.len(txn);
                    if target >= pos && target <= pos + text_len {
                        return Ok(OffsetTarget::InText {
                            text_ref: text,
                            local_offset: target - pos,
                        });
                    }
                    pos += text_len;
                }
                XmlNode::Element(nested_el) => {
                    let nested_size = element_pm_size(txn, &nested_el);
                    if target >= pos && target < pos + nested_size {
                        return find_in_element(txn, &nested_el, target, pos);
                    }
                    pos += nested_size;
                }
                XmlNode::Fragment(_) => {}
            }
        }
    }

    // Target is at or beyond the close tag position — shouldn't happen
    // if element_pm_size is correct, but handle gracefully
    Err(AppError::BadRequest(format!(
        "Could not resolve position {} in nested element",
        target
    )))
}

/// Calculate the ProseMirror size of an element (open tag + content + close tag).
fn element_pm_size<T: ReadTxn>(txn: &T, el: &yrs::XmlElementRef) -> u32 {
    let mut size: u32 = 2; // open + close tags
    let child_count = el.len(txn);
    for i in 0..child_count {
        if let Some(child) = el.get(txn, i) {
            match child {
                XmlNode::Text(text) => {
                    size += text.len(txn);
                }
                XmlNode::Element(nested) => {
                    size += element_pm_size(txn, &nested);
                }
                XmlNode::Fragment(_) => {}
            }
        }
    }
    size
}

/// Walk the tree to resolve a StickyIndex back to a PM offset.
fn resolve_pm_offset<T: ReadTxn>(
    txn: &T,
    fragment: &XmlFragmentRef,
    sticky: &yrs::StickyIndex,
) -> Option<u32> {
    let offset = sticky.get_offset(txn)?;
    let target_index = offset.index;

    // Use the Offset's branch pointer to identify which Yrs node the
    // StickyIndex resolved in, then walk the tree to compute the PM offset.
    let target_branch = offset.branch;

    let mut pm_pos: u32 = 0;
    let child_count = fragment.len(txn);

    for i in 0..child_count {
        if let Some(node) = fragment.get(txn, i) {
            match node {
                XmlNode::Element(el) => {
                    if let Some(result) =
                        resolve_in_element(txn, &el, target_branch, target_index, &mut pm_pos)
                    {
                        return Some(result);
                    }
                }
                XmlNode::Text(text) => {
                    let text_branch = BranchPtr::from(text.as_ref());
                    if text_branch == target_branch {
                        return Some(pm_pos + target_index);
                    }
                    pm_pos += text.len(txn);
                }
                XmlNode::Fragment(_) => {}
            }
        }
    }

    // If the StickyIndex points to the fragment itself (node boundary)
    let frag_branch = BranchPtr::from(fragment.as_ref());
    if frag_branch == target_branch {
        // The index refers to a child position in the fragment.
        // Walk children to accumulate PM offsets up to that child.
        let mut pm = 0u32;
        for i in 0..target_index.min(child_count) {
            if let Some(node) = fragment.get(txn, i) {
                match node {
                    XmlNode::Element(el) => {
                        pm += element_pm_size(txn, &el);
                    }
                    XmlNode::Text(text) => {
                        pm += text.len(txn);
                    }
                    XmlNode::Fragment(_) => {}
                }
            }
        }
        return Some(pm);
    }

    None
}

/// Recursively resolve a PM offset within an element.
fn resolve_in_element<T: ReadTxn>(
    txn: &T,
    el: &yrs::XmlElementRef,
    target_branch: BranchPtr,
    target_index: u32,
    pm_pos: &mut u32,
) -> Option<u32> {
    // Check if the element itself is the target branch
    let el_branch = BranchPtr::from(el.as_ref());
    if el_branch == target_branch {
        // The sticky index points to a child position within this element.
        // Accumulate PM offsets for children up to target_index.
        let mut inner_pm = *pm_pos + 1; // +1 for open tag
        let child_count = el.len(txn);
        for i in 0..target_index.min(child_count) {
            if let Some(child) = el.get(txn, i) {
                match child {
                    XmlNode::Text(text) => {
                        inner_pm += text.len(txn);
                    }
                    XmlNode::Element(nested) => {
                        inner_pm += element_pm_size(txn, &nested);
                    }
                    XmlNode::Fragment(_) => {}
                }
            }
        }
        // Update pm_pos to include this whole element before returning
        *pm_pos += element_pm_size(txn, el);
        return Some(inner_pm);
    }

    // Opening tag: +1
    *pm_pos += 1;

    let child_count = el.len(txn);
    for j in 0..child_count {
        if let Some(child) = el.get(txn, j) {
            match child {
                XmlNode::Text(text) => {
                    let text_branch = BranchPtr::from(text.as_ref());
                    if text_branch == target_branch {
                        return Some(*pm_pos + target_index);
                    }
                    *pm_pos += text.len(txn);
                }
                XmlNode::Element(nested_el) => {
                    if let Some(result) = resolve_in_element(
                        txn,
                        &nested_el,
                        target_branch,
                        target_index,
                        pm_pos,
                    ) {
                        return Some(result);
                    }
                }
                XmlNode::Fragment(_) => {}
            }
        }
    }

    // Closing tag: +1
    *pm_pos += 1;

    None
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_simple_paragraph_roundtrip() {
        // Doc: <paragraph>Hello</paragraph>
        // PM positions: 0=before para, 1=open tag, 2=H, 3=e, 4=l, 5=l, 6=o, 7=close tag
        // But PM counting from doc root:
        // pos 0: start of doc content
        // pos 1: inside paragraph, before 'H' (after open tag)
        // pos 2: after 'H', pos 3: after 'e', etc.
        // pos 6: after 'o' (end of text)
        // pos 7: after paragraph close tag
        let doc = doc_with_paragraph("Hello");

        // Position 1 = start of text inside paragraph
        let bytes = pm_offset_to_sticky_bytes(&doc, 1).unwrap();
        let resolved = sticky_bytes_to_pm_offset(&doc, &bytes).unwrap();
        assert_eq!(resolved, 1);

        // Position 3 = after "He"
        let bytes = pm_offset_to_sticky_bytes(&doc, 3).unwrap();
        let resolved = sticky_bytes_to_pm_offset(&doc, &bytes).unwrap();
        assert_eq!(resolved, 3);

        // Position 6 = after "Hello"
        let bytes = pm_offset_to_sticky_bytes(&doc, 6).unwrap();
        let resolved = sticky_bytes_to_pm_offset(&doc, &bytes).unwrap();
        assert_eq!(resolved, 6);
    }

    #[test]
    fn test_two_paragraphs_roundtrip() {
        // Doc: <paragraph>AB</paragraph><paragraph>CD</paragraph>
        // PM positions:
        // 0: start of doc
        // 1: inside para1 (open tag done), before 'A'
        // 2: after 'A'
        // 3: after 'B' (end of text in para1)
        // 4: after para1 close tag = before para2 open tag
        // 5: inside para2, before 'C'
        // 6: after 'C'
        // 7: after 'D'
        // 8: after para2 close tag
        let doc = doc_with_two_paragraphs("AB", "CD");

        for pos in [1, 2, 3, 5, 6, 7] {
            let bytes = pm_offset_to_sticky_bytes(&doc, pos).unwrap();
            let resolved = sticky_bytes_to_pm_offset(&doc, &bytes).unwrap();
            assert_eq!(resolved, pos, "roundtrip failed for position {}", pos);
        }
    }

    #[test]
    fn test_offset_survives_insert_before() {
        // Create doc with "Hello world" and anchor to position of 'w' (pm_offset=7)
        // Then insert "XY" at the beginning of the text
        // The anchor should now resolve to pm_offset=9 (shifted by 2)
        let doc = doc_with_paragraph("Hello world");

        // Anchor at 'w' = pm_offset 7 (1 for open tag + 6 chars "Hello ")
        let bytes = pm_offset_to_sticky_bytes(&doc, 7).unwrap();

        // Insert "XY" at the beginning of the text
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let para = frag.get(&txn, 0).unwrap().into_xml_element().unwrap();
            let text = para.get(&txn, 0).unwrap().into_xml_text().unwrap();
            text.insert(&mut txn, 0, "XY");
        }

        let resolved = sticky_bytes_to_pm_offset(&doc, &bytes).unwrap();
        // "XY" pushed 'w' 2 positions forward: 7 → 9
        assert_eq!(resolved, 9);
    }

    #[test]
    fn test_offset_after_deletion() {
        // Create doc with "Hello world", anchor to 'w' (pm_offset=7)
        // Delete "Hello " (6 chars from index 0)
        // The anchor should resolve to pm_offset=1 (the 'w' is now at start of text)
        let doc = doc_with_paragraph("Hello world");

        let bytes = pm_offset_to_sticky_bytes(&doc, 7).unwrap();

        // Delete "Hello " from the text
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let para = frag.get(&txn, 0).unwrap().into_xml_element().unwrap();
            let text = para.get(&txn, 0).unwrap().into_xml_text().unwrap();
            text.remove_range(&mut txn, 0, 6);
        }

        let resolved = sticky_bytes_to_pm_offset(&doc, &bytes).unwrap();
        // 'w' is now the first char, so pm_offset=1 (after open tag)
        assert_eq!(resolved, 1);
    }

    #[test]
    fn test_empty_paragraph() {
        // Doc with empty paragraph: <paragraph></paragraph>
        // PM positions: 0=before para, 1=inside para (the cursor position), 2=after para
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            frag.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        }

        // Position 0 should work (before the paragraph)
        let bytes = pm_offset_to_sticky_bytes(&doc, 0).unwrap();
        let resolved = sticky_bytes_to_pm_offset(&doc, &bytes).unwrap();
        assert_eq!(resolved, 0);
    }

    #[test]
    fn test_nested_content() {
        // Doc with a blockquote containing a paragraph:
        // <blockquote><paragraph>Hi</paragraph></blockquote>
        // PM positions:
        // 0: before blockquote
        // 1: inside blockquote (open tag)
        // 2: inside paragraph (open tag)
        // 3: after 'H'
        // 4: after 'i'
        // 5: paragraph close tag
        // 6: blockquote close tag
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let bq = frag.insert(&mut txn, 0, XmlElementPrelim::empty("blockquote"));
            let para = bq.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let text = para.insert(&mut txn, 0, XmlTextPrelim::new(""));
            text.push(&mut txn, "Hi");
        }

        // Position 3 = after 'H'
        let bytes = pm_offset_to_sticky_bytes(&doc, 3).unwrap();
        let resolved = sticky_bytes_to_pm_offset(&doc, &bytes).unwrap();
        assert_eq!(resolved, 3);
    }

    #[test]
    fn test_position_beyond_doc_returns_error() {
        let doc = doc_with_paragraph("Hi");
        // Doc size is 4 (open + 'H' + 'i' + close), so position 5 is invalid
        let result = pm_offset_to_sticky_bytes(&doc, 5);
        assert!(result.is_err());
    }
}
