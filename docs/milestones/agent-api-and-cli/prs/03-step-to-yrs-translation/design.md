# PR 3: ProseMirror Step → Yrs Translation

## Purpose

Replace the full-document-replace strategy in PR 2 with surgical Yrs operations derived from ProseMirror Steps. After this PR, the push endpoint applies changes as fine-grained CRDT operations, enabling true three-way merge — an agent's push merges cleanly with concurrent browser edits when they touch different regions of the document.

## Background

The sidecar's `/diff` endpoint returns ProseMirror Steps — a sequence of transforms that convert `old_doc` into `new_doc`. ProseMirror has a fixed, well-defined set of Step types. Each maps to specific Yrs XML/Text operations.

The Yrs document stores content in an XML-like CRDT structure that mirrors ProseMirror's node tree: `XmlElement` for block nodes (paragraph, heading, etc.), `XmlText` for text content with formatting attributes. The translation operates on this structure directly.

## Step Types and Translation

### ReplaceStep

The most common Step type. Represents inserting, deleting, or replacing a range of content.

```json
{
  "stepType": "replace",
  "from": 10,
  "to": 25,
  "slice": {
    "content": [
      /* ProseMirror node JSON */
    ],
    "openStart": 0,
    "openEnd": 0
  }
}
```

**Translation:**

1. Map ProseMirror positions (`from`, `to`) to Yrs path positions using the document's node tree.
2. If `slice` is empty (deletion): remove the content at the Yrs path range.
3. If `from == to` (insertion): insert new Yrs nodes/text at the position.
4. Otherwise (replacement): delete the range, then insert the slice content.

Position mapping is the core complexity — ProseMirror uses a flat position space (each character, node boundary counts as 1), while Yrs uses hierarchical paths. The translator walks the Yrs XML tree to convert between the two.

### ReplaceAroundStep

Wraps or unwraps content — e.g., toggling a blockquote around paragraphs, or changing a paragraph to a list item.

```json
{
  "stepType": "replaceAround",
  "from": 5,
  "to": 45,
  "gapFrom": 6,
  "gapTo": 44,
  "insert": 1,
  "slice": {
    "content": [
      /* wrapper node */
    ]
  }
}
```

**Translation:**

1. The gap (`gapFrom` to `gapTo`) is the content that stays — only the wrapper changes.
2. For wrapping: create a new `XmlElement` (e.g., `blockquote`), move the gap content into it.
3. For unwrapping: remove the wrapper `XmlElement`, promote its children to the parent level.

### AddMarkStep / RemoveMarkStep

Apply or remove inline formatting (bold, italic, code, link, strike).

```json
{
  "stepType": "addMark",
  "from": 10,
  "to": 20,
  "mark": { "type": "bold" }
}
```

**Translation:**

1. Find the `XmlText` node(s) spanning the range.
2. For AddMark: apply the mark as a Yrs text attribute over the range.
3. For RemoveMark: remove the attribute over the range.

Yrs `XmlText` supports formatting attributes natively — this is a direct mapping.

### AddNodeMarkStep / RemoveNodeMarkStep

Apply or remove marks on node boundaries (less common, used for e.g., highlighted blocks).

**Translation:** Set or remove attributes on the target `XmlElement`.

### AttrStep

Change a node's attributes (e.g., heading level, code block language, image src).

```json
{
  "stepType": "attr",
  "pos": 5,
  "attr": "level",
  "value": 2
}
```

**Translation:**

1. Find the `XmlElement` at the given position.
2. Update the attribute on the Yrs element.

## Position Mapping

The most complex part of the translation. ProseMirror uses a flat position index where:

- Position 0 is the start of the document.
- Each character in a text node counts as 1.
- Each node boundary (open tag, close tag) counts as 1.

Yrs uses hierarchical indexing — position within a parent node's children list.

The translator maintains a position mapper that walks the Yrs XML tree and ProseMirror position space in parallel:

```rust
struct PositionMapper<'a> {
    yrs_doc: &'a Doc,
    xml_fragment: XmlFragmentRef,
}

impl PositionMapper {
    /// Convert a ProseMirror flat position to a Yrs path
    fn resolve(&self, pm_pos: u32) -> YrsPosition { ... }

    /// Convert a ProseMirror range to Yrs range
    fn resolve_range(&self, from: u32, to: u32) -> YrsRange { ... }
}
```

## Module Structure

```rust
// packages/server/src/documents/step_translator.rs

pub struct StepTranslator {
    txn: TransactionMut,
    xml_fragment: XmlFragmentRef,
}

impl StepTranslator {
    pub fn new(doc: &Doc) -> Self { ... }

    /// Apply a sequence of ProseMirror Steps to the Yrs document
    pub fn apply_steps(&mut self, steps: &[serde_json::Value]) -> Result<StepResult> { ... }

    /// Apply a single Step
    fn apply_step(&mut self, step: &serde_json::Value) -> Result<()> {
        match step["stepType"].as_str() {
            Some("replace") => self.apply_replace_step(step),
            Some("replaceAround") => self.apply_replace_around_step(step),
            Some("addMark") => self.apply_add_mark_step(step),
            Some("removeMark") => self.apply_remove_mark_step(step),
            Some("addNodeMark") => self.apply_add_node_mark_step(step),
            Some("removeNodeMark") => self.apply_remove_node_mark_step(step),
            Some("attr") => self.apply_attr_step(step),
            _ => Err(TranslationError::UnknownStepType),
        }
    }
}
```

## Testing Strategy

The Step translator is tested in isolation with a comprehensive suite:

1. **Unit tests per step type:** Create a Yrs document with known content, apply a single Step, verify the resulting Yrs state matches expectations.
2. **Round-trip tests:** Create content via the sidecar (`/parse`), modify it, compute Steps via `/diff`, apply Steps via the translator, extract the result, and verify it matches the expected output.
3. **Concurrent merge tests:** Create a base document, apply Steps from an agent push while also applying direct Yrs updates (simulating browser edits), verify both sets of changes are preserved.

Key test cases:

- Insert a paragraph at the beginning/middle/end
- Delete a paragraph
- Replace text within a paragraph
- Change heading level
- Add/remove bold, italic, code marks
- Wrap paragraphs in a blockquote
- Unwrap a blockquote
- Add/remove list items
- Nested list modifications
- Code block language changes
- Multiple Steps in sequence (position offsets accumulate)

## Integration with Push Endpoint

After this PR, the push flow in `api.rs` changes from:

```rust
// PR 2: full document replace
let new_doc = sidecar.parse(&body.content).await?;
replace_yrs_content(&session.doc, &new_doc);
```

to:

```rust
// PR 3: surgical Step application
let old_doc = load_version_json(base_version).await?;
let new_doc = sidecar.parse(&body.content).await?;
let steps = sidecar.diff(&old_doc, &new_doc).await?;
let mut translator = StepTranslator::new(&session.doc);
let result = translator.apply_steps(&steps)?;
```

## What's Not Included

- Conflict detection and reporting (the CRDT handles conflicts silently — true conflict UI is deferred)
- Step optimization/compaction (apply Steps as-is from the sidecar)
- Undo/redo integration (agent pushes are not undoable from the browser — they appear as external edits)
