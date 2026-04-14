use std::collections::HashSet;

use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule, util};

/// Every `{param}` token in a path template must have a matching `in: path`
/// parameter definition at the operation or path-item level.
pub struct PathParams;

impl Rule for PathParams {
    fn id(&self) -> &'static str {
        "path-params"
    }

    fn message(&self) -> &'static str {
        "Path template parameter has no matching parameter definition."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let Some(paths) = doc["paths"].as_object() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for (path_key, path_item) in paths {
            let tokens = extract_path_tokens(path_key);
            if tokens.is_empty() {
                continue;
            }

            // Collect path-level parameters (shared across all operations).
            let path_level_params = collect_param_names(doc, &path_item["parameters"]);

            for method in HTTP_METHODS {
                let Some(operation) = path_item.get(*method) else {
                    continue;
                };

                // Operation-level params override/extend path-level params.
                let op_params = collect_param_names(doc, &operation["parameters"]);
                let defined: HashSet<&str> = path_level_params.union(&op_params).copied().collect();

                for token in &tokens {
                    if !defined.contains(token.as_str()) {
                        violations.push(Violation {
                            rule_id: self.id().to_string(),
                            message: format!(
                                "Path parameter '{{{token}}}' is not defined in the parameters list."
                            ),
                            severity: self.default_severity(),
                            path: format!("/paths/{path_key}/{method}"),
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

/// Extract `{token}` names from a path string like `/users/{userId}/items/{itemId}`.
fn extract_path_tokens(path: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = path.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            let token: String = chars.by_ref().take_while(|&c| c != '}').collect();
            if !token.is_empty() {
                tokens.push(token);
            }
        }
    }
    tokens
}

/// Collect the names of all `in: path` parameters from a parameters array.
/// Resolves `$ref` entries using the document root.
fn collect_param_names<'a>(
    doc: &'a serde_json::Value,
    params: &'a serde_json::Value,
) -> HashSet<&'a str> {
    let mut names = HashSet::new();
    let Some(arr) = params.as_array() else {
        return names;
    };

    for param in arr {
        // Resolve $ref if present.
        let resolved = if let Some(ref_ptr) = param.get("$ref").and_then(|v| v.as_str()) {
            util::resolve_ref(doc, ref_ptr, 0).unwrap_or(param)
        } else {
            param
        };

        if resolved["in"].as_str() == Some("path")
            && let Some(name) = resolved["name"].as_str()
        {
            names.insert(name);
        }
    }

    names
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_when_path_param_undefined() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/users/{userId}": {
                    "get": {
                        "operationId": "getUser",
                        "responses": {}
                    }
                }
            }
        });
        let v = PathParams.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty(), "expected violation for undefined {{userId}}");
        assert_eq!(v[0].rule_id, "path-params");
    }

    #[test]
    fn passes_when_param_defined_at_operation_level() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/users/{userId}": {
                    "get": {
                        "parameters": [
                            { "name": "userId", "in": "path", "required": true }
                        ],
                        "responses": {}
                    }
                }
            }
        });
        assert!(PathParams.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn passes_when_param_defined_at_path_level() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/users/{userId}": {
                    "parameters": [
                        { "name": "userId", "in": "path", "required": true }
                    ],
                    "get": {
                        "responses": {}
                    }
                }
            }
        });
        assert!(PathParams.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn resolves_ref_parameter() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "parameters": {
                    "UserId": { "name": "userId", "in": "path", "required": true }
                }
            },
            "paths": {
                "/users/{userId}": {
                    "get": {
                        "parameters": [
                            { "$ref": "#/components/parameters/UserId" }
                        ],
                        "responses": {}
                    }
                }
            }
        });
        assert!(PathParams.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn no_paths_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(PathParams.check(&doc, OasVersion::V3_0).is_empty());
    }
}
