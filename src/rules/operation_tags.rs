use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule};

/// Every operation must have a non-empty `tags` array.
pub struct OperationTags;

impl Rule for OperationTags {
    fn id(&self) -> &'static str {
        "operation-tags"
    }

    fn message(&self) -> &'static str {
        "Operation must have a non-empty tags array."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let mut violations = Vec::new();

        let Some(paths) = doc["paths"].as_object() else {
            return violations;
        };

        for (path_key, path_item) in paths {
            for method in HTTP_METHODS {
                let Some(operation) = path_item.get(*method) else {
                    continue;
                };
                let tags_ok = operation["tags"].as_array().is_some_and(|a| !a.is_empty());
                if !tags_ok {
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        message: self.message().to_string(),
                        severity: self.default_severity(),
                        path: format!("/paths/{path_key}/{method}"),
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

    fn parse_yaml(s: &str) -> serde_json::Value {
        let v: serde_yaml::Value = serde_yaml::from_str(s).unwrap();
        serde_json::to_value(v).unwrap()
    }

    #[test]
    fn triggers_when_tags_missing() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
paths:
  /foo:
    get:
      operationId: getFoo
      responses:
        "200":
          description: OK
"#,
        );
        let violations = OperationTags.check(&doc, OasVersion::V3_0);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_id, "operation-tags");
    }

    #[test]
    fn passes_when_tags_present() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
paths:
  /foo:
    get:
      operationId: getFoo
      tags: [foo]
      responses:
        "200":
          description: OK
"#,
        );
        let violations = OperationTags.check(&doc, OasVersion::V3_0);
        assert!(violations.is_empty());
    }
}
