//! Two-pass YAML position indexing for source-accurate violation reporting.
//!
//! Pass 1 is handled by `parser::parse()` using `serde_yaml`.
//! Pass 2 (this module) uses `yaml-rust2` to build a [`crate::position::PositionIndex`] that
//! maps every JSON-Pointer-style path to its source [`crate::position::Span`].
//!
//! **Design note:** paths stored in the index use the same raw-key concatenation
//! as the rule implementations (e.g. `/paths//foo/get`, not the RFC 6901-encoded
//! `/paths/~1foo/get`). This is intentional so that `lib::lint()` can look up
//! `v.path` directly without any re-encoding.

use std::collections::HashMap;

use yaml_rust2::parser::{Event, Parser};

/// Source location in a YAML document (both values are 1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub col: u32,
}

/// Maps violation paths to their source positions.
///
/// Keyed by the same path strings that rules store in [`crate::model::Violation::path`].
pub type PositionIndex = HashMap<String, Span>;

/// Returns an empty [`PositionIndex`] for JSON files or when yaml-rust2 errors.
///
/// Callers can safely call `get()` on the result; a miss means no position info.
#[must_use]
pub fn empty() -> PositionIndex {
    HashMap::new()
}

/// Build a [`PositionIndex`] from YAML source text.
///
/// Returns [`empty()`] on any parse error — positions are best-effort and
/// must never block linting.
#[must_use]
pub fn build_yaml(content: &str) -> PositionIndex {
    build_inner(content).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Internal implementation
// ---------------------------------------------------------------------------

enum Frame {
    /// Inside a mapping.  `pending_key` is `None` while waiting for the next
    /// key scalar, and `Some(key)` once a key has been read and we await its value.
    Mapping { pending_key: Option<String> },
    /// Inside a sequence.  `index` is the next element index.
    Sequence { index: usize },
}

/// Compute the path for whatever is currently awaiting a value.
///
/// Walks the stack and appends each confirmed path segment:
/// - For a `Mapping` frame: appends `/<pending_key>` when a key is pending.
/// - For a `Sequence` frame: appends `/<index>`.
fn path_for_pending_value(stack: &[Frame]) -> String {
    let mut buf = String::new();
    for frame in stack {
        match frame {
            Frame::Mapping {
                pending_key: Some(key),
            } => {
                buf.push('/');
                buf.push_str(key);
            }
            Frame::Sequence { index } => {
                buf.push('/');
                buf.push_str(&index.to_string());
            }
            Frame::Mapping { pending_key: None } => {
                // Waiting for a key — no segment to add yet.
            }
        }
    }
    buf
}

/// After a value (scalar, mapping, or sequence) has been consumed, advance
/// the parent frame:
/// - Mapping: clear `pending_key` (back to awaiting next key).
/// - Sequence: increment element index.
fn advance_after_value(stack: &mut [Frame]) {
    match stack.last_mut() {
        Some(Frame::Mapping { pending_key }) => *pending_key = None,
        Some(Frame::Sequence { index }) => *index += 1,
        None => {}
    }
}

fn build_inner(content: &str) -> Option<PositionIndex> {
    let mut index = PositionIndex::new();
    let mut parser = Parser::new_from_str(content);
    let mut stack: Vec<Frame> = Vec::new();

    loop {
        let (event, marker) = parser.next_token().ok()?;

        // yaml-rust2 uses 1-based lines and 0-based columns; we want 1-based.
        // Line counts will never realistically reach u32::MAX in a YAML file.
        let line = u32::try_from(marker.line()).unwrap_or(u32::MAX);
        let col = u32::try_from(marker.col() + 1).unwrap_or(u32::MAX);

        match event {
            Event::MappingStart(..) => {
                // Record position for the current pending-value path (if any).
                let path = path_for_pending_value(&stack);
                if !path.is_empty() {
                    index.insert(path, Span { line, col });
                }
                // Push a new mapping frame; the parent key stays in its frame
                // and will be cleared when the matching MappingEnd fires.
                stack.push(Frame::Mapping { pending_key: None });
            }
            Event::SequenceStart(..) => {
                let path = path_for_pending_value(&stack);
                if !path.is_empty() {
                    index.insert(path, Span { line, col });
                }
                stack.push(Frame::Sequence { index: 0 });
            }
            Event::MappingEnd | Event::SequenceEnd => {
                stack.pop();
                advance_after_value(&mut stack);
            }
            Event::Scalar(value, ..) => {
                let n = stack.len();
                if n == 0 {
                    // Top-level scalar (bare document) — no path.
                } else {
                    // Inspect the top frame without mutating first.
                    let (is_seq, is_key_pos) = match &stack[n - 1] {
                        Frame::Mapping { pending_key: None } => (false, true),
                        Frame::Mapping { .. } => (false, false),
                        Frame::Sequence { .. } => (true, false),
                    };

                    if is_key_pos {
                        // Key position — store as pending key.
                        if let Frame::Mapping { pending_key } = &mut stack[n - 1] {
                            *pending_key = Some(value);
                        }
                    } else if !is_seq {
                        // Mapping value position — record and clear.
                        let key = if let Frame::Mapping { pending_key } = &mut stack[n - 1] {
                            pending_key.take().expect("pending_key is Some")
                        } else {
                            unreachable!()
                        };
                        let mut path = path_for_pending_value(&stack[..n - 1]);
                        path.push('/');
                        path.push_str(&key);
                        index.insert(path, Span { line, col });
                    } else {
                        // Sequence element position — record and increment.
                        let i = if let Frame::Sequence { index: i } = &stack[n - 1] {
                            *i
                        } else {
                            unreachable!()
                        };
                        let mut path = path_for_pending_value(&stack[..n - 1]);
                        path.push('/');
                        path.push_str(&i.to_string());
                        index.insert(path, Span { line, col });
                        if let Frame::Sequence { index: idx } = &mut stack[n - 1] {
                            *idx += 1;
                        }
                    }
                }
            }
            Event::Alias(_) => {
                // Aliases resolve to already-indexed positions; skip.
                advance_after_value(&mut stack);
            }
            Event::StreamEnd => break,
            _ => {}
        }
    }

    Some(index)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(yaml: &str) -> PositionIndex {
        build_yaml(yaml)
    }

    #[test]
    fn simple_scalar_value() {
        let index = idx("foo: bar\n");
        assert!(index.contains_key("/foo"), "index: {index:?}");
    }

    #[test]
    fn nested_path() {
        let index = idx("info:\n  title: Test\n");
        assert!(index.contains_key("/info/title"), "index: {index:?}");
    }

    #[test]
    fn sequence_elements() {
        let index = idx("tags:\n  - name: foo\n  - name: bar\n");
        assert!(index.contains_key("/tags"), "index: {index:?}");
        // /tags/0 is a mapping start (object), /tags/0/name is the scalar
        assert!(index.contains_key("/tags/0/name"), "index: {index:?}");
        assert!(index.contains_key("/tags/1/name"), "index: {index:?}");
    }

    #[test]
    fn empty_document_returns_empty_index() {
        let index = idx("");
        assert!(index.is_empty());
    }

    #[test]
    fn path_with_slash_in_key() {
        // OAS path keys like /foo are stored raw (no RFC 6901 encoding)
        // to match what rule implementations produce.
        let index = idx("paths:\n  /foo:\n    get:\n      operationId: getFoo\n");
        assert!(
            index.contains_key("/paths//foo/get/operationId"),
            "index: {index:?}"
        );
    }

    #[test]
    fn invalid_yaml_returns_empty() {
        let index = idx("key: :\n  bad yaml here: :");
        // Should not panic; may or may not be empty.
        let _ = index;
    }
}
