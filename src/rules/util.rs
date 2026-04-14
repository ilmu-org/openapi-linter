//! Shared utilities for rule implementations.

use serde_json::Value;

/// Maximum recursion depth for `$ref` resolution to prevent infinite loops.
const MAX_REF_DEPTH: u8 = 16;

/// Resolve an internal `$ref` pointer within `doc`.
///
/// Only document-local refs (starting with `#/`) are resolved.
/// External refs (HTTP URLs, relative file paths) are silently skipped and
/// `None` is returned to avoid false positives.
///
/// Returns `None` on: external ref, unresolvable pointer, or cycle depth
/// exceeding [`MAX_REF_DEPTH`].
#[must_use]
pub(crate) fn resolve_ref<'a>(doc: &'a Value, pointer: &str, depth: u8) -> Option<&'a Value> {
    if depth >= MAX_REF_DEPTH {
        return None;
    }

    let fragment = pointer.strip_prefix("#/")?;
    let resolved = resolve_pointer(doc, fragment)?;

    // If the resolved value itself has a $ref, follow it (one level of indirection).
    if let Some(next_ref) = resolved.get("$ref").and_then(|v| v.as_str()) {
        resolve_ref(doc, next_ref, depth + 1)
    } else {
        Some(resolved)
    }
}

/// Walk a JSON Pointer fragment (without the leading `#/`) within `doc`.
fn resolve_pointer<'a>(doc: &'a Value, fragment: &str) -> Option<&'a Value> {
    let mut current = doc;
    for segment in fragment.split('/') {
        let key = unescape_pointer_segment(segment);
        match current {
            Value::Object(map) => {
                current = map.get(key.as_ref())?;
            }
            Value::Array(arr) => {
                let idx: usize = key.parse().ok()?;
                current = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Decode RFC 6901 escape sequences: `~1` → `/`, `~0` → `~`.
fn unescape_pointer_segment(segment: &str) -> std::borrow::Cow<'_, str> {
    if segment.contains('~') {
        std::borrow::Cow::Owned(segment.replace("~1", "/").replace("~0", "~"))
    } else {
        std::borrow::Cow::Borrowed(segment)
    }
}

/// Walk every string value in `doc` that belongs to a field named `description`
/// or `summary`, yielding the JSON Pointer path and the string value.
///
/// Used by markdown-scanning rules (`no-eval-in-markdown`, `no-script-tags-in-markdown`).
pub(crate) fn walk_markdown_fields<'a>(
    doc: &'a Value,
    path: &str,
    out: &mut Vec<(String, &'a str)>,
) {
    match doc {
        Value::Object(map) => {
            for (key, value) in map {
                let child_path = format!("{path}/{key}");
                if (key == "description" || key == "summary")
                    && let Some(s) = value.as_str()
                {
                    out.push((child_path.clone(), s));
                }
                walk_markdown_fields(value, &child_path, out);
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let child_path = format!("{path}/{i}");
                walk_markdown_fields(item, &child_path, out);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_inline_object() {
        let doc = json!({
            "components": {
                "schemas": {
                    "Foo": { "type": "object" }
                }
            }
        });
        let resolved = resolve_ref(&doc, "#/components/schemas/Foo", 0);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap()["type"], "object");
    }

    #[test]
    fn resolve_local_ref() {
        let doc = json!({
            "components": {
                "parameters": {
                    "petId": { "name": "petId", "in": "path" }
                }
            },
            "paths": {
                "/pets/{petId}": {
                    "get": {
                        "parameters": [{ "$ref": "#/components/parameters/petId" }]
                    }
                }
            }
        });
        let param_ref = doc["paths"]["/pets/{petId}"]["get"]["parameters"][0]["$ref"]
            .as_str()
            .unwrap();
        let resolved = resolve_ref(&doc, param_ref, 0);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap()["name"], "petId");
    }

    #[test]
    fn cycle_defence() {
        // Chain of 17 hops exceeds MAX_REF_DEPTH=16 — must return None without overflow.
        let doc = json!({
            "a": { "$ref": "#/b" },
            "b": { "$ref": "#/c" },
            "c": { "$ref": "#/d" },
            "d": { "$ref": "#/e" },
            "e": { "$ref": "#/f" },
            "f": { "$ref": "#/g" },
            "g": { "$ref": "#/h" },
            "h": { "$ref": "#/i" },
            "i": { "$ref": "#/j" },
            "j": { "$ref": "#/k" },
            "k": { "$ref": "#/l" },
            "l": { "$ref": "#/m" },
            "m": { "$ref": "#/n" },
            "n": { "$ref": "#/o" },
            "o": { "$ref": "#/p" },
            "p": { "$ref": "#/q" },
            "q": { "type": "string" }
        });
        let result = resolve_ref(&doc, "#/a", 0);
        assert!(result.is_none());
    }

    #[test]
    fn external_ref_skipped() {
        let doc = json!({});
        assert!(resolve_ref(&doc, "https://example.com/schema.yaml", 0).is_none());
        assert!(resolve_ref(&doc, "./other.yaml#/Foo", 0).is_none());
    }

    #[test]
    fn nested_ref_resolved() {
        let doc = json!({
            "components": {
                "parameters": {
                    "base": { "name": "x", "in": "query" },
                    "alias": { "$ref": "#/components/parameters/base" }
                }
            }
        });
        let resolved = resolve_ref(&doc, "#/components/parameters/alias", 0);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap()["name"], "x");
    }
}
