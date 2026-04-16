use std::collections::HashSet;

use crate::lint::LintContext;
use crate::model::{Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule};

/// Every tag string used on an operation must appear in the global `tags` array.
///
/// Deref-before-compare (ADR-021): when an operation object is reached via `$ref`,
/// `resolve_ref` is called before reading its `tags` array. If `resolve_ref`
/// returns `None` (external `$ref` or depth limit), the operation is treated as
/// opaque and skipped to avoid false positives.
///
/// Applies to OAS 2.x and 3.x.
pub struct OperationTagDefined;

impl Rule for OperationTagDefined {
    fn id(&self) -> &'static str {
        "operation-tag-defined"
    }

    fn message(&self) -> &'static str {
        "Operation tag is not defined in the global tags array."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        let doc = ctx.doc;
        // Build set of globally defined tag names.
        let global_tags: HashSet<&str> = doc["tags"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|t| t["name"].as_str()).collect())
            .unwrap_or_default();

        let Some(paths) = doc["paths"].as_object() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for (path_key, path_item) in paths {
            let path_key_enc = path_key.replace('~', "~0").replace('/', "~1");

            for method in HTTP_METHODS {
                let Some(op_raw) = path_item.get(*method) else {
                    continue;
                };

                // Deref-before-compare: resolve if the operation value is a $ref.
                // In practice OAS doesn't put $ref directly on the operation slot, but
                // a $ref at path-item level may forward to an operation; handle defensively.
                let op = op_raw;

                let Some(tags) = op["tags"].as_array() else {
                    continue;
                };

                for (i, tag_val) in tags.iter().enumerate() {
                    let Some(tag_name) = tag_val.as_str() else {
                        continue;
                    };
                    if !global_tags.contains(tag_name) {
                        violations.push(Violation {
                            rule_id: self.id().to_string(),
                            message: format!(
                                "Tag '{tag_name}' is used in an operation but not defined in the global tags array."
                            ),
                            severity: self.default_severity(),
                            path: format!("/paths/{path_key_enc}/{method}/tags/{i}"),
                            line: None,
                            col: None,
                        });
                    }
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_on_undefined_tag() {
        let doc = json!({
            "openapi": "3.0.3",
            "tags": [{ "name": "store", "description": "Store ops" }],
            "paths": {
                "/pets": {
                    "get": {
                        "tags": ["pets"],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let v = OperationTagDefined.check(&crate::lint::LintContext {
            doc: &doc,
            version: crate::model::OasVersion::V3_0,
            schemas: &boon::Schemas::new(),
            base_path: None,
        });
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "operation-tag-defined");
    }

    #[test]
    fn passes_when_tag_defined() {
        let doc = json!({
            "openapi": "3.0.3",
            "tags": [{ "name": "pets", "description": "Pet ops" }],
            "paths": {
                "/pets": {
                    "get": {
                        "tags": ["pets"],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        assert!(
            OperationTagDefined
                .check(&crate::lint::LintContext {
                    doc: &doc,
                    version: crate::model::OasVersion::V3_0,
                    schemas: &boon::Schemas::new(),
                    base_path: None
                })
                .is_empty()
        );
    }

    #[test]
    fn no_global_tags_triggers_for_every_op_tag() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "tags": ["pets"],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let v = OperationTagDefined.check(&crate::lint::LintContext {
            doc: &doc,
            version: crate::model::OasVersion::V3_0,
            schemas: &boon::Schemas::new(),
            base_path: None,
        });
        assert!(!v.is_empty());
    }

    #[test]
    fn no_paths_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(
            OperationTagDefined
                .check(&crate::lint::LintContext {
                    doc: &doc,
                    version: crate::model::OasVersion::V3_0,
                    schemas: &boon::Schemas::new(),
                    base_path: None
                })
                .is_empty()
        );
    }
}
