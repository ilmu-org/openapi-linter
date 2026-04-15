use serde_json::Value;

use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// `enum` arrays must not contain duplicate values (deep equality via `serde_json::Value` `PartialEq`).
///
/// Applies to OAS 2.x and 3.x.
pub struct DuplicatedEntryInEnum;

impl Rule for DuplicatedEntryInEnum {
    fn id(&self) -> &'static str {
        "duplicated-entry-in-enum"
    }

    fn message(&self) -> &'static str {
        "enum array must not contain duplicate values."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let mut violations = Vec::new();
        check_value(doc, "", &mut violations);
        violations
    }
}

fn check_value(value: &Value, path: &str, violations: &mut Vec<Violation>) {
    match value {
        Value::Object(obj) => {
            // Check if this object has an "enum" key with an array value.
            if let Some(Value::Array(enum_arr)) = obj.get("enum") {
                let enum_path = format!("{path}/enum");
                for (i, item) in enum_arr.iter().enumerate() {
                    // Check if this value appears earlier in the array.
                    if enum_arr[..i].contains(item) {
                        violations.push(Violation {
                            rule_id: "duplicated-entry-in-enum".to_string(),
                            message: "enum array must not contain duplicate values.".to_string(),
                            severity: Severity::Error,
                            path: format!("{enum_path}/{i}"),
                            line: None,
                            col: None,
                        });
                    }
                }
            }

            // Recurse into object fields.
            for (key, child) in obj {
                let child_path = if path.is_empty() {
                    format!("/{key}")
                } else {
                    format!("{path}/{key}")
                };
                check_value(child, &child_path, violations);
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let child_path = format!("{path}/{i}");
                check_value(item, &child_path, violations);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_on_duplicate_integer() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Status": { "type": "integer", "enum": [1, 2, 1] }
                }
            }
        });
        let v = DuplicatedEntryInEnum.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "duplicated-entry-in-enum");
    }

    #[test]
    fn triggers_on_duplicate_string() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Color": { "type": "string", "enum": ["red", "blue", "red"] }
                }
            }
        });
        let v = DuplicatedEntryInEnum.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
    }

    #[test]
    fn passes_with_unique_enum_values() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Status": { "type": "integer", "enum": [1, 2, 3] }
                }
            }
        });
        assert!(
            DuplicatedEntryInEnum
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn no_enum_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(
            DuplicatedEntryInEnum
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }
}
