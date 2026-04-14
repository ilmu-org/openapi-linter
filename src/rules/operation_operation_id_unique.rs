use std::collections::HashMap;

use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{HTTP_METHODS, Rule};

/// All `operationId` values across the spec must be unique.
pub struct OperationOperationIdUnique;

impl Rule for OperationOperationIdUnique {
    fn id(&self) -> &'static str {
        "operation-operationId-unique"
    }

    fn message(&self) -> &'static str {
        "operationId must be unique across all operations."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        // Collect all (operationId, path) pairs first, then detect duplicates.
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut violations = Vec::new();

        let Some(paths) = doc["paths"].as_object() else {
            return violations;
        };

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

                let current_path = format!("/paths/{path_key}/{method}");
                if let Some(first_path) = seen.get(op_id) {
                    violations.push(Violation {
                        rule_id: self.id().to_string(),
                        message: format!("operationId '{op_id}' is already used at {first_path}."),
                        severity: self.default_severity(),
                        path: current_path,
                        line: None,
                        col: None,
                    });
                } else {
                    seen.insert(op_id.to_string(), current_path);
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
    fn triggers_on_duplicate_operation_id() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
paths:
  /foo:
    get:
      operationId: listItems
      responses:
        "200":
          description: OK
  /bar:
    get:
      operationId: listItems
      responses:
        "200":
          description: OK
"#,
        );
        let violations = OperationOperationIdUnique.check(&doc, OasVersion::V3_0);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_id, "operation-operationId-unique");
    }

    #[test]
    fn passes_when_all_unique() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
paths:
  /foo:
    get:
      operationId: listFoo
      responses:
        "200":
          description: OK
  /bar:
    get:
      operationId: listBar
      responses:
        "200":
          description: OK
"#,
        );
        let violations = OperationOperationIdUnique.check(&doc, OasVersion::V3_0);
        assert!(violations.is_empty());
    }
}
