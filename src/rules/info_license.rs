use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// The `info` object must have a `license` field.
pub struct InfoLicense;

impl Rule for InfoLicense {
    fn id(&self) -> &'static str {
        "info-license"
    }

    fn message(&self) -> &'static str {
        "Info object must have a license field."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        if doc["info"]["license"].is_null() {
            vec![Violation {
                rule_id: self.id().to_string(),
                message: self.message().to_string(),
                severity: self.default_severity(),
                path: "/info/license".to_string(),
                line: None,
                col: None,
            }]
        } else {
            vec![]
        }
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
    fn triggers_when_license_missing() {
        let doc = parse_yaml("openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\n");
        assert!(!InfoLicense.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn passes_when_license_present() {
        let doc = parse_yaml(
            "openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\n  license:\n    name: MIT\n",
        );
        assert!(InfoLicense.check(&doc, OasVersion::V3_0).is_empty());
    }
}
