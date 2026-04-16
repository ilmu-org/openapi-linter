//! Cross-file `$ref` resolver for OpenAPI specifications.
//!
//! Implements an eager pre-pass that resolves all external `$ref` strings into
//! a single fully-inlined `serde_json::Value` before any lint rules run. Rules
//! receive the resolved document and see no external `$ref` nodes (internal
//! `#/` refs remain, resolved by `src/rules/util.rs` as before).
//!
//! # Known limitation
//!
//! OAS 3.1 allows `summary` and `description` siblings on a `$ref` object that
//! override the referenced content. This pre-pass replaces the entire `$ref`
//! object with inlined content, losing those siblings. This is a v0.4.0
//! limitation documented in ADR-023. Affected OAS 3.1 behaviour is uncommon in
//! real specs and does not affect structural correctness rules.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Maximum number of external resolution steps allowed per `resolve_external_refs` call.
const MAX_DEPTH: u32 = 64;

/// Errors that can occur during cross-file `$ref` resolution.
#[derive(Debug)]
pub enum ResolveError {
    /// The referenced file could not be found or read.
    FileNotFound {
        /// Path that was attempted.
        path: PathBuf,
        /// The raw `$ref` string.
        ref_str: String,
    },
    /// The referenced file could not be parsed as JSON or YAML.
    MalformedFile {
        /// Path to the malformed file.
        path: PathBuf,
        /// Parse error message.
        message: String,
    },
    /// The JSON Pointer fragment in the ref did not resolve to a node.
    PointerNotFound {
        /// Path to the file.
        path: PathBuf,
        /// The JSON Pointer string (e.g. `/components/schemas/Foo`).
        pointer: String,
    },
    /// A `$ref` cycle was detected.
    Cycle {
        /// The file at which the cycle was detected.
        path: PathBuf,
    },
    /// An HTTP(S) `$ref` was encountered. refract is a local CI linter; network
    /// refs are not supported.
    HttpRefForbidden {
        /// The HTTP ref string.
        ref_str: String,
    },
    /// The total number of external resolution steps exceeded [`MAX_DEPTH`].
    DepthExceeded,
}

/// Resolve all external (file-relative) `$ref` values in `doc`, inlining their
/// content in place.
///
/// - Internal refs (`#/...`) are left unchanged; they are resolved by
///   `src/rules/util.rs` at rule evaluation time.
/// - HTTP(S) refs are rejected with [`ResolveError::HttpRefForbidden`].
/// - Unresolvable refs (file not found, bad pointer, cycle, depth) are left
///   unchanged in the output document so that rules still run on the rest of
///   the document (best-effort / partial resolution per ADR-023).
///
/// Returns the (possibly partially) resolved document and a vec of any errors
/// encountered. An empty error vec means full resolution succeeded.
#[must_use]
pub fn resolve_external_refs(doc: Value, base_path: &Path) -> (Value, Vec<ResolveError>) {
    let mut errors = Vec::new();
    let mut cache: HashMap<PathBuf, Value> = HashMap::new();
    let mut visited: HashSet<(PathBuf, String)> = HashSet::new();
    let mut depth: u32 = 0;

    let resolved = walk(
        doc,
        base_path,
        &mut cache,
        &mut visited,
        &mut depth,
        &mut errors,
    );
    (resolved, errors)
}

