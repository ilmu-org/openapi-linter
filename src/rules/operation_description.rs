use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule};

/// Every operation should have a non-empty `description` string.
pub struct OperationDescription;

impl Rule for OperationDescription {
    fn id(&self) -> &'static str {
        "operation-description"
    }

    fn message(&self) -> &'static str {
        "Operation should have a non-empty description."
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
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
                let desc_ok = operation["description"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty());
                if !desc_ok {
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        message: self.message().to_string(),
                        severity: self.default_severity(),
                        path: format!("/paths/{path_key}/{method}"),
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
    fn triggers_when_description_missing() {
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
        let violations = OperationDescription.check(&doc, OasVersion::V3_0);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_id, "operation-description");
    }

    #[test]
    fn passes_when_description_present() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
paths:
  /foo:
    get:
      operationId: getFoo
      description: Returns a single foo resource.
      responses:
        "200":
          description: OK
"#,
        );
        let violations = OperationDescription.check(&doc, OasVersion::V3_0);
        assert!(violations.is_empty());
    }
}
