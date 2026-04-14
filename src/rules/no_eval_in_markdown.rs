use crate::model::{OasVersion, Severity, Violation};
use crate::rules::{Rule, util};

/// No `eval(` string must appear in any markdown `description` or `summary` field.
pub struct NoEvalInMarkdown;

impl Rule for NoEvalInMarkdown {
    fn id(&self) -> &'static str {
        "no-eval-in-markdown"
    }

    fn message(&self) -> &'static str {
        "Markdown fields must not contain eval() — potential XSS risk."
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &serde_json::Value, _version: OasVersion) -> Vec<Violation> {
        let mut fields = Vec::new();
        util::walk_markdown_fields(doc, "", &mut fields);

        fields
            .into_iter()
            .filter(|(_, s)| s.contains("eval("))
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
    fn triggers_on_eval_in_description() {
        let doc = json!({ "info": { "description": "Call eval(x) here." } });
        assert!(!NoEvalInMarkdown.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn triggers_on_eval_in_summary() {
        let doc = json!({ "paths": { "/foo": { "get": { "summary": "eval(bad)" } } } });
        assert!(!NoEvalInMarkdown.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn passes_when_no_eval() {
        let doc = json!({ "info": { "description": "A safe description." } });
        assert!(NoEvalInMarkdown.check(&doc, OasVersion::V3_0).is_empty());
    }
}
