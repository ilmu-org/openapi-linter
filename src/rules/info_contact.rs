use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// The `info` object must have a `contact` field.
pub struct InfoContact;

impl Rule for InfoContact {
    fn id(&self) -> &'static str {
        "info-contact"
    }

    fn message(&self) -> &'static str {
        "Info object must have a contact field."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let contact_ok = !doc["info"]["contact"].is_null();
        if contact_ok {
            return vec![];
        }
        vec![Violation {
            rule_id: self.id().to_string(),
            message: self.message().to_string(),
            severity: self.default_severity(),
            path: "/info".to_string(),
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
    fn triggers_when_contact_missing() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0"
"#,
        );
        let violations = InfoContact.check(&doc, OasVersion::V3_0);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_id, "info-contact");
    }

    #[test]
    fn passes_when_contact_present() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0"
  contact:
    name: Support
"#,
        );
        let violations = InfoContact.check(&doc, OasVersion::V3_0);
        assert!(violations.is_empty());
    }
}
