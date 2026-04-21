//! `oas2-valid-schema-example` — validate OAS 2.0 schema examples against their schemas.
//!
//! Walks OAS 2.0 `definitions`, parameter schemas, and response schemas for
//! `example` fields. Validates each example against its enclosing schema using boon.
//! One `Violation` per boon leaf output unit. Truncated at 64 (ADR-022).
//!
//! OAS 2.0 only has a single `example` field (not `examples` map). The field
//! appears directly on schema objects and on response objects (media-type keyed map).
//! Only the schema-level `example` is validated here; response-level `example` maps
//! contain arbitrary media type objects and are out of scope.

use serde_json::Value;

use crate::lint::LintContext;
use crate::model::{OasVersion, Severity, Violation};
use crate::rules::util::{self, resolve_ref};

/// Maximum leaf violations emitted per rule call before truncation.
const MAX_VIOLATIONS: usize = 64;

/// Validate OAS 2.0 schema `example` values against their schemas.
pub struct Oas2ValidSchemaExample;

impl crate::rules::Rule for Oas2ValidSchemaExample {
    fn id(&self) -> &'static str {
        "oas2-valid-schema-example"
    }

    fn message(&self) -> &'static str {
        "Schema example does not validate against its schema."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        if ctx.version != OasVersion::V2 {
            return vec![];
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

/// Recursively walk the document for schema objects with `example` fields.
///
/// Same heuristic as the OAS 3.x rule: detect schema objects by the presence
/// of schema-defining keys. OAS 2.0 schemas also use `type`, `properties`,
/// `allOf`, `items`, `$ref`. The `x-` extension fields are ignored.
fn walk_schemas(doc: &Value, node: &Value, path: &str, out: &mut Vec<Violation>, rule_id: &str) {
    match node {
        Value::Object(map) => {
            let has_schema_key = map.contains_key("type")
                || map.contains_key("properties")
                || map.contains_key("allOf")
                || map.contains_key("items")
                || map.contains_key("$ref");

            if has_schema_key {
                let schema_node: &Value =
                    if let Some(ref_str) = map.get("$ref").and_then(|v| v.as_str()) {
                        match resolve_ref(doc, ref_str, 0) {
                            Some(resolved) => resolved,
                            None => node,
                        }
                    } else {
                        node
                    };

                if let Some(example) = map.get("example") {
                    util::validate_example(schema_node, example, path, out, rule_id);
                }
            }

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
    fn skipped_for_oas3() {
        let doc = json!({ "openapi": "3.0.3" });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        assert!(Oas2ValidSchemaExample.check(&ctx).is_empty());
    }

    #[test]
    fn valid_example_produces_no_violations() {
        let doc = json!({
            "swagger": "2.0",
            "definitions": {
                "Pet": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    },
                    "example": { "name": "Fido" }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        let violations = Oas2ValidSchemaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "valid example should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn invalid_example_produces_violation() {
        let doc = json!({
            "swagger": "2.0",
            "definitions": {
                "Count": {
                    "type": "integer",
                    "example": "not-a-number"
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        let violations = Oas2ValidSchemaExample.check(&ctx);
        assert!(
            !violations.is_empty(),
            "invalid example should produce a violation"
        );
    }

    #[test]
    fn no_example_produces_no_violations() {
        let doc = json!({
            "swagger": "2.0",
            "definitions": {
                "Pet": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        assert!(Oas2ValidSchemaExample.check(&ctx).is_empty());
    }
}
