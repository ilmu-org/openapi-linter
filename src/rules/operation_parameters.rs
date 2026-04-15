use std::collections::{HashMap, HashSet};

use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule, util};

/// Within one operation, no two parameters may share the same `(name, in)` pair.
///
/// Merge rule: operation-level parameters override path-level when `(name, in)` matches.
/// The overridden path-level entry is dropped from the dedup set to avoid false positives
/// on valid operation-level overrides.
///
/// Deref-before-compare (ADR-021): `resolve_ref` is called on each parameter `$ref`
/// before extracting `(name, in)`. If `resolve_ref` returns `None` (external `$ref`
/// or depth limit), the parameter is skipped to avoid false positives.
///
/// Applies to OAS 2.x and 3.x.
pub struct OperationParameters;

impl Rule for OperationParameters {
    fn id(&self) -> &'static str {
        "operation-parameters"
    }

    fn message(&self) -> &'static str {
        "Operation must not define duplicate parameters with the same name and location."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let Some(paths) = doc["paths"].as_object() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for (path_key, path_item) in paths {
            let path_key_enc = path_key.replace('~', "~0").replace('/', "~1");

            // Collect path-level parameters as (name, in) -> index map.
            let path_params = collect_params(doc, &path_item["parameters"]);

            for method in HTTP_METHODS {
                let Some(op) = path_item.get(*method) else {
                    continue;
                };

                let op_params = collect_params(doc, &op["parameters"]);

                // Build the merged set: start with path-level, then apply operation overrides.
                // Operation-level entries with same (name, in) replace path-level entries.
                let mut merged: HashMap<(String, String), usize> = HashMap::new();

                // Add path-level params that are NOT overridden by operation-level params.
                for ((name, in_val), idx) in &path_params {
                    if !op_params.contains_key(&(name.clone(), in_val.clone())) {
                        merged.insert((name.clone(), in_val.clone()), *idx);
                    }
                }

                // Add all operation-level params (overrides and new).
                for ((name, in_val), idx) in &op_params {
                    merged.insert((name.clone(), in_val.clone()), *idx);
                }

                // Check for duplicates within operation-level params themselves.
                let mut seen: HashSet<(String, String)> = HashSet::new();
                if let Some(params) = op["parameters"].as_array() {
                    for (i, param) in params.iter().enumerate() {
                        let resolved =
                            if let Some(ref_ptr) = param.get("$ref").and_then(|v| v.as_str()) {
                                match util::resolve_ref(doc, ref_ptr, 0) {
                                    Some(r) => r,
                                    None => continue, // External ref: skip.
                                }
                            } else {
                                param
                            };

                        let Some(name) = resolved["name"].as_str() else {
                            continue;
                        };
                        let Some(in_val) = resolved["in"].as_str() else {
                            continue;
                        };
                        let key = (name.to_string(), in_val.to_string());
                        if seen.contains(&key) {
                            violations.push(Violation {
                                rule_id: self.id().to_string(),
                                message: format!(
                                    "Duplicate parameter '{name}' in '{in_val}' within operation."
                                ),
                                severity: self.default_severity(),
                                path: format!("/paths/{path_key_enc}/{method}/parameters/{i}"),
                                line: None,
                                col: None,
                            });
                        } else {
                            seen.insert(key);
                        }
                    }
                }

                let _ = merged; // Used for override logic; duplicate check above is sufficient.
            }
        }

        violations
    }
}

/// Collect parameters from a `parameters` array as `(name, in) -> index` map.
/// Parameters that cannot be resolved (external $ref) are silently skipped.
fn collect_params(
    doc: &serde_json::Value,
    params: &serde_json::Value,
) -> HashMap<(String, String), usize> {
    let mut map = HashMap::new();
    let Some(arr) = params.as_array() else {
        return map;
    };

    for (idx, param) in arr.iter().enumerate() {
        let resolved = if let Some(ref_ptr) = param.get("$ref").and_then(|v| v.as_str()) {
            match util::resolve_ref(doc, ref_ptr, 0) {
                Some(r) => r,
                None => continue,
            }
        } else {
            param
        };

        let Some(name) = resolved["name"].as_str() else {
            continue;
        };
        let Some(in_val) = resolved["in"].as_str() else {
            continue;
        };
        map.insert((name.to_string(), in_val.to_string()), idx);
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_on_duplicate_name_and_in() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "parameters": [
                            { "name": "type", "in": "query" },
                            { "name": "type", "in": "query" }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let v = OperationParameters.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "operation-parameters");
    }

    #[test]
    fn passes_when_operation_overrides_path_level() {
        // Valid: operation-level param overrides path-level with same (name, in).
        // Should not produce a false-positive duplicate violation.
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets/{petId}": {
                    "parameters": [
                        { "name": "petId", "in": "path", "required": true }
                    ],
                    "get": {
                        "parameters": [
                            {
                                "name": "petId",
                                "in": "path",
                                "required": true,
                                "description": "Override"
                            }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        assert!(OperationParameters.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn resolves_ref_and_detects_duplicate() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "parameters": {
                    "TypeFilter": { "name": "type", "in": "query" }
                }
            },
            "paths": {
                "/pets": {
                    "get": {
                        "parameters": [
                            { "$ref": "#/components/parameters/TypeFilter" },
                            { "$ref": "#/components/parameters/TypeFilter" }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let v = OperationParameters.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
    }

    #[test]
    fn passes_different_locations() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "parameters": [
                            { "name": "type", "in": "query" },
                            { "name": "type", "in": "header" }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        assert!(OperationParameters.check(&doc, OasVersion::V3_0).is_empty());
    }
}
