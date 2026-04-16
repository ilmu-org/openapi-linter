//! `oas3-schema` — validate an OAS 3.x document against the bundled OAS JSON Schema.
//!
//! Uses the boon JSON Schema validator to check the entire document against the
//! OAS 3.0 or 3.1 JSON Schema (selected by the document's detected version).
//! One `Violation` is emitted per boon leaf output unit (instance location +
//! error message). Truncated at 64 violations per call to prevent unbounded
//! output on heavily malformed documents (ADR-022).
//!
//! Note: the boon `Compiler::compile` call is cheap when the schema is already
//! parsed (OnceLock) and the validation itself is the dominant cost.

use boon::Compiler;

use crate::lint::LintContext;
use crate::model::{OasVersion, Severity, Violation};
use crate::schemas;

/// Maximum leaf violations emitted per rule call before truncation.
const MAX_VIOLATIONS: usize = 64;

/// Validate the entire OAS 3.x document against its bundled JSON Schema.
pub struct Oas3Schema;

impl crate::rules::Rule for Oas3Schema {
    fn id(&self) -> &'static str {
        "oas3-schema"
    }

    fn message(&self) -> &'static str {
        "Document does not conform to the OAS 3.x JSON Schema."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        // Only applies to OAS 3.x documents.
        match ctx.version {
            OasVersion::V3_0 | OasVersion::V3_1 => {}
            _ => return vec![],
        }

        let (schema_uri, schema_value) = match ctx.version {
            OasVersion::V3_0 => (schemas::OAS3_0_SCHEMA_URI, schemas::oas3_0_schema().clone()),
            _ => (schemas::OAS3_1_SCHEMA_URI, schemas::oas3_1_schema().clone()),
        };

        // Build a local boon registry and compile the schema.
        // The schema Value is already parsed (OnceLock), so this is cheap.
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
///
/// Non-leaf output units (those with child causes) are skipped per ADR-022.
/// Truncates at [`MAX_VIOLATIONS`] and appends a summary if exceeded.
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

/// Recursively collect leaf output units from a boon error tree.
///
/// `ValidationError` has public fields: `causes` (Vec), `instance_location`, `kind`.
fn collect_leaves(rule_id: &str, err: &boon::ValidationError<'_, '_>, out: &mut Vec<Violation>) {
    if err.causes.is_empty() {
        // Leaf node: emit a violation.
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
    fn skipped_for_oas2() {
        let doc = json!({ "swagger": "2.0" });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V2, &schemas);
        assert!(Oas3Schema.check(&ctx).is_empty());
    }

    #[test]
    fn skipped_for_unknown_version() {
        let doc = json!({ "foo": "bar" });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::Unknown, &schemas);
        assert!(Oas3Schema.check(&ctx).is_empty());
    }

    #[test]
    fn valid_oas30_produces_no_violations() {
        let doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Test", "version": "1.0" },
            "paths": {}
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3Schema.check(&ctx);
        assert!(
            violations.is_empty(),
            "valid OAS 3.0 doc should produce no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn invalid_oas30_missing_info_produces_violations() {
        // Missing required 'info' field.
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {}
        });
        let schemas = Schemas::new();
        let ctx = make_ctx(&doc, OasVersion::V3_0, &schemas);
        let violations = Oas3Schema.check(&ctx);
        assert!(
            !violations.is_empty(),
            "missing 'info' should produce violations"
        );
    }
}
