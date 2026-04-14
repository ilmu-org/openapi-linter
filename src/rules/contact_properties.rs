use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// When `info.contact` is present it must have at least a `name` or `email` field.
pub struct ContactProperties;

impl Rule for ContactProperties {
    fn id(&self) -> &'static str {
        "contact-properties"
    }

    fn message(&self) -> &'static str {
        "Contact object must have at least a name or email field."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let contact = &doc["info"]["contact"];
        if contact.is_null() {
            return vec![];
        }
        let has_name = contact["name"].as_str().is_some_and(|s| !s.is_empty());
        let has_email = contact["email"].as_str().is_some_and(|s| !s.is_empty());
        if has_name || has_email {
            return vec![];
        }
        vec![Violation {
            rule_id: self.id().to_string(),
            message: self.message().to_string(),
            severity: self.default_severity(),
            path: "/info/contact".to_string(),
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
    fn passes_when_no_contact() {
        let doc = parse_yaml("openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\n");
        assert!(ContactProperties.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn passes_with_name() {
        let doc = parse_yaml(
            "openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\n  contact:\n    name: Support\n",
        );
        assert!(ContactProperties.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn passes_with_email() {
        let doc = parse_yaml(
            "openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\n  contact:\n    email: support@example.com\n",
        );
        assert!(ContactProperties.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn triggers_when_contact_empty() {
        let doc = parse_yaml(
            "openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\n  contact:\n    url: https://example.com\n",
        );
        assert!(!ContactProperties.check(&doc, OasVersion::V3_0).is_empty());
    }
}
