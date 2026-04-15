use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// Every top-level tag must have a non-empty description.
///
/// Applies to OAS 2.x and 3.x.
pub struct TagDescription;

impl Rule for TagDescription {
    fn id(&self) -> &'static str {
        "tag-description"
    }

    fn message(&self) -> &'static str {
        "Tag must have a non-empty description."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let Some(tags) = doc["tags"].as_array() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for (index, tag) in tags.iter().enumerate() {
            let has_description = tag["description"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty());
            if !has_description {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    message: self.message().to_string(),
                    severity: self.default_severity(),
                    path: format!("/tags/{index}"),
                    line: None,
                    col: None,
                });
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_when_description_missing() {
        let doc = json!({
            "openapi": "3.0.3",
            "tags": [{ "name": "pets" }]
        });
        let v = TagDescription.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "tag-description");
    }

    #[test]
    fn triggers_when_description_empty() {
        let doc = json!({
            "openapi": "3.0.3",
            "tags": [{ "name": "pets", "description": "" }]
        });
        let v = TagDescription.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
    }

    #[test]
    fn passes_with_non_empty_description() {
        let doc = json!({
            "openapi": "3.0.3",
            "tags": [{ "name": "pets", "description": "Everything about pets" }]
        });
        assert!(TagDescription.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn no_tags_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(TagDescription.check(&doc, OasVersion::V3_0).is_empty());
    }
}
