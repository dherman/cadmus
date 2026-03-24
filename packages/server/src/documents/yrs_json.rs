use serde_json::{json, Value};
use yrs::types::text::Diff;
use yrs::types::xml::{XmlFragment, XmlNode};
use yrs::types::{Attrs, Value as YrsValue};
use yrs::{Any, Doc, ReadTxn, Text, Transact, Xml, XmlElementRef, XmlTextRef};

/// Extract ProseMirror JSON from a Yrs document.
///
/// TipTap's Collaboration extension stores document content in an XmlFragment
/// named "default". This function walks the Yrs XML tree and produces the
/// equivalent ProseMirror JSON representation.
pub fn extract_prosemirror_json(doc: &Doc) -> Result<Value, String> {
    let txn = doc.transact();

    let fragment = txn
        .get_xml_fragment("default")
        .ok_or_else(|| "No default fragment found".to_string())?;

    let child_count = fragment.len(&txn);
    if child_count == 0 {
        return Ok(empty_doc_json());
    }

    let mut children = Vec::new();
    for i in 0..child_count {
        if let Some(node) = fragment.get(&txn, i) {
            children.push(extract_node(&txn, &node));
        }
    }

    if children.is_empty() {
        return Ok(empty_doc_json());
    }

    Ok(json!({
        "type": "doc",
        "content": children
    }))
}

/// Default empty document JSON for documents with no CRDT content.
pub fn empty_doc_json() -> Value {
    json!({
        "type": "doc",
        "content": [{ "type": "paragraph" }]
    })
}

/// Extract a single XmlNode into ProseMirror JSON.
fn extract_node<T: ReadTxn>(txn: &T, node: &XmlNode) -> Value {
    match node {
        XmlNode::Element(el) => extract_element(txn, el),
        XmlNode::Text(text) => extract_text_node(txn, text),
        XmlNode::Fragment(_) => {
            // Nested fragments shouldn't appear in normal y-prosemirror docs
            json!({ "type": "paragraph" })
        }
    }
}

/// Extract an XmlElement into a ProseMirror node JSON object.
fn extract_element<T: ReadTxn>(txn: &T, el: &XmlElementRef) -> Value {
    let tag = el.tag().to_string();

    // Collect attributes
    let mut attrs = serde_json::Map::new();
    for (key, value) in el.attributes(txn) {
        attrs.insert(key.to_string(), Value::String(value));
    }

    // Collect children, flattening XmlText nodes that expand to multiple text nodes
    let child_count = el.len(txn);
    let mut children = Vec::new();
    for i in 0..child_count {
        if let Some(child) = el.get(txn, i) {
            let value = extract_node(txn, &child);
            // XmlText returns an array when it has multiple formatted chunks —
            // flatten those into the parent's content array
            if let Value::Array(nodes) = value {
                children.extend(nodes);
            } else {
                children.push(value);
            }
        }
    }

    let mut node = serde_json::Map::new();
    node.insert("type".to_string(), Value::String(tag));

    if !attrs.is_empty() {
        node.insert("attrs".to_string(), Value::Object(attrs));
    }

    if !children.is_empty() {
        node.insert("content".to_string(), Value::Array(children));
    }

    Value::Object(node)
}

/// Extract an XmlText into one or more ProseMirror text nodes.
///
/// y-prosemirror represents inline content as XmlText with formatting deltas.
/// Each delta chunk becomes a separate ProseMirror text node with marks.
fn extract_text_node<T: ReadTxn>(txn: &T, text: &XmlTextRef) -> Value {
    // Use diff() to get formatted text chunks
    let diffs: Vec<Diff<()>> = text.diff(txn, |_| ());

    if diffs.is_empty() {
        return json!({ "type": "text", "text": "" });
    }

    // If there's a single chunk with no marks, return a simple text node
    if diffs.len() == 1 {
        return diff_to_text_node(&diffs[0]);
    }

    // Multiple chunks: we need to return them as an array, but ProseMirror
    // expects text nodes to be siblings in a content array. Since this function
    // returns a single Value, we wrap multiple text nodes in a marker that the
    // caller can flatten. However, the standard approach is that XmlText is always
    // a child of an XmlElement, and the parent element's content array will contain
    // these. We return a JSON array that the parent can flatten.
    //
    // Actually, looking at the ProseMirror JSON format more carefully: an XmlText
    // in y-prosemirror represents all inline content within a block node. The parent
    // XmlElement (e.g., "paragraph") contains a single XmlText child, and that
    // XmlText's diff chunks become the multiple text nodes in the parent's content array.
    //
    // Since extract_element calls extract_node for each child and pushes to children,
    // we need a way to expand a single XmlText into multiple text nodes. We'll use a
    // special wrapper that extract_element can detect and flatten.
    let nodes: Vec<Value> = diffs.iter().map(diff_to_text_node).collect();

    // Return as an array — the caller (extract_element) needs to handle flattening
    Value::Array(nodes)
}

/// Convert a single Diff chunk into a ProseMirror text node.
fn diff_to_text_node(diff: &Diff<()>) -> Value {
    let text_str = match &diff.insert {
        YrsValue::Any(Any::String(s)) => s.to_string(),
        _ => return json!({ "type": "text", "text": "" }),
    };

    let mut node = serde_json::Map::new();
    node.insert("type".to_string(), Value::String("text".to_string()));
    node.insert("text".to_string(), Value::String(text_str));

    if let Some(ref attrs) = diff.attributes {
        let marks = attrs_to_marks(attrs);
        if !marks.is_empty() {
            node.insert("marks".to_string(), Value::Array(marks));
        }
    }

    Value::Object(node)
}

