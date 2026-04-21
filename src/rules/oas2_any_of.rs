use crate::lint::LintContext;
use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;
use serde_json::Value;

/// The `anyOf` keyword is not valid in OAS 2.x (Swagger) schemas.
///
/// Walks the entire document recursively, emitting a violation for every
/// object that contains `anyOf` AND at least one other schema key (`type`,
/// `properties`, `allOf`, `oneOf`, `items`, `$ref`). The secondary key gate
/// prevents false positives on example payloads that happen to have a literal
/// `anyOf` field for business reasons.
///
/// Applies to OAS 2.x only.
pub struct Oas2AnyOf;

/// Schema-context keys that confirm an object is used as a JSON Schema.
/// `anyOf` itself is excluded: the triggering key is not part of the gate.
const SCHEMA_KEYS: &[&str] = &["type", "properties", "allOf", "oneOf", "items", "$ref"];

impl Rule for Oas2AnyOf {
    fn id(&self) -> &'static str {
        "oas2-anyOf"
    }

    fn message(&self) -> &'static str {
        "anyOf keyword is not valid in OAS 2.x schemas."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        if ctx.version != OasVersion::V2 {
            return vec![];
        }

        let mut violations = Vec::new();
        walk(ctx.doc, "", &mut violations);
        violations
    }
}

fn walk(value: &Value, path: &str, violations: &mut Vec<Violation>) {
    match value {
        Value::Object(map) => {
            if map.contains_key("anyOf") && has_schema_key(map) {
                violations.push(Violation {
                    rule_id: "oas2-anyOf".to_string(),
                    message: "anyOf keyword is not valid in OAS 2.x schemas.".to_string(),
                    severity: Severity::Error,
                    path: path.to_string(),
                    line: None,
                    col: None,
                });
            }

            for (key, child) in map {
                let key_enc = key.replace('~', "~0").replace('/', "~1");
                let child_path = format!("{path}/{key_enc}");
                walk(child, &child_path, violations);
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let child_path = format!("{path}/{i}");
                walk(item, &child_path, violations);
            }
        }
        _ => {}
    }
}

fn has_schema_key(map: &serde_json::Map<String, Value>) -> bool {
    SCHEMA_KEYS.iter().any(|k| map.contains_key(*k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_on_anyof_in_schema() {
        let doc = json!({
            "swagger": "2.0",
            "definitions": {
                "MyType": {
                    "type": "object",
                    "anyOf": [{ "type": "string" }]
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
        let v = Oas2AnyOf.check(&ctx);
        assert!(!v.is_empty(), "expected violation for anyOf with type key");
        assert!(v.iter().any(|x| x.rule_id == "oas2-anyOf"));
    }

    #[test]
    fn no_false_positive_on_example_payload() {
        // A response example object with a literal "anyOf" key but no schema keys.
        // This should NOT trigger the rule.
        let doc = json!({
            "swagger": "2.0",
            "definitions": {
                "Example": {
                    "anyOf": "some business value"
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
        let v = Oas2AnyOf.check(&ctx);
        assert!(
            v.is_empty(),
            "literal anyOf key without schema context must not trigger: {v:#?}"
        );
    }

    #[test]
    fn skipped_for_oas3() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "MyType": {
                        "type": "object",
                        "anyOf": [{ "type": "string" }]
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
            Oas2AnyOf.check(&ctx).is_empty(),
            "rule must not fire for OAS 3.x"
        );
    }

    #[test]
    fn triggers_in_definitions() {
        let doc = json!({
            "swagger": "2.0",
            "definitions": {
                "X": {
                    "properties": { "name": { "type": "string" } },
                    "anyOf": [{ "type": "object" }]
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
        let v = Oas2AnyOf.check(&ctx);
        assert!(!v.is_empty(), "expected violation for anyOf in definitions");
    }
}
