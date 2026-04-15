//! Data model types shared across the linter.

/// Violation types — severity and the violation record itself.
pub mod violation;

pub use violation::{Severity, Violation};

/// The `OpenAPI` specification version detected in a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OasVersion {
    /// OpenAPI / Swagger 2.x.
    V2,
    /// OpenAPI 3.0.x.
    V3_0,
    /// OpenAPI 3.1.x.
    V3_1,
    /// OpenAPI version could not be determined from the document.
    Unknown,
}

impl OasVersion {
    /// Detect the `OpenAPI` version from a parsed document.
    ///
    /// Returns [`OasVersion::Unknown`] when the version string is absent or unrecognized.
    /// Callers should emit a diagnostic when `Unknown` is returned.
    #[must_use]
    pub fn detect(doc: &serde_json::Value) -> OasVersion {
        if let Some(swagger) = doc["swagger"].as_str()
            && swagger.starts_with('2')
        {
            return OasVersion::V2;
        }

        if let Some(openapi) = doc["openapi"].as_str() {
            if openapi.starts_with("3.0") {
                return OasVersion::V3_0;
            }
            if openapi.starts_with("3.1") {
                return OasVersion::V3_1;
            }
        }

        OasVersion::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detect_swagger_2() {
        let doc = json!({ "swagger": "2.0" });
        assert_eq!(OasVersion::detect(&doc), OasVersion::V2);
    }

    #[test]
    fn detect_openapi_3_0() {
        let doc = json!({ "openapi": "3.0.3" });
        assert_eq!(OasVersion::detect(&doc), OasVersion::V3_0);
    }

    #[test]
    fn detect_openapi_3_1() {
        let doc = json!({ "openapi": "3.1.0" });
        assert_eq!(OasVersion::detect(&doc), OasVersion::V3_1);
    }

    #[test]
    fn detect_unknown_returns_unknown() {
        let doc = json!({ "foo": "bar" });
        assert_eq!(OasVersion::detect(&doc), OasVersion::Unknown);
    }
}
