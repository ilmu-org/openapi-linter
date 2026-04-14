//! Data model types shared across the linter.

/// Violation types — severity and the violation record itself.
pub mod violation;

pub use violation::{Severity, Violation};

use crate::error::LintError;

/// The `OpenAPI` specification version detected in a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OasVersion {
    /// OpenAPI / Swagger 2.x.
    V2,
    /// OpenAPI 3.0.x.
    V3_0,
    /// OpenAPI 3.1.x.
    V3_1,
}

impl OasVersion {
    /// Detect the `OpenAPI` version from a parsed document.
    ///
    /// # Errors
    ///
    /// Returns [`LintError::InvalidSpec`] if the version cannot be determined.
    pub fn detect(doc: &serde_json::Value) -> Result<OasVersion, LintError> {
        if let Some(swagger) = doc["swagger"].as_str()
            && swagger.starts_with('2')
        {
            return Ok(OasVersion::V2);
        }

        if let Some(openapi) = doc["openapi"].as_str() {
            if openapi.starts_with("3.0") {
                return Ok(OasVersion::V3_0);
            }
            if openapi.starts_with("3.1") {
                return Ok(OasVersion::V3_1);
            }
        }

        Err(LintError::InvalidSpec(
            "cannot determine OpenAPI version".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detect_swagger_2() {
        let doc = json!({ "swagger": "2.0" });
        assert_eq!(OasVersion::detect(&doc).unwrap(), OasVersion::V2);
    }

    #[test]
    fn detect_openapi_3_0() {
        let doc = json!({ "openapi": "3.0.3" });
        assert_eq!(OasVersion::detect(&doc).unwrap(), OasVersion::V3_0);
    }

    #[test]
    fn detect_openapi_3_1() {
        let doc = json!({ "openapi": "3.1.0" });
        assert_eq!(OasVersion::detect(&doc).unwrap(), OasVersion::V3_1);
    }

    #[test]
    fn detect_unknown_returns_error() {
        let doc = json!({ "foo": "bar" });
        assert!(OasVersion::detect(&doc).is_err());
    }
}
