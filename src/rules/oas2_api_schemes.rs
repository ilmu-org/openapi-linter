use crate::lint::LintContext;
use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

const VALID_SCHEMES: &[&str] = &["http", "https", "ws", "wss"];

/// An OAS 2.x API must declare at least one valid transfer scheme.
///
/// Checks that the top-level `schemes` array is present, non-empty, and
/// contains only values from `{"http", "https", "ws", "wss"}`.
///
/// Applies to OAS 2.x only.
pub struct Oas2ApiSchemes;

impl Rule for Oas2ApiSchemes {
    fn id(&self) -> &'static str {
        "oas2-api-schemes"
    }

    fn message(&self) -> &'static str {
        "API must define at least one scheme."
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

        let schemes = &doc["schemes"];

        // Missing, null, or not an array.
        let Some(arr) = schemes.as_array() else {
            violations.push(Violation {
                rule_id: "oas2-api-schemes".to_string(),
                message: "API must define at least one scheme.".to_string(),
                severity: Severity::Warn,
                path: "/schemes".to_string(),
                line: None,
                col: None,
            });
            return violations;
        };

        // Empty array.
        if arr.is_empty() {
            violations.push(Violation {
                rule_id: "oas2-api-schemes".to_string(),
                message: "API must define at least one scheme.".to_string(),
                severity: Severity::Warn,
                path: "/schemes".to_string(),
                line: None,
                col: None,
            });
            return violations;
        }

        // Validate each entry.
        for (i, entry) in arr.iter().enumerate() {
            let valid = entry.as_str().is_some_and(|s| VALID_SCHEMES.contains(&s));

            if !valid {
                violations.push(Violation {
                    rule_id: "oas2-api-schemes".to_string(),
                    message: "Scheme must be one of http, https, ws, wss.".to_string(),
                    severity: Severity::Warn,
                    path: format!("/schemes/{i}"),
                    line: None,
                    col: None,
                });
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_when_schemes_absent() {
        let doc = json!({
            "swagger": "2.0",
            "info": { "title": "Test", "version": "1.0" },
            "paths": {}
        });
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V2,
            schemas: &schemas,
            base_path: None,
        };
        let v = Oas2ApiSchemes.check(&ctx);
        assert_eq!(v.len(), 1, "expected 1 violation for absent schemes");
        assert_eq!(v[0].path, "/schemes");
        assert_eq!(v[0].rule_id, "oas2-api-schemes");
    }

    #[test]
    fn triggers_when_schemes_empty() {
        let doc = json!({
            "swagger": "2.0",
            "info": { "title": "Test", "version": "1.0" },
            "paths": {},
            "schemes": []
        });
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V2,
            schemas: &schemas,
            base_path: None,
        };
        let v = Oas2ApiSchemes.check(&ctx);
        assert_eq!(v.len(), 1, "expected 1 violation for empty schemes");
        assert_eq!(v[0].path, "/schemes");
    }

    #[test]
    fn triggers_on_invalid_scheme() {
        let doc = json!({
            "swagger": "2.0",
            "info": { "title": "Test", "version": "1.0" },
            "paths": {},
            "schemes": ["ftp"]
        });
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V2,
            schemas: &schemas,
            base_path: None,
        };
        let v = Oas2ApiSchemes.check(&ctx);
        assert_eq!(v.len(), 1, "expected 1 violation for invalid scheme");
        assert_eq!(v[0].path, "/schemes/0");
        assert!(v[0].message.contains("http, https, ws, wss"));
    }

    #[test]
    fn passes_with_valid_schemes() {
        let doc = json!({
            "swagger": "2.0",
            "info": { "title": "Test", "version": "1.0" },
            "paths": {},
            "schemes": ["https", "wss"]
        });
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V2,
            schemas: &schemas,
            base_path: None,
        };
        assert!(
            Oas2ApiSchemes.check(&ctx).is_empty(),
            "valid schemes must not produce violations"
        );
    }

    #[test]
    fn skipped_for_oas3() {
        let doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Test", "version": "1.0" },
            "paths": {}
        });
        let schemas = boon::Schemas::new();
        let ctx = LintContext {
            doc: &doc,
            version: OasVersion::V3_0,
            schemas: &schemas,
            base_path: None,
        };
        assert!(
            Oas2ApiSchemes.check(&ctx).is_empty(),
            "rule must not fire for OAS 3.x"
        );
    }
}
