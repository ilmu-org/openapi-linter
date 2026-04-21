//! `oas3-valid-media-example` — validate MediaType, Parameter, and Header examples.
//!
//! Walks OAS 3.x documents for `example`/`examples` fields on MediaType objects
//! (in `content` maps), Parameter objects, and Header objects. Each found example
//! is validated against the sibling `schema` field using boon.
//!
//! Distinct scope from `oas3-valid-schema-example`, which validates inline schema
//! `example`/`examples` fields. This rule handles examples at the MediaType,
//! Parameter, and Header level where `schema` is a sibling field rather than a
//! parent context.
//!
//! Only document-local `$ref`s are resolved (ADR-021). External refs are silently
//! skipped to avoid false positives. Truncated at 64 violations (ADR-022).

use serde_json::Value;

use crate::lint::LintContext;
use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, util};

/// Maximum leaf violations emitted per rule call before truncation.
const MAX_VIOLATIONS: usize = 64;

/// Validate OAS 3.x MediaType, Parameter, and Header `example`/`examples` values.
pub struct Oas3ValidMediaExample;

impl crate::rules::Rule for Oas3ValidMediaExample {
    fn id(&self) -> &'static str {
        "oas3-valid-media-example"
    }

    fn message(&self) -> &'static str {
        "Example does not validate against its schema."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        match ctx.version {
            OasVersion::V3_0 | OasVersion::V3_1 => {}
            _ => return vec![],
        }

        let doc = ctx.doc;
        let mut violations = Vec::new();

        if let Some(paths) = doc["paths"].as_object() {
            for (path_key, path_item) in paths {
                let path_enc = path_key.replace('~', "~0").replace('/', "~1");

                // Path-level parameters.
                if let Some(params) = path_item["parameters"].as_array() {
                    for (i, param) in params.iter().enumerate() {
                        let ptr = format!("/paths/{path_enc}/parameters/{i}");
                        check_node(doc, param, &ptr, &mut violations, self.id());
                    }
                }

                for method in HTTP_METHODS {
                    let Some(op) = path_item.get(*method) else {
                        continue;
                    };

                    let op_base = format!("/paths/{path_enc}/{method}");

                    // Operation-level parameters.
                    if let Some(params) = op["parameters"].as_array() {
                        for (i, param) in params.iter().enumerate() {
                            let ptr = format!("{op_base}/parameters/{i}");
                            check_node(doc, param, &ptr, &mut violations, self.id());
                        }
                    }

                    // requestBody content MediaType objects.
                    if let Some(content) = op["requestBody"]["content"].as_object() {
                        for (mime, media_type) in content {
                            let ptr = format!("{op_base}/requestBody/content/{mime}");
                            check_media_type(doc, media_type, &ptr, &mut violations, self.id());
                        }
                    }

                    // Response content MediaType objects and headers.
                    if let Some(responses) = op["responses"].as_object() {
                        for (status, response) in responses {
                            let resp_base = format!("{op_base}/responses/{status}");

                            // Response content.
                            if let Some(content) = response["content"].as_object() {
                                for (mime, media_type) in content {
                                    let ptr = format!("{resp_base}/content/{mime}");
                                    check_media_type(
                                        doc,
                                        media_type,
                                        &ptr,
                                        &mut violations,
                                        self.id(),
                                    );
                                }
                            }

                            // Response headers.
                            if let Some(headers) = response["headers"].as_object() {
                                for (header_name, header) in headers {
                                    let ptr = format!("{resp_base}/headers/{header_name}");
                                    check_node(doc, header, &ptr, &mut violations, self.id());
                                }
                            }
                        }
                    }
                }
            }
        }

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

/// Check a node (Parameter or Header) that may itself be a `$ref`.
///
/// Resolves the node's `$ref` first (layer 1), then resolves the `schema` field's
/// `$ref` (layer 2), and validates any `example`/`examples` on the resolved node.
fn check_node(doc: &Value, node: &Value, ptr: &str, out: &mut Vec<Violation>, rule_id: &str) {
    // Layer 1: deref the node itself (ADR-021).
    let resolved = if let Some(ref_str) = node.get("$ref").and_then(|v| v.as_str()) {
        match util::resolve_ref(doc, ref_str, 0) {
            Some(r) => r,
            None => return, // external ref, skip
        }
    } else {
        node
    };

    validate_node_examples(doc, resolved, ptr, out, rule_id);
}

