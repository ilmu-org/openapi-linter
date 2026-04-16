//! `oas3-valid-schema-example` — validate inline schema examples against their schemas.
//!
//! Walks an OAS 3.x document for any schema object that has an `example` or
//! `examples` field. Resolves `$ref` schemas via the internal resolver. For each
//! located example value, registers the schema with boon and validates the example.
//! One `Violation` is emitted per boon leaf output unit. Truncated at 64 violations
//! per rule invocation (ADR-022).
//!
//! Only `example` (single inline value) is validated. The OAS 3.0 `examples` map
//! contains `Example` objects (not raw values) — their `value` field is validated
//! when present. `externalValue` examples are skipped (no HTTP access).

use boon::Compiler;
use serde_json::Value;

use crate::lint::LintContext;
use crate::model::{OasVersion, Severity, Violation};
use crate::rules::util::resolve_ref;

/// Maximum leaf violations emitted per rule call before truncation.
const MAX_VIOLATIONS: usize = 64;

/// Validate OAS 3.x schema `example` and `examples` values against their schemas.
pub struct Oas3ValidSchemaExample;

impl crate::rules::Rule for Oas3ValidSchemaExample {
    fn id(&self) -> &'static str {
        "oas3-valid-schema-example"
    }

    fn message(&self) -> &'static str {
        "Schema example does not validate against its schema."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        match ctx.version {
            OasVersion::V3_0 | OasVersion::V3_1 => {}
            _ => return vec![],
        }

        let mut violations = Vec::new();
        walk_schemas(ctx.doc, ctx.doc, "", &mut violations, self.id());

        if violations.len() > MAX_VIOLATIONS {
            let extra = violations.len() - MAX_VIOLATIONS;
            violations.truncate(MAX_VIOLATIONS);
            violations.push(Violation::new(
                self.id(),
                format!("... {extra} more example violations omitted"),
                Severity::Error,
                "",
            ));
        }

        violations
    }
}

/// Recursively walk the document looking for schema objects with `example`/`examples`.
///
/// A "schema object" is any JSON object that has a `type`, `properties`,
/// `allOf`, `oneOf`, `anyOf`, `items`, or `$ref` key — or is nested under a
/// `schema` key in a Parameter, RequestBody, MediaType, or Response object.
/// Rather than maintaining a strict allow-list of schema locations (fragile),
/// we walk every object and check for the presence of example fields alongside
/// schema-defining keys.
fn walk_schemas(doc: &Value, node: &Value, path: &str, out: &mut Vec<Violation>, rule_id: &str) {
    match node {
        Value::Object(map) => {
            // Check if this object looks like a schema with an example.
            let has_schema_key = map.contains_key("type")
                || map.contains_key("properties")
                || map.contains_key("allOf")
                || map.contains_key("oneOf")
                || map.contains_key("anyOf")
                || map.contains_key("items")
                || map.contains_key("$ref");

            if has_schema_key {
                // Resolve $ref if needed.
                let schema_node: &Value =
                    if let Some(ref_str) = map.get("$ref").and_then(|v| v.as_str()) {
                        match resolve_ref(doc, ref_str, 0) {
                            Some(resolved) => resolved,
                            None => node, // external or unresolvable — use as-is
                        }
                    } else {
                        node
                    };

                // Validate `example` field.
                if let Some(example) = map.get("example") {
                    validate_example(schema_node, example, path, out, rule_id);
                }

                // Validate `examples` map (OAS 3.x `Example` objects).
                if let Some(Value::Object(examples_map)) = map.get("examples") {
                    for (example_name, example_obj) in examples_map {
                        let example_path = format!("{path}/examples/{example_name}");
                        // `Example` object: use `value` field if present; skip `externalValue`.
                        if let Value::Object(ex_obj) = example_obj
                            && let Some(value) = ex_obj.get("value")
                        {
                            validate_example(schema_node, value, &example_path, out, rule_id);
                        }
                    }
                }
            }

            // Recurse into all child values.
            for (key, value) in map {
                let child_path = format!("{path}/{key}");
                walk_schemas(doc, value, &child_path, out, rule_id);
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let child_path = format!("{path}/{i}");
                walk_schemas(doc, item, &child_path, out, rule_id);
            }
        }
        _ => {}
    }
}

