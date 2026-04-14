use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// The top-level `tags` array must exist and be non-empty.
pub struct OpenApiTags;

impl Rule for OpenApiTags {
    fn id(&self) -> &'static str {
        "openapi-tags"
    }

    fn message(&self) -> &'static str {
        "OpenAPI document must have a non-empty top-level tags array."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let tags_ok = doc["tags"].as_array().is_some_and(|a| !a.is_empty());
        if tags_ok {
            return vec![];
        }
        vec![Violation {
            rule_id: self.id().to_string(),
            message: self.message().to_string(),
            severity: self.default_severity(),
            path: "/tags".to_string(),
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
    fn triggers_when_tags_missing() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0"
"#,
        );
        let violations = OpenApiTags.check(&doc, OasVersion::V3_0);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].rule_id, "openapi-tags");
    }

    #[test]
    fn passes_when_tags_present() {
        let doc = parse_yaml(
            r#"
openapi: "3.0.3"
tags:
  - name: pets
"#,
        );
        let violations = OpenApiTags.check(&doc, OasVersion::V3_0);
        assert!(violations.is_empty());
    }
}
