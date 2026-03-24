# PR 3: ProseMirror Step → Yrs Translation — Implementation Plan

## Prerequisites

- [ ] PR 2 (Content Push Endpoint) is merged

## Steps

### 1. Create the step_translator module

- [ ] Create `packages/server/src/documents/step_translator.rs` with the module structure:

```rust
use serde_json::Value;
use yrs::types::xml::{XmlFragmentRef, XmlElementRef, XmlTextRef};
use yrs::{Doc, TransactionMut};

pub struct StepTranslator<'a> {
    txn: &'a mut TransactionMut<'a>,
    fragment: XmlFragmentRef,
}

#[derive(Debug)]
pub struct StepResult {
    pub steps_applied: usize,
    pub steps_failed: usize,
}

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
```

- [ ] Add `pub mod step_translator;` to `packages/server/src/documents/mod.rs`.

### 2. Implement position mapping

- [ ] Add `PositionMapper` to the step_translator module:

```rust
struct PositionMapper {
    // Walks the Yrs XML tree to convert ProseMirror flat positions
    // to Yrs hierarchical positions
}

struct YrsPosition {
    parent: XmlElementRef, // or XmlFragmentRef
    index: u32,            // child index within parent
    text_offset: u32,      // offset within a text node (0 for element boundaries)
}

struct YrsRange {
    start: YrsPosition,
    end: YrsPosition,
}

impl PositionMapper {
    fn new(fragment: &XmlFragmentRef, txn: &TransactionMut) -> Self { ... }

    /// Walk the Yrs tree and map a ProseMirror flat position.
    /// ProseMirror position counting:
    /// - Each node opening tag: +1
    /// - Each character in text: +1
    /// - Each node closing tag: +1
    fn resolve(&self, pm_pos: u32, txn: &TransactionMut) -> Result<YrsPosition, TranslationError> { ... }

    fn resolve_range(&self, from: u32, to: u32, txn: &TransactionMut) -> Result<YrsRange, TranslationError> { ... }
}
```

- [ ] Test position mapping with a simple document:
  - `<doc><paragraph>Hello</paragraph></doc>` → position 0 is before doc, 1 is start of paragraph, 2 is 'H', 6 is 'o', 7 is end of paragraph, 8 is end of doc.

### 3. Implement ReplaceStep translation

- [ ] Implement `apply_replace_step`:

```rust
fn apply_replace_step(&mut self, step: &Value) -> Result<(), TranslationError> {
    let from = step["from"].as_u64().ok_or(TranslationError::MissingField("from"))? as u32;
    let to = step["to"].as_u64().ok_or(TranslationError::MissingField("to"))? as u32;
    let slice = step.get("slice");

    let range = self.mapper.resolve_range(from, to, self.txn)?;

    // Delete existing content in range (if from != to)
    if from != to {
        self.delete_range(&range)?;
    }

    // Insert slice content (if slice exists and has content)
    if let Some(slice) = slice {
        if let Some(content) = slice.get("content") {
            let insert_pos = self.mapper.resolve(from, self.txn)?;
            self.insert_content(&insert_pos, content)?;
        }
    }

    Ok(())
}
```

- [ ] Implement helper methods:
  - `delete_range(range: &YrsRange)` — removes content between two Yrs positions.
  - `insert_content(pos: &YrsPosition, nodes: &Value)` — inserts ProseMirror node JSON as Yrs XML elements.
  - `insert_text(text_ref: &XmlTextRef, offset: u32, text: &str, marks: &[Value])` — inserts text with formatting.

### 4. Implement AddMarkStep / RemoveMarkStep translation

- [ ] Implement `apply_add_mark_step` and `apply_remove_mark_step`:

```rust
fn apply_add_mark_step(&mut self, step: &Value) -> Result<(), TranslationError> {
    let from = step["from"].as_u64().ok_or(...)? as u32;
    let to = step["to"].as_u64().ok_or(...)? as u32;
    let mark = step.get("mark").ok_or(...)?;
    let mark_type = mark["type"].as_str().ok_or(...)?;

    // Find the XmlText node(s) spanning this range
    // Apply formatting attribute
    let range = self.mapper.resolve_range(from, to, self.txn)?;
    self.apply_mark_to_range(&range, mark_type, mark.get("attrs"))?;

    Ok(())
}
```

- [ ] Mark types map to Yrs formatting attributes:
  - `bold` → `{ "bold": true }`
  - `italic` → `{ "italic": true }`
  - `code` → `{ "code": true }`
  - `strike` → `{ "strike": true }`
  - `link` → `{ "link": { "href": "...", "title": "..." } }`

### 5. Implement ReplaceAroundStep translation

- [ ] Implement `apply_replace_around_step`:

```rust
fn apply_replace_around_step(&mut self, step: &Value) -> Result<(), TranslationError> {
    let from = step["from"].as_u64().ok_or(...)? as u32;
    let to = step["to"].as_u64().ok_or(...)? as u32;
    let gap_from = step["gapFrom"].as_u64().ok_or(...)? as u32;
    let gap_to = step["gapTo"].as_u64().ok_or(...)? as u32;
    let slice = step.get("slice");

    // The content between gapFrom and gapTo is preserved
    // The wrapper (from..gapFrom and gapTo..to) is replaced with slice

    // 1. Extract the gap content (children to preserve)
    // 2. Remove the old wrapper
    // 3. Insert the new wrapper from slice
    // 4. Move the preserved content into the new wrapper

    Ok(())
}
```

