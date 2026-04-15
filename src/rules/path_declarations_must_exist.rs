use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// Path templates must not contain empty `{}` or `{ }` placeholders.
///
/// Applies to OAS 2.x and 3.x.
pub struct PathDeclarationsMustExist;

impl Rule for PathDeclarationsMustExist {
    fn id(&self) -> &'static str {
        "path-declarations-must-exist"
    }

    fn message(&self) -> &'static str {
        "Path template contains an empty parameter placeholder."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let Some(paths) = doc["paths"].as_object() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for path_key in paths.keys() {
            if has_empty_placeholder(path_key) {
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

/// Returns true if `path` contains an empty or whitespace-only `{}` placeholder.
fn has_empty_placeholder(path: &str) -> bool {
    let mut chars = path.chars();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            let inner: String = chars.by_ref().take_while(|&c| c != '}').collect();
            if inner.trim().is_empty() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_on_empty_placeholder() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets/{}": {}
            }
        });
        let v = PathDeclarationsMustExist.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "path-declarations-must-exist");
    }

    #[test]
    fn triggers_on_whitespace_placeholder() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets/{ }": {}
            }
        });
        let v = PathDeclarationsMustExist.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
    }

    #[test]
    fn passes_on_named_placeholder() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets/{petId}": {}
            }
        });
        assert!(
            PathDeclarationsMustExist
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn no_paths_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(
            PathDeclarationsMustExist
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }
}
