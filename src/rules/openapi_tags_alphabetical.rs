use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// The top-level `tags` array must be sorted alphabetically by `name`.
pub struct OpenApiTagsAlphabetical;

impl Rule for OpenApiTagsAlphabetical {
    fn id(&self) -> &'static str {
        "openapi-tags-alphabetical"
    }

    fn message(&self) -> &'static str {
        "Top-level tags array must be sorted alphabetically by name."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let Some(tags) = doc["tags"].as_array() else {
            return vec![];
        };
        if tags.len() < 2 {
            return vec![];
        }

        let names: Vec<&str> = tags.iter().filter_map(|t| t["name"].as_str()).collect();

        let sorted = {
            let mut s = names.clone();
            s.sort_unstable();
            s
        };

        if names == sorted {
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
    fn passes_when_sorted() {
        let doc = parse_yaml("tags:\n  - name: alpha\n  - name: beta\n  - name: gamma\n");
        assert!(
            OpenApiTagsAlphabetical
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn triggers_when_unsorted() {
        let doc = parse_yaml("tags:\n  - name: zebra\n  - name: alpha\n");
        assert!(
            !OpenApiTagsAlphabetical
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn passes_single_tag() {
        let doc = parse_yaml("tags:\n  - name: only\n");
        assert!(
            OpenApiTagsAlphabetical
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn passes_no_tags() {
        let doc = parse_yaml("openapi: \"3.0.3\"\n");
        assert!(
            OpenApiTagsAlphabetical
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }
}
