use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// The `info` object must have a non-empty `description` string.
pub struct InfoDescription;

impl Rule for InfoDescription {
    fn id(&self) -> &'static str {
        "info-description"
    }

    fn message(&self) -> &'static str {
        "Info object must have a non-empty description."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let desc_ok = doc["info"]["description"]
            .as_str()
            .is_some_and(|s| !s.is_empty());
        if desc_ok {
            return vec![];
        }
        vec![Violation {
            rule_id: self.id().to_string(),
            message: self.message().to_string(),
            severity: self.default_severity(),
            path: "/info/description".to_string(),
            line: None,
            col: None,
        }]
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
info:
  title: Test
  version: "1.0"
"#,
        );
        let violations = InfoDescription.check(&doc, OasVersion::V3_0);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_id, "info-description");
    }

    #[test]
    fn passes_when_description_present() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0"
  description: A test API.
"#,
        );
        let violations = InfoDescription.check(&doc, OasVersion::V3_0);
        assert!(violations.is_empty());
    }
}
