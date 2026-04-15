use std::collections::HashMap;

use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// Top-level tag names must be unique.
///
/// Applies to OAS 2.x and 3.x.
pub struct OpenApiTagsUniqueness;

impl Rule for OpenApiTagsUniqueness {
    fn id(&self) -> &'static str {
        "openapi-tags-uniqueness"
    }

    fn message(&self) -> &'static str {
        "Tag names must be unique."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let Some(tags) = doc["tags"].as_array() else {
            return vec![];
        };

        let mut seen: HashMap<&str, usize> = HashMap::new();
        let mut violations = Vec::new();

        for (index, tag) in tags.iter().enumerate() {
            let Some(name) = tag["name"].as_str() else {
                continue;
            };
            if let Some(&first_index) = seen.get(name) {
                let _ = first_index;
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    message: format!("Tag name '{name}' is not unique."),
                    severity: self.default_severity(),
                    path: format!("/tags/{index}"),
                    line: None,
                    col: None,
                });
            } else {
                seen.insert(name, index);
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
    fn triggers_on_duplicate_tag_name() {
        let doc = json!({
            "openapi": "3.0.3",
            "tags": [
                { "name": "pets", "description": "Pet ops" },
                { "name": "pets", "description": "Duplicate" }
            ]
        });
        let v = OpenApiTagsUniqueness.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "openapi-tags-uniqueness");
        assert!(v[0].path.contains("/tags/1"));
    }

    #[test]
    fn passes_with_unique_tags() {
        let doc = json!({
            "openapi": "3.0.3",
            "tags": [
                { "name": "pets" },
                { "name": "store" }
            ]
        });
        assert!(
            OpenApiTagsUniqueness
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn no_tags_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(
            OpenApiTagsUniqueness
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }
}
