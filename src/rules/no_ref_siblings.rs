use serde_json::Value;

use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule};

/// Objects containing `$ref` must not have sibling fields.
///
/// Applies to OAS 2.x and 3.0. Skipped for OAS 3.1 (JSON Schema 2020-12 permits
/// `$ref` siblings in Schema Objects).
///
/// Scans Schema Objects and Response Objects only.
pub struct NoRefSiblings;

impl Rule for NoRefSiblings {
    fn id(&self) -> &'static str {
        "no-$ref-siblings"
    }

    fn message(&self) -> &'static str {
        "Objects with $ref must not have sibling fields."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &serde_json::Value, version: OasVersion) -> Vec<Violation> {
        // OAS 3.1 adopts JSON Schema 2020-12 which permits $ref siblings.
        // Unknown version: skip to avoid false positives on unrecognized spec formats.
        if matches!(version, OasVersion::V3_1 | OasVersion::Unknown) {
            return vec![];
        }

        let mut violations = Vec::new();

        // Scan components/schemas (OAS 3.x) and definitions (OAS 2.x).
        collect_schema_violations(
            doc,
            &doc["components"]["schemas"],
            "/components/schemas",
            &mut violations,
        );
        collect_schema_violations(doc, &doc["definitions"], "/definitions", &mut violations);

        // Scan inline schemas in path operations.
        if let Some(paths) = doc["paths"].as_object() {
            for (path_key, path_item) in paths {
                let path_key_enc = path_key.replace('~', "~0").replace('/', "~1");

                // Path-level parameters.
                if let Some(params) = path_item["parameters"].as_array() {
                    for (pi, param) in params.iter().enumerate() {
                        let ptr = format!("/paths/{path_key_enc}/parameters/{pi}/schema");
                        check_schema_node(&param["schema"], &ptr, &mut violations);
                    }
                }

                for method in HTTP_METHODS {
                    let Some(op) = path_item.get(*method) else {
                        continue;
                    };

                    // Operation parameters.
                    if let Some(params) = op["parameters"].as_array() {
                        for (pi, param) in params.iter().enumerate() {
                            let ptr =
                                format!("/paths/{path_key_enc}/{method}/parameters/{pi}/schema");
                            check_schema_node(&param["schema"], &ptr, &mut violations);
                        }
                    }

                    // Request body schemas (OAS 3.x).
                    if let Some(content) = op["requestBody"]["content"].as_object() {
                        for (media_type, media_obj) in content {
                            let ptr = format!(
                                "/paths/{path_key_enc}/{method}/requestBody/content/{media_type}/schema"
                            );
                            check_schema_node(&media_obj["schema"], &ptr, &mut violations);
                        }
                    }

                    // Response objects and their schemas.
                    if let Some(responses) = op["responses"].as_object() {
                        for (status, response) in responses {
                            let resp_ptr =
                                format!("/paths/{path_key_enc}/{method}/responses/{status}");
                            check_ref_siblings(response, &resp_ptr, &mut violations);

                            // Response body schemas (OAS 3.x).
                            if let Some(content) = response["content"].as_object() {
                                for (media_type, media_obj) in content {
                                    let ptr = format!("{resp_ptr}/content/{media_type}/schema");
                                    check_schema_node(&media_obj["schema"], &ptr, &mut violations);
                                }
                            }

                            // OAS 2.x response schema.
                            let ptr = format!("{resp_ptr}/schema");
                            check_schema_node(&response["schema"], &ptr, &mut violations);
                        }
                    }
                }
            }
        }

        // components/responses (OAS 3.x).
        if let Some(responses) = doc["components"]["responses"].as_object() {
            for (name, response) in responses {
                let resp_ptr = format!("/components/responses/{name}");
                check_ref_siblings(response, &resp_ptr, &mut violations);
            }
        }

        violations
    }
}

/// Check all entries in a schemas map for $ref + siblings.
fn collect_schema_violations(
    _doc: &Value,
    schemas_node: &Value,
    base_ptr: &str,
    violations: &mut Vec<crate::model::Violation>,
) {
    let Some(map) = schemas_node.as_object() else {
        return;
    };
    for (name, schema) in map {
        let ptr = format!("{base_ptr}/{name}");
        check_schema_node(schema, &ptr, violations);
    }
}

/// Check a single schema node for $ref siblings. Only acts on object values.
fn check_schema_node(value: &Value, ptr: &str, violations: &mut Vec<crate::model::Violation>) {
    if value.is_null() {
        return;
    }
    check_ref_siblings(value, ptr, violations);
}

/// If `value` is an object with a `$ref` key and other keys, add a violation.
fn check_ref_siblings(value: &Value, ptr: &str, violations: &mut Vec<crate::model::Violation>) {
    let Some(obj) = value.as_object() else {
        return;
    };
    if obj.contains_key("$ref") && obj.len() > 1 {
        violations.push(crate::model::Violation {
            rule_id: "no-$ref-siblings".to_string(),
            message: "Objects with $ref must not have sibling fields.".to_string(),
            severity: Severity::Error,
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
    fn triggers_on_ref_with_sibling_in_schema() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Pet": {
                        "$ref": "#/components/schemas/Animal",
                        "description": "A pet"
                    }
                }
            }
        });
        let v = NoRefSiblings.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "no-$ref-siblings");
    }

    #[test]
    fn passes_with_ref_only() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Pet": { "$ref": "#/components/schemas/Animal" }
                }
            }
        });
        assert!(NoRefSiblings.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn skipped_for_oas31() {
        let doc = json!({
            "openapi": "3.1.0",
            "components": {
                "schemas": {
                    "Pet": {
                        "$ref": "#/components/schemas/Animal",
                        "description": "A pet"
                    }
                }
            }
        });
        assert!(NoRefSiblings.check(&doc, OasVersion::V3_1).is_empty());
    }

    #[test]
    fn triggers_on_response_ref_with_sibling() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "$ref": "#/components/responses/PetList",
                                "description": "extra"
                            }
                        }
                    }
                }
            }
        });
        let v = NoRefSiblings.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
    }
}