/// Convert Yrs formatting attributes to ProseMirror marks array.
///
/// In y-prosemirror, formatting attributes on text diffs correspond to
/// ProseMirror marks. Simple boolean marks (bold, italic, etc.) are stored
/// as `{ "bold": Any::Bool(true) }`. Complex marks like links have
/// attribute values stored as `Any::Map`. `Any::Null` means the mark was
/// removed and should be skipped.
fn attrs_to_marks(attrs: &Attrs) -> Vec<Value> {
    let mut marks = Vec::new();

    for (name, value) in attrs.iter() {
        let mark_type = name.to_string();

        match value {
            // Null means mark removed — skip
            Any::Null => continue,
            // Boolean marks: the key with true means the mark is active
            Any::Bool(true) => {
                marks.push(json!({ "type": mark_type }));
            }
            Any::Bool(false) => continue,
            // Map attributes become mark attrs (e.g., link with href)
            Any::Map(map) => {
                let mut mark_attrs = serde_json::Map::new();
                for (k, v) in map.iter() {
                    mark_attrs.insert(k.clone(), any_to_json(v));
                }
                if mark_attrs.is_empty() {
                    marks.push(json!({ "type": mark_type }));
                } else {
                    marks.push(json!({ "type": mark_type, "attrs": mark_attrs }));
                }
            }
            // String value — treat as a single-attr mark
            Any::String(s) => {
                marks.push(
                    json!({ "type": mark_type, "attrs": { mark_type.clone(): s.to_string() } }),
                );
            }
            _ => {
                // Other values: include the mark with no attrs
                marks.push(json!({ "type": mark_type }));
            }
        }
    }

    marks
}

/// Convert a Yrs Any value to serde_json Value.
fn any_to_json(any: &Any) -> Value {
    match any {
        Any::Null | Any::Undefined => Value::Null,
        Any::Bool(b) => Value::Bool(*b),
        Any::Number(n) => json!(n),
        Any::BigInt(n) => json!(n),
        Any::String(s) => Value::String(s.to_string()),
        Any::Array(arr) => Value::Array(arr.iter().map(any_to_json).collect()),
        Any::Map(map) => {
            let obj: serde_json::Map<String, Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), any_to_json(v)))
                .collect();
            Value::Object(obj)
        }
        Any::Buffer(_) => Value::Null, // binary data not relevant for ProseMirror
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::types::xml::XmlTextPrelim;
    use yrs::types::Attrs;
    use yrs::{Any, Doc, Text, Transact, WriteTxn, XmlFragment};

    #[test]
    fn test_empty_doc_returns_default() {
        let doc = Doc::new();
        let result = extract_prosemirror_json(&doc);
        // No "prosemirror" fragment → error
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_fragment_returns_default() {
        let doc = Doc::new();
        {
            // Create the fragment but don't add anything
            let mut txn = doc.transact_mut();
            txn.get_or_insert_xml_fragment("default");
        }
        let result = extract_prosemirror_json(&doc).unwrap();
        assert_eq!(result["type"], "doc");
        assert_eq!(result["content"][0]["type"], "paragraph");
    }

    #[test]
    fn test_paragraph_with_text() {
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let para = frag.insert(
                &mut txn,
                0,
                yrs::types::xml::XmlElementPrelim::empty("paragraph"),
            );
            let text = para.insert(&mut txn, 0, XmlTextPrelim::new(""));
            text.push(&mut txn, "Hello world");
        }
        let result = extract_prosemirror_json(&doc).unwrap();
        assert_eq!(result["type"], "doc");
        assert_eq!(result["content"][0]["type"], "paragraph");

        // The paragraph's content should contain text
        let content = &result["content"][0]["content"];
        assert!(content.is_array());
        // Find text node(s)
        let text_nodes: Vec<&Value> = content
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["type"] == "text")
            .collect();
        assert!(!text_nodes.is_empty());
        assert_eq!(text_nodes[0]["text"], "Hello world");
    }

    #[test]
    fn test_bold_text() {
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let frag = txn.get_or_insert_xml_fragment("default");
            let para = frag.insert(
                &mut txn,
                0,
                yrs::types::xml::XmlElementPrelim::empty("paragraph"),
            );
            let text = para.insert(&mut txn, 0, XmlTextPrelim::new(""));
            let bold_attrs = Attrs::from([(std::sync::Arc::from("bold"), Any::Bool(true))]);
            text.insert_with_attributes(&mut txn, 0, "bold text", bold_attrs);
        }
        let result = extract_prosemirror_json(&doc).unwrap();
        let content = &result["content"][0]["content"];
        let text_nodes: Vec<&Value> = content.as_array().unwrap().iter().collect();
        assert!(!text_nodes.is_empty());

        let bold_node = &text_nodes[0];
        assert_eq!(bold_node["text"], "bold text");
        assert!(
            bold_node["marks"].is_array(),
            "marks should be array, got: {:?}",
            bold_node
        );
        assert_eq!(bold_node["marks"][0]["type"], "bold");
    }

    #[test]
    fn test_empty_doc_json_helper() {
        let result = empty_doc_json();
        assert_eq!(result["type"], "doc");
        assert_eq!(result["content"][0]["type"], "paragraph");
    }
}
