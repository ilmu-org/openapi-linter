use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{Rule, util};

/// No `<script` tag must appear in any markdown `description` or `summary` field.
pub struct NoScriptTagsInMarkdown;

impl Rule for NoScriptTagsInMarkdown {
    fn id(&self) -> &'static str {
        "no-script-tags-in-markdown"
    }

    fn message(&self) -> &'static str {
        "Markdown fields must not contain <script tags — potential XSS risk."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let mut fields = Vec::new();
        util::walk_markdown_fields(doc, "", &mut fields);

        fields
            .into_iter()
            .filter(|(_, s)| s.contains("<script"))
            .map(|(path, _)| Violation {
                rule_id: self.id().to_string(),
                message: self.message().to_string(),
                severity: self.default_severity(),
                path,
                line: None,
                col: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_on_script_tag_in_description() {
        let doc = json!({ "info": { "description": "See <script>alert(1)</script>" } });
        assert!(
            !NoScriptTagsInMarkdown
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn passes_when_no_script_tag() {
        let doc = json!({ "info": { "description": "A safe description." } });
        assert!(
            NoScriptTagsInMarkdown
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn triggers_on_script_in_summary() {
        let doc = json!({ "paths": { "/x": { "get": { "summary": "<script src='x.js'>" } } } });
        assert!(
            !NoScriptTagsInMarkdown
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }
}
