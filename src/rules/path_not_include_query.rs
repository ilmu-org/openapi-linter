use crate::lint::LintContext;
use crate::model::{Severity, Violation};
use crate::rules::Rule;

/// Path keys must not contain a query string (the `?` character).
///
/// Applies to OAS 2.x and 3.x.
pub struct PathNotIncludeQuery;

impl Rule for PathNotIncludeQuery {
    fn id(&self) -> &'static str {
        "path-not-include-query"
    }

    fn message(&self) -> &'static str {
        "Path key must not include a query string."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        let doc = ctx.doc;
        let Some(paths) = doc["paths"].as_object() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for path_key in paths.keys() {
            if path_key.contains('?') {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    message: self.message().to_string(),
                    severity: self.default_severity(),
                    path: format!("/paths/{path_key}"),
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
    fn triggers_on_query_in_path() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets?type=dog": {}
            }
        });
        let v = PathNotIncludeQuery.check(&crate::lint::LintContext {
            doc: &doc,
            version: crate::model::OasVersion::V3_0,
            schemas: &boon::Schemas::new(),
            base_path: None,
        });
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "path-not-include-query");
    }

    #[test]
    fn passes_on_clean_path() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {}
            }
        });
        assert!(
            PathNotIncludeQuery
                .check(&crate::lint::LintContext {
                    doc: &doc,
                    version: crate::model::OasVersion::V3_0,
                    schemas: &boon::Schemas::new(),
                    base_path: None
                })
                .is_empty()
        );
    }

    #[test]
    fn no_paths_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(
            PathNotIncludeQuery
                .check(&crate::lint::LintContext {
                    doc: &doc,
                    version: crate::model::OasVersion::V3_0,
                    schemas: &boon::Schemas::new(),
                    base_path: None
                })
                .is_empty()
        );
    }
}
