//! `oas2-valid-media-example` — validate OAS 2.x response-level `examples` maps.
//!
//! In OAS 2.x the Response Object carries an `examples` map of the form
//! `{ "mime-type": <value> }`. Each value is a literal example (not an Example
//! Object as in OAS 3.x). This rule validates each such value against the
//! response's `schema` field.
//!
//! This is distinct from `oas2-valid-schema-example`, which validates the
//! schema-level `example` field on Schema Objects.
//!
//! Only document-local `$ref`s are resolved (ADR-021). External refs are silently
//! skipped. Truncated at 64 violations (ADR-022).

use crate::lint::LintContext;
use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, util};

/// Maximum leaf violations emitted per rule call before truncation.
const MAX_VIOLATIONS: usize = 64;

/// Validate OAS 2.x response-level `examples` map values against the response schema.
pub struct Oas2ValidMediaExample;

impl crate::rules::Rule for Oas2ValidMediaExample {
    fn id(&self) -> &'static str {
        "oas2-valid-media-example"
    }

    fn message(&self) -> &'static str {
        "Example does not validate against its schema."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        if ctx.version != OasVersion::V2 {
            return vec![];
        }

        let doc = ctx.doc;
        let mut violations = Vec::new();

        if let Some(paths) = doc["paths"].as_object() {
            for (path_key, path_item) in paths {
                let path_enc = path_key.replace('~', "~0").replace('/', "~1");

                for method in HTTP_METHODS {
                    let Some(op) = path_item.get(*method) else {
                        continue;
                    };

                    let Some(responses) = op["responses"].as_object() else {
                        continue;
                    };

                    for (status, response) in responses {
                        let resp_base = format!("/paths/{path_enc}/{method}/responses/{status}");

                        // Layer 1: deref the response object itself if it is a $ref.
                        let resolved_resp =
                            if let Some(ref_str) = response.get("$ref").and_then(|v| v.as_str()) {
                                match util::resolve_ref(doc, ref_str, 0) {
                                    Some(r) => r,
                                    None => continue, // external ref, skip
                                }
                            } else {
                                response
                            };

                        // Both `schema` and `examples` must be present to validate.
                        let Some(examples_map) = resolved_resp["examples"].as_object() else {
                            continue;
                        };

                        // Layer 2: resolve the schema field's $ref if present.
                        let schema = if let Some(ref_str) = resolved_resp
                            .get("schema")
                            .and_then(|s| s.get("$ref"))
                            .and_then(|v| v.as_str())
                        {
                            match util::resolve_ref(doc, ref_str, 0) {
                                Some(r) => r,
                                None => continue, // external schema ref, skip
                            }
                        } else if let Some(s) = resolved_resp.get("schema") {
                            s
                        } else {
                            continue; // no schema, nothing to validate against
                        };

                        // Validate each mime-type keyed example value directly.
                        for (mime, example_value) in examples_map {
                            let example_path = format!("{resp_base}/examples/{mime}");
                            util::validate_example(
                                schema,
                                example_value,
                                &example_path,
                                &mut violations,
                                self.id(),
                            );
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use boon::Schemas;
    use serde_json::{Value, json};

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
    fn skipped_for_oas3() {
        let doc = json!({ "openapi": "3.0.3", "paths": {} });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        assert!(Oas2ValidMediaExample.check(&ctx).is_empty());
    }

    #[test]
    fn valid_example_produces_no_violations() {
        let doc = json!({
            "swagger": "2.0",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "schema": { "type": "object", "properties": { "name": { "type": "string" } } },
                                "examples": {
                                    "application/json": { "name": "Fido" }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        let violations = Oas2ValidMediaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "valid example should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn invalid_example_produces_violation() {
        let doc = json!({
            "swagger": "2.0",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "schema": { "type": "object", "properties": { "age": { "type": "integer" } } },
                                "examples": {
                                    "application/json": { "age": "not-a-number" }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        let violations = Oas2ValidMediaExample.check(&ctx);
        assert!(
            !violations.is_empty(),
            "invalid example should produce a violation"
        );
    }

    #[test]
    fn missing_schema_produces_no_violations() {
        let doc = json!({
            "swagger": "2.0",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "examples": {
                                    "application/json": { "name": "Fido" }
                                }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        let violations = Oas2ValidMediaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "missing schema should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn missing_examples_produces_no_violations() {
        let doc = json!({
            "swagger": "2.0",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "schema": { "type": "object", "properties": { "name": { "type": "string" } } }
                            }
                        }
                    }
                }
            }
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        let violations = Oas2ValidMediaExample.check(&ctx);
        assert!(
            violations.is_empty(),
            "missing examples should produce no violations, got: {violations:#?}"
        );
    }
}