/// Validate a single example value against a schema using boon.
///
/// Silently skips validation if boon compilation fails (malformed schema is a
/// separate concern — the `oas3-schema` rule catches structural schema errors).
fn validate_example(
    schema: &Value,
    example: &Value,
    path: &str,
    out: &mut Vec<Violation>,
    rule_id: &str,
) {
    // Build a fresh local registry for this schema.
    let schema_uri = "https://refract-linter.internal/example-schema";
    let mut compiler = Compiler::new();
    let mut local_schemas = boon::Schemas::new();

    // Strip `example`/`examples` from the schema before registering to avoid
    // boon interpreting them as draft keywords.
    let clean_schema = strip_example_keys(schema);

    if compiler.add_resource(schema_uri, clean_schema).is_err() {
        return;
    }
    let Ok(sch_index) = compiler.compile(schema_uri, &mut local_schemas) else {
        return;
    };

    let err = match local_schemas.validate(example, sch_index) {
        Ok(()) => return,
        Err(e) => e,
    };

    collect_leaves(rule_id, &err, path, out);
}

/// Return a copy of the schema `Value` with `example` and `examples` keys removed.
fn strip_example_keys(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let cleaned: serde_json::Map<String, Value> = map
                .iter()
                .filter(|(k, _)| *k != "example" && *k != "examples")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            Value::Object(cleaned)
        }
        other => other.clone(),
    }
}

/// Recursively collect leaf output units from a boon error tree into violations.
fn collect_leaves(
    rule_id: &str,
    err: &boon::ValidationError<'_, '_>,
    base_path: &str,
    out: &mut Vec<Violation>,
) {
    if err.causes.is_empty() {
        let instance_path = format!("{}", err.instance_location);
        let path = if instance_path.is_empty() || instance_path == "/" {
            base_path.to_owned()
        } else {
            format!("{base_path}{instance_path}")
        };
        let message = format!("{}", err.kind);
        out.push(Violation::new(rule_id, message, Severity::Error, path));
    } else {
        for cause in &err.causes {
            collect_leaves(rule_id, cause, base_path, out);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use boon::Schemas;
    use serde_json::json;

    use super::*;
    use crate::rules::Rule;

    fn make_ctx<'a>(doc: &'a Value, version: OasVersion, schemas: &'a Schemas) -> LintContext<'a> {
        LintContext {
            doc,
            version,
            schemas,
            base_path: None,
        }
    }

    #[test]
    fn skipped_for_oas2() {
        let doc = json!({ "swagger": "2.0" });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        assert!(Oas3ValidSchemaExample.check(&ctx).is_empty());
    }

    #[test]
    fn skipped_for_unknown() {
        let doc = json!({});
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::Unknown, &schemas);
        assert!(Oas3ValidSchemaExample.check(&ctx).is_empty());
    }

    #[test]
    fn valid_example_produces_no_violations() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Pet": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" }
                        },
                        "example": { "name": "Fido" }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidSchemaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "valid example should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn invalid_example_produces_violation() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Pet": {
                        "type": "object",
                        "properties": {
                            "age": { "type": "integer" }
                        },
                        "example": { "age": "not-a-number" }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidSchemaExample.check(&ctx);
        assert!(
            !violations.is_empty(),
            "invalid example should produce at least one violation"
        );
    }

    #[test]
    fn no_example_produces_no_violations() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Pet": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        assert!(Oas3ValidSchemaExample.check(&ctx).is_empty());
    }

    #[test]
    fn examples_map_valid_value_produces_no_violations() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Status": {
                        "type": "string",
                        "enum": ["active", "inactive"],
                        "examples": {
                            "active_example": { "value": "active" }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidSchemaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "valid examples map value should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn examples_map_invalid_value_produces_violation() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Count": {
                        "type": "integer",
                        "examples": {
                            "bad": { "value": "not-an-integer" }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidSchemaExample.check(&ctx);
        assert!(
            !violations.is_empty(),
            "invalid examples map value should produce a violation"
        );
    }
}
