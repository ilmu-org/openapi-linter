use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule};

/// `operationId` values must consist only of URL path-segment-safe characters.
///
/// Matches Spectral permissive default: catches whitespace and non-URL-safe
/// characters only. Allowed set: `A-Za-z0-9`, and `-._~:@!$&()*+,;=`.
///
/// Applies to OAS 2.x and 3.x.
pub struct OperationOperationIdValidInUrl;

impl Rule for OperationOperationIdValidInUrl {
    fn id(&self) -> &'static str {
        "operation-operationId-valid-in-url"
    }

    fn message(&self) -> &'static str {
        "operationId contains characters that are not safe for use in a URL path segment."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let Some(paths) = doc["paths"].as_object() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for (path_key, path_item) in paths {
            for method in HTTP_METHODS {
                let Some(operation) = path_item.get(*method) else {
                    continue;
                };

                let Some(op_id) = operation["operationId"].as_str() else {
                    continue;
                };
                if op_id.is_empty() {
                    continue;
                }

                if !is_valid_op_id(op_id) {
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        message: self.message().to_string(),
                        severity: self.default_severity(),
                        path: format!("/paths/{path_key}/{method}/operationId"),
                        line: None,
                        col: None,
                    });
                }
            }
        }

        violations
    }
}

/// Returns true if every character in `s` is in the allowed URL path-segment set.
///
/// Allowed: `A-Za-z0-9` and `-._~:@!$&()*+,;=`.
/// Matches Spectral permissive default.
fn is_valid_op_id(s: &str) -> bool {
    s.chars().all(|c| {
        matches!(c,
            'A'..='Z' | 'a'..='z' | '0'..='9' |
            '-' | '.' | '_' | '~' | ':' | '@' | '!' | '$' |
            '&' | '(' | ')' | '*' | '+' | ',' | ';' | '='
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_on_space_in_operation_id() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": { "operationId": "get pets list", "responses": {} }
                }
            }
        });
        let v = OperationOperationIdValidInUrl.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "operation-operationId-valid-in-url");
    }

    #[test]
    fn passes_with_alphanumeric_id() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": { "operationId": "listPets", "responses": {} }
                }
            }
        });
        assert!(
            OperationOperationIdValidInUrl
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn passes_with_allowed_special_chars() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": { "operationId": "list-pets.all_v2~x:y@z!$&(test)*+,;=", "responses": {} }
                }
            }
        });
        assert!(
            OperationOperationIdValidInUrl
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn skipped_when_no_operation_id() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": { "responses": {} }
                }
            }
        });
        assert!(
            OperationOperationIdValidInUrl
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn triggers_on_non_ascii_char() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": { "operationId": "getPets\u{00e9}", "responses": {} }
                }
            }
        });
        let v = OperationOperationIdValidInUrl.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
    }
}