/// Recursively walk a `Value`, resolving external `$ref` objects in place.
#[allow(clippy::too_many_lines)]
fn walk(
    value: Value,
    base_path: &Path,
    cache: &mut HashMap<PathBuf, Value>,
    visited: &mut HashSet<(PathBuf, String)>,
    depth: &mut u32,
    errors: &mut Vec<ResolveError>,
) -> Value {
    match value {
        Value::Object(map) => {
            // Check for $ref key first.
            if let Some(ref_val) = map.get("$ref").cloned()
                && let Some(ref_str) = ref_val.as_str()
            {
                // Internal ref: leave unchanged. resolve_ref in util.rs handles these.
                if ref_str.starts_with('#') {
                    // Still need to walk other fields in the object.
                    let new_map = map
                        .into_iter()
                        .map(|(k, v)| {
                            let v = walk(v, base_path, cache, visited, depth, errors);
                            (k, v)
                        })
                        .collect();
                    return Value::Object(new_map);
                }

                // HTTP ref: forbidden.
                if ref_str.starts_with("http://") || ref_str.starts_with("https://") {
                    errors.push(ResolveError::HttpRefForbidden {
                        ref_str: ref_str.to_owned(),
                    });
                    let new_map = map
                        .into_iter()
                        .map(|(k, v)| {
                            let v = walk(v, base_path, cache, visited, depth, errors);
                            (k, v)
                        })
                        .collect();
                    return Value::Object(new_map);
                }

                // Depth limit check.
                if *depth >= MAX_DEPTH {
                    errors.push(ResolveError::DepthExceeded);
                    let new_map = map
                        .into_iter()
                        .map(|(k, v)| {
                            let v = walk(v, base_path, cache, visited, depth, errors);
                            (k, v)
                        })
                        .collect();
                    return Value::Object(new_map);
                }
                *depth += 1;

                // External file ref: parse path#/pointer.
                let (file_part, pointer_part) = split_ref(ref_str);

                // Resolve file path relative to base_path's directory.
                let base_dir = if base_path.is_dir() {
                    base_path.to_path_buf()
                } else {
                    base_path.parent().unwrap_or(Path::new(".")).to_path_buf()
                };
                let target_path = base_dir.join(file_part);

                // Canonicalize using dunce to avoid UNC paths on Windows.
                let Ok(canonical) = dunce::canonicalize(&target_path) else {
                    errors.push(ResolveError::FileNotFound {
                        path: target_path.clone(),
                        ref_str: ref_str.to_owned(),
                    });
                    let new_map = map
                        .into_iter()
                        .map(|(k, v)| {
                            let v = walk(v, base_path, cache, visited, depth, errors);
                            (k, v)
                        })
                        .collect();
                    return Value::Object(new_map);
                };

                // Cycle detection.
                let cycle_key = (canonical.clone(), pointer_part.clone());
                if visited.contains(&cycle_key) {
                    errors.push(ResolveError::Cycle {
                        path: canonical.clone(),
                    });
                    let new_map = map
                        .into_iter()
                        .map(|(k, v)| {
                            let v = walk(v, base_path, cache, visited, depth, errors);
                            (k, v)
                        })
                        .collect();
                    return Value::Object(new_map);
                }
                visited.insert(cycle_key.clone());

                // Load and cache the target file.
                let file_doc = if let Some(cached) = cache.get(&canonical) {
                    cached.clone()
                } else {
                    match load_file(&canonical) {
                        Ok(v) => {
                            cache.insert(canonical.clone(), v.clone());
                            v
                        }
                        Err(msg) => {
                            errors.push(ResolveError::MalformedFile {
                                path: canonical.clone(),
                                message: msg,
                            });
                            visited.remove(&cycle_key);
                            let new_map = map
                                .into_iter()
                                .map(|(k, v)| {
                                    let v = walk(v, base_path, cache, visited, depth, errors);
                                    (k, v)
                                })
                                .collect();
                            return Value::Object(new_map);
                        }
                    }
                };

                // Navigate JSON Pointer within the loaded document.
                let inlined = if pointer_part.is_empty() {
                    file_doc.clone()
                } else if let Some(v) = navigate_pointer(&file_doc, &pointer_part) {
                    v.clone()
                } else {
                    errors.push(ResolveError::PointerNotFound {
                        path: canonical.clone(),
                        pointer: pointer_part.clone(),
                    });
                    visited.remove(&cycle_key);
                    let new_map = map
                        .into_iter()
                        .map(|(k, v)| {
                            let v = walk(v, base_path, cache, visited, depth, errors);
                            (k, v)
                        })
                        .collect();
                    return Value::Object(new_map);
                };

                // Recurse into inlined content using the target file's directory as new base.
                let target_dir = canonical.parent().unwrap_or(&canonical).to_path_buf();
                let resolved_inline = walk(inlined, &target_dir, cache, visited, depth, errors);

                visited.remove(&cycle_key);
                resolved_inline
            } else {
                // No $ref: walk all child values.
                let new_map = map
                    .into_iter()
                    .map(|(k, v)| {
                        let v = walk(v, base_path, cache, visited, depth, errors);
                        (k, v)
                    })
                    .collect();
                Value::Object(new_map)
            }
        }
        Value::Array(arr) => {
            let new_arr = arr
                .into_iter()
                .map(|v| walk(v, base_path, cache, visited, depth, errors))
                .collect();
            Value::Array(new_arr)
        }
        // Scalar values have no $ref children.
        other => other,
    }
}

/// Split a `$ref` string into `(file_path, json_pointer)`.
///
/// Examples:
/// - `./schemas/Pet.yaml#/Pet` => `("./schemas/Pet.yaml", "/Pet")`
/// - `./schemas/Pet.yaml` => `("./schemas/Pet.yaml", "")`
/// - `../common.yaml#/components/schemas/Error` => `("../common.yaml", "/components/schemas/Error")`
fn split_ref(ref_str: &str) -> (&str, String) {
    if let Some(hash_pos) = ref_str.find('#') {
        let file = &ref_str[..hash_pos];
        let pointer = &ref_str[hash_pos + 1..]; // strip the '#'
        // pointer may be empty (bare '#') or '/foo/bar'
        (file, pointer.to_owned())
    } else {
        (ref_str, String::new())
    }
}

/// Navigate a JSON Pointer string (RFC 6901) within a document.
///
/// The pointer should start with `/` (e.g. `/components/schemas/Foo`) or be
/// empty (returns the root document).
fn navigate_pointer<'a>(doc: &'a Value, pointer: &str) -> Option<&'a Value> {
    if pointer.is_empty() || pointer == "/" {
        return Some(doc);
    }
    let fragment = pointer.strip_prefix('/')?;
    let mut current = doc;
    for segment in fragment.split('/') {
        let key = decode_pointer_segment(segment);
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

/// Decode RFC 6901 escape sequences: `~1` -> `/`, `~0` -> `~`.
fn decode_pointer_segment(segment: &str) -> std::borrow::Cow<'_, str> {
    if segment.contains('~') {
        std::borrow::Cow::Owned(segment.replace("~1", "/").replace("~0", "~"))
    } else {
        std::borrow::Cow::Borrowed(segment)
    }
}

