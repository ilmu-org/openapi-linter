use serde_json::Value;

use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{Rule, util};

/// Every schema with `type: array` must declare an `items` property.
///
/// Deref-before-compare (ADR-021): `resolve_ref` is called on any `$ref` schema
/// before inspecting the `type` and `items` fields. If `resolve_ref` returns
/// `None` (external `$ref` or depth limit), the node is treated as opaque and
/// skipped to avoid false positives.
///
/// Applies to OAS 2.x and 3.x.
pub struct ArrayItems;

impl Rule for ArrayItems {
    fn id(&self) -> &'static str {
        "array-items"
    }

    fn message(&self) -> &'static str {
        "Schema with type: array must declare an items property."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let mut violations = Vec::new();
        check_value(doc, doc, "", &mut violations);
        violations
    }
}

/// Recursively walk `value` looking for schema objects with `type: array` and no `items`.
fn check_value(doc: &Value, value: &Value, path: &str, violations: &mut Vec<Violation>) {
    match value {
        Value::Object(obj) => {
            // If this is a $ref object, resolve it; if resolution fails, treat as opaque.
            if let Some(ref_ptr) = obj.get("$ref").and_then(|v| v.as_str()) {
                match util::resolve_ref(doc, ref_ptr, 0) {
                    Some(resolved) => {
                        // Check the resolved value instead.
                        if resolved["type"].as_str() == Some("array") && resolved["items"].is_null()
                        {
                            violations.push(Violation {
                                rule_id: "array-items".to_string(),
                                message: "Schema with type: array must declare an items property."
                                    .to_string(),
                                severity: Severity::Error,
                                path: path.to_string(),
                                line: None,
                                col: None,
                            });
                        }
                    }
                    None => {
                        // External ref or unresolvable: treat as opaque, skip.
                        return;
                    }
                }
                return;
            }

            // Inline schema: check if type is array and items is missing.
            if obj.get("type").and_then(|v| v.as_str()) == Some("array")
                && obj.get("items").is_none()
            {
                violations.push(Violation {
                    rule_id: "array-items".to_string(),
                    message: "Schema with type: array must declare an items property.".to_string(),
                    severity: Severity::Error,
                    path: path.to_string(),
                    line: None,
                    col: None,
                });
            }

            // Recurse into object fields.
            for (key, child) in obj {
                let child_path = if path.is_empty() {
                    format!("/{key}")
                } else {
                    format!("{path}/{key}")
                };
                check_value(doc, child, &child_path, violations);
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let child_path = format!("{path}/{i}");
                check_value(doc, item, &child_path, violations);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_on_array_without_items() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "TagList": { "type": "array" }
                }
            }
        });
        let v = ArrayItems.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "array-items");
    }

    #[test]
    fn passes_with_items_defined() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "TagList": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }
        });
        assert!(ArrayItems.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn resolves_ref_and_checks_resolved() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "BadArray": { "type": "array" },
                    "Alias": { "$ref": "#/components/schemas/BadArray" }
                }
            }
        });
        let v = ArrayItems.check(&doc, OasVersion::V3_0);
        // BadArray directly triggers, Alias resolves to BadArray and triggers again.
        assert!(!v.is_empty());
    }

    #[test]
    fn external_ref_skipped() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "External": { "$ref": "https://example.com/schema.yaml" }
                }
            }
        });
        // External ref cannot be resolved; should not trigger false positive.
        let v = ArrayItems.check(&doc, OasVersion::V3_0);
        assert!(v.is_empty());
    }

    #[test]
    fn no_paths_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(ArrayItems.check(&doc, OasVersion::V3_0).is_empty());
    }
}
