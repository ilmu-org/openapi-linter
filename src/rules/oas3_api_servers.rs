use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// OAS 3.x documents must define a non-empty `servers` array.
///
/// Applies to OAS 3.x only. Skipped for OAS 2.x and unknown versions.
pub struct Oas3ApiServers;

impl Rule for Oas3ApiServers {
    fn id(&self) -> &'static str {
        "oas3-api-servers"
    }

    fn message(&self) -> &'static str {
        "OpenAPI 3.x document must define a non-empty servers array."
    }

    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, doc: &serde_json::Value, version: OasVersion) -> Vec<Violation> {
        if !matches!(version, OasVersion::V3_0 | OasVersion::V3_1) {
            return vec![];
        }

        let has_servers = doc["servers"].as_array().is_some_and(|a| !a.is_empty());
        if has_servers {
            return vec![];
        }

        vec![Violation {
            rule_id: self.id().to_string(),
            message: self.message().to_string(),
            severity: self.default_severity(),
            path: "/servers".to_string(),
            line: None,
            col: None,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_when_servers_absent() {
        let doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Test", "version": "1.0.0" },
            "paths": {}
        });
        let v = Oas3ApiServers.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "oas3-api-servers");
    }

    #[test]
    fn triggers_when_servers_empty() {
        let doc = json!({
            "openapi": "3.0.3",
            "servers": []
        });
        let v = Oas3ApiServers.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
    }

    #[test]
    fn passes_with_non_empty_servers() {
        let doc = json!({
            "openapi": "3.0.3",
            "servers": [{ "url": "https://api.example.com" }]
        });
        assert!(Oas3ApiServers.check(&doc, OasVersion::V3_0).is_empty());
    }

    #[test]
    fn skipped_for_oas2() {
        let doc = json!({ "swagger": "2.0" });
        assert!(Oas3ApiServers.check(&doc, OasVersion::V2).is_empty());
    }
}