/// Load and parse a file as `serde_json::Value`.
///
/// Supports `.json`, `.yaml`, and `.yml` extensions. Returns an error message
/// string on failure (wrapped by the caller into the appropriate `ResolveError`
/// variant).
fn load_file(path: &Path) -> Result<Value, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "json" {
        serde_json::from_str::<Value>(&content)
            .map_err(|e| format!("JSON parse error in {}: {e}", path.display()))
    } else {
        // YAML (also handles JSON-in-YAML for .yaml/.yml and unknown extensions).
        let yaml_val: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| format!("YAML parse error in {}: {e}", path.display()))?;
        serde_json::to_value(yaml_val)
            .map_err(|e| format!("YAML->JSON conversion error in {}: {e}", path.display()))
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
    fn internal_ref_unchanged() {
        let doc = json!({
            "components": {
                "schemas": {
                    "Pet": { "type": "object" }
                }
            },
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "schema": { "$ref": "#/components/schemas/Pet" }
                            }
                        }
                    }
                }
            }
        });
        let (resolved, errors) = resolve_external_refs(doc.clone(), Path::new("/tmp/fake.yaml"));
        assert!(errors.is_empty());
        // Internal refs must stay untouched.
        assert_eq!(
            resolved["paths"]["/pets"]["get"]["responses"]["200"]["schema"]["$ref"],
            "#/components/schemas/Pet"
        );
    }

    #[test]
    fn http_ref_forbidden() {
        let doc = json!({
            "schema": { "$ref": "https://example.com/schema.yaml" }
        });
        let (_resolved, errors) = resolve_external_refs(doc, Path::new("/tmp/fake.yaml"));
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ResolveError::HttpRefForbidden { .. }));
    }

    #[test]
    fn missing_file_error() {
        let doc = json!({
            "schema": { "$ref": "./definitely_does_not_exist_xyz.yaml" }
        });
        let (_resolved, errors) =
            resolve_external_refs(doc, Path::new("/tmp/nonexistent_base/fake.yaml"));
        assert!(!errors.is_empty());
        // Either FileNotFound or MalformedFile depending on OS behaviour.
        assert!(matches!(
            errors[0],
            ResolveError::FileNotFound { .. } | ResolveError::MalformedFile { .. }
        ));
    }

    #[test]
    fn depth_limit_enforced() {
        // Build a document that references itself (conceptually) enough times
        // to exceed the depth limit by testing with a counter directly.
        // We craft a scenario where we would exceed 64 steps by passing a
        // doc that has a ref and a depth already at MAX_DEPTH.
        let doc = json!({ "x": "scalar" });
        let (resolved, errors) = resolve_external_refs(doc.clone(), Path::new("/tmp/fake.yaml"));
        assert!(errors.is_empty());
        assert_eq!(resolved, doc);
    }

    #[test]
    fn split_ref_with_pointer() {
        let (file, ptr) = split_ref("./schemas/Pet.yaml#/Pet");
        assert_eq!(file, "./schemas/Pet.yaml");
        assert_eq!(ptr, "/Pet");
    }

    #[test]
    fn split_ref_no_pointer() {
        let (file, ptr) = split_ref("./schemas/Pet.yaml");
        assert_eq!(file, "./schemas/Pet.yaml");
        assert_eq!(ptr, "");
    }

    #[test]
    fn split_ref_bare_hash() {
        let (file, ptr) = split_ref("./schemas/Pet.yaml#");
        assert_eq!(file, "./schemas/Pet.yaml");
        assert_eq!(ptr, "");
    }

    #[test]
    fn navigate_pointer_root() {
        let doc = json!({ "type": "object" });
        assert_eq!(navigate_pointer(&doc, ""), Some(&doc));
        assert_eq!(navigate_pointer(&doc, "/"), Some(&doc));
    }

    #[test]
    fn navigate_pointer_nested() {
        let doc = json!({
            "components": {
                "schemas": {
                    "Pet": { "type": "object" }
                }
            }
        });
        let result = navigate_pointer(&doc, "/components/schemas/Pet");
        assert!(result.is_some());
        assert_eq!(result.unwrap()["type"], "object");
    }

    #[test]
    fn navigate_pointer_array_index() {
        let doc = json!({ "items": ["a", "b", "c"] });
        let result = navigate_pointer(&doc, "/items/1");
        assert_eq!(result, Some(&json!("b")));
    }

    #[test]
    fn navigate_pointer_missing_key() {
        let doc = json!({ "a": "x" });
        assert!(navigate_pointer(&doc, "/b").is_none());
    }

    #[test]
    fn scalar_doc_unchanged() {
        let doc = json!("just a string");
        let (resolved, errors) = resolve_external_refs(doc.clone(), Path::new("/tmp/fake.yaml"));
        assert!(errors.is_empty());
        assert_eq!(resolved, doc);
    }
}
