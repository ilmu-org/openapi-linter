use crate::lint::LintContext;
use crate::model::{Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule};

/// Every operation must define at least one 2xx success response.
///
/// The `default` response key does not count as a success response.
/// Applies to OAS 2.x and 3.x.
pub struct OperationSuccessResponse;

impl Rule for OperationSuccessResponse {
    fn id(&self) -> &'static str {
        "operation-success-response"
    }

    fn message(&self) -> &'static str {
        "Operation must define at least one 2xx success response."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation> {
        let doc = ctx.doc;
        let Some(paths) = doc["paths"].as_object() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for (path_key, path_item) in paths {
            for method in HTTP_METHODS {
                let Some(operation) = path_item.get(*method) else {
                    continue;
                };

                let has_success = operation["responses"]
                    .as_object()
                    .is_some_and(|responses| responses.keys().any(|code| code.starts_with('2')));

                if !has_success {
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        message: self.message().to_string(),
                        severity: self.default_severity(),
                        path: format!("/paths/{path_key}/{method}/responses"),
                        line: None,
                        col: None,
                    });
                }
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
    fn triggers_when_no_2xx_response() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "400": { "description": "Bad request" },
                            "500": { "description": "Server error" }
                        }
                    }
                }
            }
        });
        let v = OperationSuccessResponse.check(&crate::lint::LintContext {
            doc: &doc,
            version: crate::model::OasVersion::V3_0,
            schemas: &boon::Schemas::new(),
            base_path: None,
        });
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "operation-success-response");
    }

    #[test]
    fn default_does_not_count_as_success() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "default": { "description": "Unexpected error" }
                        }
                    }
                }
            }
        });
        let v = OperationSuccessResponse.check(&crate::lint::LintContext {
            doc: &doc,
            version: crate::model::OasVersion::V3_0,
            schemas: &boon::Schemas::new(),
            base_path: None,
        });
        assert!(!v.is_empty());
    }

    #[test]
    fn passes_with_200_response() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "get": {
                        "responses": {
                            "200": { "description": "OK" }
                        }
                    }
                }
            }
        });
        assert!(
            OperationSuccessResponse
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
    fn passes_with_201_response() {
        let doc = json!({
            "openapi": "3.0.3",
            "paths": {
                "/pets": {
                    "post": {
                        "responses": {
                            "201": { "description": "Created" }
                        }
                    }
                }
            }
        });
        assert!(
            OperationSuccessResponse
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
            OperationSuccessResponse
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
