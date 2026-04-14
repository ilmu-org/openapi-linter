use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// When `info.license` is present it must also have a `url` field.
pub struct LicenseUrl;

impl Rule for LicenseUrl {
    fn id(&self) -> &'static str {
        "license-url"
    }

    fn message(&self) -> &'static str {
        "License object must have a url field."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let license = &doc["info"]["license"];
        if license.is_null() {
            // No license present — nothing to check.
            return vec![];
        }
        if license["url"].as_str().is_some_and(|s| !s.is_empty()) {
            return vec![];
        }
        vec![Violation {
            rule_id: self.id().to_string(),
            message: self.message().to_string(),
            severity: self.default_severity(),
            path: "/info/license/url".to_string(),
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
    fn passes_when_no_license() {
        let doc = parse_yaml("openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\n");
        assert!(LicenseUrl.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn triggers_when_license_has_no_url() {
        let doc = parse_yaml(
            "openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\n  license:\n    name: MIT\n",
        );
        assert!(!LicenseUrl.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn passes_when_license_has_url() {
        let doc = parse_yaml(
            "openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\n  license:\n    name: MIT\n    url: https://opensource.org/licenses/MIT\n",
        );
        assert!(LicenseUrl.check(&doc, OasVersion::V3_0).is_empty());
    }
}
