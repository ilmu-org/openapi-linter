use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// Path keys must not end with a trailing slash.
///
/// Applies to OAS 2.x and 3.x. The root path "/" is exempt.
pub struct PathKeysNoTrailingSlash;

impl Rule for PathKeysNoTrailingSlash {
    fn id(&self) -> &'static str {
        "path-keys-no-trailing-slash"
    }

    fn message(&self) -> &'static str {
        "Path key must not end with a trailing slash."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let Some(paths) = doc["paths"].as_object() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for path_key in paths.keys() {
            // The root path "/" is not a trailing slash.
            if path_key == "/" {
                continue;
            }
            if path_key.ends_with('/') {
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
    fn triggers_on_trailing_slash() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets/": {}
            }
        });
        let v = PathKeysNoTrailingSlash.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "path-keys-no-trailing-slash");
    }

    #[test]
    fn passes_without_trailing_slash() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {}
            }
        });
        assert!(
            PathKeysNoTrailingSlash
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn root_path_exempt() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/": {}
            }
        });
        assert!(
            PathKeysNoTrailingSlash
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn passes_on_oas2() {
        let doc = json!({
            "swagger": "2.0",
            "paths": {
                "/pets": {}
            }
        });
        assert!(
            PathKeysNoTrailingSlash
                .check(&doc, OasVersion::V2)
                .is_empty()
        );
    }

    #[test]
    fn no_paths_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(
            PathKeysNoTrailingSlash
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }
}
