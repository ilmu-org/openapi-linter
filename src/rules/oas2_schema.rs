//! `oas2-schema` — validate an OAS 2.0 (Swagger) document against the bundled JSON Schema.
//!
//! Uses the boon JSON Schema validator to check the entire document against the
//! OAS 2.0 (Swagger) JSON Schema. One `Violation` is emitted per boon leaf
//! output unit. Truncated at 64 per ADR-022.

use boon::Compiler;

use crate::lint::LintContext;
use crate::model::{OasVersion, Severity, Violation};
use crate::schemas;

/// Maximum leaf violations emitted per rule call before truncation.
const MAX_VIOLATIONS: usize = 64;

/// Validate the entire OAS 2.0 document against its bundled JSON Schema.
pub struct Oas2Schema;

impl crate::rules::Rule for Oas2Schema {
    fn id(&self) -> &'static str {
        "oas2-schema"
    }

    fn message(&self) -> &'static str {
        "Document does not conform to the OAS 2.0 (Swagger) JSON Schema."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        // Only applies to OAS 2.0 documents.
        if ctx.version != OasVersion::V2 {
            return vec![];
        }

        let schema_uri = schemas::OAS2_SCHEMA_URI;
        let schema_value = schemas::oas2_schema().clone();

        let mut compiler = Compiler::new();
        let mut local_schemas = boon::Schemas::new();
        if compiler.add_resource(schema_uri, schema_value).is_err() {
            return vec![];
        }
        let Ok(sch_index) = compiler.compile(schema_uri, &mut local_schemas) else {
            return vec![];
        };

        let err = match local_schemas.validate(ctx.doc, sch_index) {
            Ok(()) => return vec![],
            Err(e) => e,
        };

        collect_violations(self.id(), &err)
    }
}

/// Collect leaf output units from a boon `ValidationError` into `Violation`s.
fn collect_violations(rule_id: &str, err: &boon::ValidationError<'_, '_>) -> Vec<Violation> {
    let mut violations = Vec::new();
    collect_leaves(rule_id, err, &mut violations);

    if violations.len() > MAX_VIOLATIONS {
        let extra = violations.len() - MAX_VIOLATIONS;
        violations.truncate(MAX_VIOLATIONS);
        violations.push(Violation::new(
            rule_id,
            format!("... {extra} more schema violations omitted"),
            Severity::Error,
            "",
        ));
    }

    violations
}

fn collect_leaves(rule_id: &str, err: &boon::ValidationError<'_, '_>, out: &mut Vec<Violation>) {
    if err.causes.is_empty() {
        let path = format!("{}", err.instance_location);
        let message = format!("{}", err.kind);
        out.push(Violation::new(rule_id, message, Severity::Error, path));
    } else {
        for cause in &err.causes {
            collect_leaves(rule_id, cause, out);
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

    fn make_ctx<'a>(
        doc: &'a serde_json::Value,
        version: OasVersion,
        schemas: &'a Schemas,
    ) -> LintContext<'a> {
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
        assert!(Oas2Schema.check(&ctx).is_empty());
    }

    #[test]
    fn valid_oas2_produces_no_violations() {
        let doc = json!({
            "swagger": "2.0",
            "info": { "title": "Test", "version": "1.0" },
            "paths": {}
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        let violations = Oas2Schema.check(&ctx);
        assert!(
            violations.is_empty(),
            "valid OAS 2.0 doc should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn invalid_oas2_missing_info_produces_violations() {
        // Missing required 'info' field.
        let doc = json!({
            "swagger": "2.0",
            "paths": {}
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        let violations = Oas2Schema.check(&ctx);
        assert!(
            !violations.is_empty(),
            "missing 'info' should produce violations"
        );
    }
}
