use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// Server URLs must not end with a trailing slash.
///
/// Applies to OAS 3.x only. Skipped for OAS 2.x and unknown versions.
pub struct Oas3ServerTrailingSlash;

impl Rule for Oas3ServerTrailingSlash {
    fn id(&self) -> &'static str {
        "oas3-server-trailing-slash"
    }

    fn message(&self) -> &'static str {
        "Server URL must not end with a trailing slash."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, version: OasVersion) -> Vec<Violation> {
        if !matches!(version, OasVersion::V3_0 | OasVersion::V3_1) {
            return vec![];
        }

        let Some(servers) = doc["servers"].as_array() else {
            return vec![];
        };

        let mut violations = Vec::new();

        for (index, server) in servers.iter().enumerate() {
            let Some(url) = server["url"].as_str() else {
                continue;
            };
            if url.ends_with('/') {
                violations.push(Violation {
                    rule_id: self.id().to_string(),
                    message: self.message().to_string(),
                    severity: self.default_severity(),
                    path: format!("/servers/{index}/url"),
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
    fn triggers_on_trailing_slash_in_url() {
        let doc = json!({
            "openapi": "3.0.3",
            "servers": [{ "url": "https://api.example.com/v1/" }]
        });
        let v = Oas3ServerTrailingSlash.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "oas3-server-trailing-slash");
    }

    #[test]
    fn passes_without_trailing_slash() {
        let doc = json!({
            "openapi": "3.0.3",
            "servers": [{ "url": "https://api.example.com/v1" }]
        });
        assert!(
            Oas3ServerTrailingSlash
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn skipped_for_oas2() {
        let doc = json!({
            "swagger": "2.0",
            "servers": [{ "url": "https://api.example.com/v1/" }]
        });
        assert!(
            Oas3ServerTrailingSlash
                .check(&doc, OasVersion::V2)
                .is_empty()
        );
    }

    #[test]
    fn no_servers_returns_empty() {
        let doc = json!({ "openapi": "3.0.3" });
        assert!(
            Oas3ServerTrailingSlash
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }
}
