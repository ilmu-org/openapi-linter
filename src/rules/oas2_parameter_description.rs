use crate::lint::LintContext;
use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule, util};

/// Every parameter in an OAS 2.x document must have a non-empty `description` field.
///
/// Deref-before-compare (ADR-021): `resolve_ref` is called on each parameter
/// `$ref` before checking the `description` field. If `resolve_ref` returns
/// `None` (external `$ref` or depth limit), the parameter is treated as opaque
/// and skipped to avoid false positives.
///
/// Applies to OAS 2.x only.
pub struct Oas2ParameterDescription;

impl Rule for Oas2ParameterDescription {
    fn id(&self) -> &'static str {
        "oas2-parameter-description"
    }

    fn message(&self) -> &'static str {
        "Parameter must have non-empty description."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        if ctx.version != OasVersion::V2 {
            return vec![];
        }

        let doc = ctx.doc;
        let mut violations = Vec::new();

        if let Some(paths) = doc["paths"].as_object() {
            for (path_key, path_item) in paths {
                let path_key_enc = path_key.replace('~', "~0").replace('/', "~1");

                // Path-level parameters.
                if let Some(params) = path_item["parameters"].as_array() {
                    for (i, param) in params.iter().enumerate() {
                        let ptr = format!("/paths/{path_key_enc}/parameters/{i}");
                        check_param(doc, param, &ptr, &mut violations);
                    }
                }

                for method in HTTP_METHODS {
                    let Some(op) = path_item.get(*method) else {
                        continue;
                    };

                    if let Some(params) = op["parameters"].as_array() {
                        for (i, param) in params.iter().enumerate() {
                            let ptr = format!("/paths/{path_key_enc}/{method}/parameters/{i}");
                            check_param(doc, param, &ptr, &mut violations);
                        }
                    }
                }
            }
        }

        violations
    }
}

/// Check a single parameter (or `$ref` to a parameter) for a non-empty description.
///
/// Source location is reported at the original parameter node (the `$ref` site),
/// not at the resolved definition target.
fn check_param(
    doc: &serde_json::Value,
    param: &serde_json::Value,
    ptr: &str,
    violations: &mut Vec<Violation>,
) {
    // Deref-before-compare contract (ADR-021).
    let resolved = if let Some(ref_ptr) = param.get("$ref").and_then(|v| v.as_str()) {
        match util::resolve_ref(doc, ref_ptr, 0) {
            Some(r) => r,
            None => return, // External ref: treat as opaque, skip.
        }
    } else {
        param
    };

    let has_description = resolved["description"]
        .as_str()
        .is_some_and(|s| !s.trim().is_empty());

    if !has_description {
        violations.push(Violation {
            rule_id: "oas2-parameter-description".to_string(),
            message: "Parameter must have non-empty description.".to_string(),
            severity: Severity::Warn,
            path: ptr.to_string(),
            line: None,
            col: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_when_description_missing() {
        let doc = json!({
            "swagger": "2.0",
            "paths": {
                "/pets/{petId}": {
                    "get": {
                        "parameters": [
                            { "name": "petId", "in": "path", "required": true }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V2,
            schemas: &schemas,
            base_path: None,
        };
        let v = Oas2ParameterDescription.check(&ctx);
        assert!(!v.is_empty(), "expected violation for missing description");
        assert_eq!(v[0].rule_id, "oas2-parameter-description");
        assert_eq!(v[0].path, "/paths/~1pets~1{petId}/get/parameters/0");
    }

    #[test]
    fn passes_with_description() {
        let doc = json!({
            "swagger": "2.0",
            "paths": {
                "/pets/{petId}": {
                    "get": {
                        "parameters": [
                            {
                                "name": "petId",
                                "in": "path",
                                "required": true,
                                "description": "The ID of the pet"
                            }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V2,
            schemas: &schemas,
            base_path: None,
        };
        assert!(Oas2ParameterDescription.check(&ctx).is_empty());
    }

    #[test]
    fn resolves_ref_and_triggers() {
        let doc = json!({
            "swagger": "2.0",
            "parameters": {
                "PetId": { "name": "petId", "in": "path", "required": true }
            },
            "paths": {
                "/pets/{petId}": {
                    "get": {
                        "parameters": [
                            { "$ref": "#/parameters/PetId" }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        // PetId has no description; resolving the $ref should trigger a violation.
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V2,
            schemas: &schemas,
            base_path: None,
        };
        let v = Oas2ParameterDescription.check(&ctx);
        assert!(
            !v.is_empty(),
            "expected violation after resolving $ref to param without description"
        );
    }

    #[test]
    fn skipped_for_oas3() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "parameters": [{ "name": "type", "in": "query" }],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V3_0,
            schemas: &schemas,
            base_path: None,
        };
        assert!(
            Oas2ParameterDescription.check(&ctx).is_empty(),
            "rule must not fire for OAS 3.x"
        );
    }

    #[test]
    fn empty_string_description_triggers() {
        let doc = json!({
            "swagger": "2.0",
            "paths": {
                "/pets": {
                    "get": {
                        "parameters": [
                            { "name": "type", "in": "query", "description": "" }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V2,
            schemas: &schemas,
            base_path: None,
        };
        let v = Oas2ParameterDescription.check(&ctx);
        assert!(!v.is_empty(), "empty description should trigger violation");
    }
}
