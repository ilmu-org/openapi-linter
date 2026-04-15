use crate::model::{OasVersion, Severity, Violation};
use crate::rules::Rule;

/// Server URLs must not use `example.com` as the host.
///
/// Applies to OAS 3.x only. Skipped for OAS 2.x and unknown versions.
pub struct Oas3ServerNotExampleCom;

impl Rule for Oas3ServerNotExampleCom {
    fn id(&self) -> &'static str {
        "oas3-server-not-example.com"
    }

    fn message(&self) -> &'static str {
        "Server URL must not use example.com as the host."
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
            if host_is_example_com(url) {
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

/// Returns true if the host portion of `url` is `example.com` (ignoring port).
fn host_is_example_com(url: &str) -> bool {
    // Extract host: find "://" then take the segment before the next "/" or end.
    let after_scheme = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };

    // Take everything up to the next "/" (path separator).
    let host_and_port = match after_scheme.find('/') {
        Some(pos) => &after_scheme[..pos],
        None => after_scheme,
    };

    // Strip port if present.
    let host = match host_and_port.find(':') {
        Some(pos) => &host_and_port[..pos],
        None => host_and_port,
    };

    host.eq_ignore_ascii_case("example.com")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn triggers_on_example_com_host() {
        let doc = json!({
            "openapi": "3.0.3",
            "servers": [{ "url": "https://example.com/v1" }]
        });
        let v = Oas3ServerNotExampleCom.check(&doc, OasVersion::V3_0);
        assert!(!v.is_empty());
        assert_eq!(v[0].rule_id, "oas3-server-not-example.com");
    }

    #[test]
    fn passes_with_real_host() {
        let doc = json!({
            "openapi": "3.0.3",
            "servers": [{ "url": "https://api.myservice.com/v1" }]
        });
        assert!(
            Oas3ServerNotExampleCom
                .check(&doc, OasVersion::V3_0)
                .is_empty()
        );
    }

    #[test]
    fn skipped_for_oas2() {
        let doc = json!({
            "swagger": "2.0",
            "servers": [{ "url": "https://example.com/v1" }]
        });
        assert!(
            Oas3ServerNotExampleCom
                .check(&doc, OasVersion::V2)
                .is_empty()
        );
    }

    #[test]
    fn host_extraction_with_port() {
        assert!(host_is_example_com("https://example.com:8080/v1"));
        assert!(!host_is_example_com("https://api.example.com/v1"));
    }
}