### 6. Implement AttrStep translation

- [ ] Implement `apply_attr_step`:

```rust
fn apply_attr_step(&mut self, step: &Value) -> Result<(), TranslationError> {
    let pos = step["pos"].as_u64().ok_or(...)? as u32;
    let attr = step["attr"].as_str().ok_or(...)?;
    let value = &step["value"];

    let yrs_pos = self.mapper.resolve(pos, self.txn)?;
    // Find the XmlElement at this position and update its attribute
    // e.g., heading level change: set "level" attribute
    self.set_element_attr(&yrs_pos, attr, value)?;

    Ok(())
}
```

### 7. Implement the top-level apply_steps method

- [ ] Wire all step handlers together:

```rust
impl<'a> StepTranslator<'a> {
    pub fn apply_steps(&mut self, steps: &[Value]) -> Result<StepResult, TranslationError> {
        let mut applied = 0;
        let mut failed = 0;

        for step in steps {
            match self.apply_step(step) {
                Ok(()) => applied += 1,
                Err(e) => {
                    tracing::warn!("Step translation failed: {e}");
                    failed += 1;
                    // Continue applying remaining steps — best effort
                }
            }
        }

        Ok(StepResult {
            steps_applied: applied,
            steps_failed: failed,
        })
    }

    fn apply_step(&mut self, step: &Value) -> Result<(), TranslationError> {
        match step["stepType"].as_str() {
            Some("replace") => self.apply_replace_step(step),
            Some("replaceAround") => self.apply_replace_around_step(step),
            Some("addMark") => self.apply_add_mark_step(step),
            Some("removeMark") => self.apply_remove_mark_step(step),
            Some("addNodeMark") => self.apply_add_node_mark_step(step),
            Some("removeNodeMark") => self.apply_remove_node_mark_step(step),
            Some("attr") => self.apply_attr_step(step),
            Some(other) => Err(TranslationError::UnknownStepType(other.to_string())),
            None => Err(TranslationError::MissingField("stepType".to_string())),
        }
    }
}
```

### 8. Integrate with push_content handler

- [ ] In `packages/server/src/documents/api.rs`, replace the full-document-replace call with Step translation:

```rust
// Replace:
//   yrs_json::replace_yrs_content(&session.doc, &new_doc.doc)?;
// With:
let mut translator = StepTranslator::new(&session.doc);
let result = translator.apply_steps(&steps.steps)?;
```

- [ ] Update the change summary to include `steps_failed` if any Steps failed translation.

### 9. Write unit tests for position mapping

- [ ] Create test file or add tests to `step_translator.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_paragraph_positions() {
        // Create a Yrs doc with: <doc><p>Hello</p></doc>
        // Verify PM positions map correctly:
        // 0 = before doc, 1 = start of p, 2 = 'H', 6 = 'o', 7 = end of p
    }

    #[test]
    fn test_multi_paragraph_positions() { ... }

    #[test]
    fn test_nested_list_positions() { ... }
}
```

### 10. Write unit tests for each step type

- [ ] Test ReplaceStep:
  - Insert text at beginning of paragraph
  - Insert text in middle of paragraph
  - Delete text range
  - Replace paragraph content
  - Insert a new paragraph between existing ones
  - Delete an entire paragraph

- [ ] Test AddMarkStep/RemoveMarkStep:
  - Add bold to a text range
  - Remove italic from a text range
  - Add a link mark with attributes

- [ ] Test AttrStep:
  - Change heading level (1 → 2)
  - Change code block language

- [ ] Test ReplaceAroundStep:
  - Wrap paragraphs in blockquote
  - Unwrap blockquote

### 11. Write integration tests

- [ ] Create round-trip integration tests that:
  1. Start with a known markdown document.
  2. Parse it via the sidecar → `old_doc`.
  3. Create a modified version → `new_doc`.
  4. Diff via sidecar → Steps.
  5. Apply Steps to a Yrs document containing `old_doc`.
  6. Extract the result and verify it matches `new_doc`.

Test cases:

- [ ] Add a paragraph to the end
- [ ] Modify text in the middle
- [ ] Delete a section
- [ ] Change formatting (add bold, remove italic)
- [ ] Change heading levels
- [ ] Restructure lists (add items, change nesting)
- [ ] Mixed changes (add + delete + reformat in one push)

### 12. Build and format check

- [ ] Run `cargo build` in `packages/server/` — compiles without errors.
- [ ] Run `cargo test` in `packages/server/` — all tests pass.
- [ ] Run `pnpm run format:check` — no formatting issues.

## Verification

- [ ] All Step types translate correctly in isolation (unit tests pass)
- [ ] Position mapping handles nested structures (lists, blockquotes)
- [ ] Round-trip tests pass (parse → diff → translate → extract = expected)
- [ ] Push endpoint uses Step translation instead of full replace
- [ ] Concurrent edits merge: push changes to paragraph 3 while browser edited paragraph 1 → both changes preserved
- [ ] Multiple Steps in sequence apply correctly (positions shift after each step)
- [ ] Large documents (50+ paragraphs) translate without errors
- [ ] Failed Step translation doesn't crash — logs warning and continues

## Files Modified

| File                                               | Change                             |
| -------------------------------------------------- | ---------------------------------- |
| `packages/server/src/documents/step_translator.rs` | New: Step → Yrs translation layer  |
| `packages/server/src/documents/mod.rs`             | Add `pub mod step_translator`      |
| `packages/server/src/documents/api.rs`             | Use StepTranslator in push_content |