/// Check a MediaType object (no $ref at the object level for MediaType).
///
/// MediaType objects in content maps are never `$ref`s themselves; only their
/// `schema` field may be a `$ref`.
fn check_media_type(
    doc: &Value,
    media_type: &Value,
    ptr: &str,
    out: &mut Vec<Violation>,
    rule_id: &str,
) {
    validate_node_examples(doc, media_type, ptr, out, rule_id);
}

/// Validate `example` and `examples` fields on a resolved node against its `schema`.
///
/// Layer 2: resolves the `schema` field's `$ref` if present.
fn validate_node_examples(
    doc: &Value,
    node: &Value,
    ptr: &str,
    out: &mut Vec<Violation>,
    rule_id: &str,
) {
    // Layer 2: resolve the schema field.
    let schema = if let Some(ref_str) = node
        .get("schema")
        .and_then(|s| s.get("$ref"))
        .and_then(|v| v.as_str())
    {
        match util::resolve_ref(doc, ref_str, 0) {
            Some(r) => r,
            None => return, // external schema ref, skip
        }
    } else if let Some(s) = node.get("schema") {
        s
    } else {
        return; // no schema, nothing to validate against
    };

    // Validate inline `example`.
    if let Some(example) = node.get("example") {
        util::validate_example(schema, example, ptr, out, rule_id);
    }

    // Validate `examples` map entries (OAS 3.x `Example` objects).
    if let Some(Value::Object(examples_map)) = node.get("examples") {
        for (example_name, example_obj) in examples_map {
            // Skip `externalValue` examples (no HTTP access).
            if example_obj.get("externalValue").is_some() {
                continue;
            }
            if let Some(value) = example_obj.get("value") {
                let example_path = format!("{ptr}/examples/{example_name}");
                util::validate_example(schema, value, &example_path, out, rule_id);
            }
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
    use crate::lint::LintContext;
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
        let doc = json!({ "swagger": "2.0", "paths": {} });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        assert!(Oas3ValidMediaExample.check(&ctx).is_empty());
    }

    #[test]
    fn skipped_for_unknown() {
        let doc = json!({});
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::Unknown, &schemas);
        assert!(Oas3ValidMediaExample.check(&ctx).is_empty());
    }

    #[test]
    fn valid_mediatype_example_produces_no_violations() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": { "type": "object", "properties": { "name": { "type": "string" } } },
                                        "example": { "name": "Fido" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidMediaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "valid example should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn invalid_mediatype_example_produces_violation() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": { "type": "object", "properties": { "age": { "type": "integer" } } },
                                        "example": { "age": "not-a-number" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidMediaExample.check(&ctx);
        assert!(
            !violations.is_empty(),
            "invalid example should produce at least one violation"
        );
    }

    #[test]
    fn valid_parameter_example_produces_no_violations() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "parameters": [
                            {
                                "name": "limit",
                                "in": "query",
                                "schema": { "type": "integer" },
                                "example": 10
                            }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidMediaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "valid parameter example should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn invalid_parameter_example_produces_violation() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "parameters": [
                            {
                                "name": "limit",
                                "in": "query",
                                "schema": { "type": "integer" },
                                "example": "not-a-number"
                            }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidMediaExample.check(&ctx);
        assert!(
            !violations.is_empty(),
            "invalid parameter example should produce a violation"
        );
    }

    #[test]
    fn valid_header_example_produces_no_violations() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "headers": {
                                    "X-Rate-Limit": {
                                        "schema": { "type": "integer" },
                                        "example": 100
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidMediaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "valid header example should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn invalid_header_example_produces_violation() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "headers": {
                                    "X-Rate-Limit": {
                                        "schema": { "type": "integer" },
                                        "example": "not-a-number"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidMediaExample.check(&ctx);
        assert!(
            !violations.is_empty(),
            "invalid header example should produce a violation"
        );
    }

    #[test]
    fn missing_schema_produces_no_violations() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "example": { "name": "Fido" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidMediaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "missing schema should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn examples_map_value_validated() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": { "type": "integer" },
                                        "examples": {
                                            "foo": { "value": "not-an-integer" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidMediaExample.check(&ctx);
        assert!(
            !violations.is_empty(),
            "invalid examples.foo.value should produce a violation"
        );
    }

    #[test]
    fn external_value_skipped() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": { "type": "integer" },
                                        "examples": {
                                            "foo": { "externalValue": "https://example.com/example.json" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3ValidMediaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "externalValue should be skipped, got: {violations:#?}"
        );
    }
}
