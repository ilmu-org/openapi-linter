use serde_json::Value;

use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// Each value in an `enum` array must be compatible with the declared schema `type`.
///
/// Coercion semantics (ADR-021):
/// - `integer` and `number` accept any JSON numeric `Value`.
/// - `integer` additionally requires `fract() == 0.0` to permit YAML-coerced values
///   such as `1.0`. This means `1e30` passes (fract() == 0.0 in f64), `-0.0` passes.
/// - `string` requires `Value::String`.
/// - `boolean` requires `Value::Bool`.
/// - `null` requires `Value::Null`.
/// - `array` requires `Value::Array`.
/// - `object` requires `Value::Object`.
/// - OAS 3.1 multi-type schemas (`type: ["string", "null"]`) pass if any listed type matches.
///
/// Applies to OAS 2.x and 3.x.
pub struct TypedEnum;

impl Rule for TypedEnum {
    fn id(&self) -> &'static str {
        "typed-enum"
    }

    fn message(&self) -> &'static str {
        "enum value is not compatible with the declared schema type."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
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
            // If this object has both "type" and "enum", check compatibility.
            if let Some(Value::Array(enum_arr)) = obj.get("enum")
                && let Some(type_val) = obj.get("type")
            {
                let enum_path = format!("{path}/enum");
                for (i, item) in enum_arr.iter().enumerate() {
                    if !value_matches_type(item, type_val) {
                        violations.push(Violation {
                            rule_id: "typed-enum".to_string(),
                            message: "enum value is not compatible with the declared schema type."
                                .to_string(),
                            severity: Severity::Warn,
                            path: format!("{enum_path}/{i}"),
                            line: None,
                            col: None,
                        });
                    }
                }
            }

            // Recurse.
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

/// Returns true if `value` is compatible with `type_decl`.
///
/// Handles both single-type strings and multi-type arrays (OAS 3.1).
fn value_matches_type(value: &Value, type_decl: &Value) -> bool {
    match type_decl {
        Value::String(type_str) => matches_single_type(value, type_str),
        Value::Array(types) => {
            // OAS 3.1 multi-type: pass if any type matches.
            types
                .iter()
                .filter_map(|t| t.as_str())
                .any(|t| matches_single_type(value, t))
        }
        _ => true, // Unknown type declaration: do not flag.
    }
}

/// Returns true if `value` is compatible with a single `type` string.
///
/// Coercion semantics per ADR-021:
/// - `integer`: any JSON number where `fract() == 0.0` (permits `1.0`, `1e30`, `-0.0`).
/// - `number`: any JSON number.
fn matches_single_type(value: &Value, type_str: &str) -> bool {
    match type_str {
        "integer" => match value {
            Value::Number(n) => {
                n.is_i64() || n.is_u64() || n.as_f64().is_some_and(|f| f.fract() == 0.0)
            }
            _ => false,
        },
        "number" => matches!(value, Value::Number(_)),
        "string" => matches!(value, Value::String(_)),
        "boolean" => matches!(value, Value::Bool(_)),
        "null" => matches!(value, Value::Null),
        "array" => matches!(value, Value::Array(_)),
        "object" => matches!(value, Value::Object(_)),
        _ => true, // Unknown type: do not flag.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_when_string_under_integer_type() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Status": { "type": "integer", "enum": ["cat", "dog"] }
                }
            }
        });
        let v = TypedEnum.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "typed-enum");
    }

    #[test]
    fn passes_integers_under_integer_type() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Status": { "type": "integer", "enum": [1, 2, 3] }
                }
            }
        });
        assert!(TypedEnum.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn passes_float_integers_under_integer_type() {
        // 1.0, 2.0 have fract() == 0.0, so they pass per ADR-021.
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Status": { "type": "integer", "enum": [1.0, 2.0] }
                }
            }
        });
        assert!(TypedEnum.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn passes_1e30_under_integer_type() {
        // 1e30 has fract() == 0.0 in f64, so it passes per ADR-021. Freezes edge-case behavior.
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Big": { "type": "integer", "enum": [1e30] }
                }
            }
        });
        assert!(TypedEnum.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn passes_negative_zero_under_integer_type() {
        // -0.0 has fract() == 0.0 so it passes. Freezes edge-case behavior.
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Zero": { "type": "integer", "enum": [-0.0] }
                }
            }
        });
        assert!(TypedEnum.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn oas31_multi_type_passes_when_any_matches() {
        let doc = json!({
            "openapi": "3.1.0",
            "components": {
                "schemas": {
                    "NullableString": {
                        "type": ["string", "null"],
                        "enum": ["hello", null]
                    }
                }
            }
        });
        assert!(TypedEnum.check(&doc, OasVersion::V3_1).is_empty());
    }

    #[test]
    fn triggers_string_under_number_type() {
        let doc = json!({
            "openapi": "3.0.3",
            "components": {
                "schemas": {
                    "Rate": { "type": "number", "enum": ["fast", 1.5] }
                }
            }
        });
        let v = TypedEnum.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
    }

    #[test]
    fn no_enum_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(TypedEnum.check(&doc, OasVersion::V3_0).is_empty());
    }
}
