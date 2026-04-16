//! Bundled OAS JSON Schema constants and lazy-init accessors.
//!
//! Each OAS version's JSON Schema is embedded at compile time via `include_str!()` and
//! parsed into a `serde_json::Value` on first use via `std::sync::OnceLock`. This
//! avoids runtime file I/O and keeps schema loading off the hot path after the first call.

use std::sync::OnceLock;

use serde_json::Value;

static OAS2_SCHEMA: OnceLock<Value> = OnceLock::new();
static OAS3_0_SCHEMA: OnceLock<Value> = OnceLock::new();
static OAS3_1_SCHEMA: OnceLock<Value> = OnceLock::new();

/// Canonical URI used when registering the OAS 2.0 JSON Schema with boon.
pub(crate) const OAS2_SCHEMA_URI: &str = "https://refract-linter.internal/schemas/oas2.0.json";
/// Canonical URI used when registering the OAS 3.0 JSON Schema with boon.
pub(crate) const OAS3_0_SCHEMA_URI: &str = "https://refract-linter.internal/schemas/oas3.0.json";
/// Canonical URI used when registering the OAS 3.1 JSON Schema with boon.
pub(crate) const OAS3_1_SCHEMA_URI: &str = "https://refract-linter.internal/schemas/oas3.1.json";

/// Return a reference to the parsed OAS 2.0 JSON Schema.
///
/// Panics on first call only if the bundled schema bytes are invalid JSON, which
/// would be a packaging bug caught at compile/test time.
#[must_use]
pub(crate) fn oas2_schema() -> &'static Value {
    OAS2_SCHEMA.get_or_init(|| {
        let raw = include_str!("../assets/schemas/oas2.0.json");
        serde_json::from_str(raw).expect("bundled OAS 2.0 schema must be valid JSON")
    })
}

/// Return a reference to the parsed OAS 3.0 JSON Schema.
#[must_use]
pub(crate) fn oas3_0_schema() -> &'static Value {
    OAS3_0_SCHEMA.get_or_init(|| {
        let raw = include_str!("../assets/schemas/oas3.0.json");
        serde_json::from_str(raw).expect("bundled OAS 3.0 schema must be valid JSON")
    })
}

/// Return a reference to the parsed OAS 3.1 JSON Schema.
#[must_use]
pub(crate) fn oas3_1_schema() -> &'static Value {
    OAS3_1_SCHEMA.get_or_init(|| {
        let raw = include_str!("../assets/schemas/oas3.1.json");
        serde_json::from_str(raw).expect("bundled OAS 3.1 schema must be valid JSON")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oas2_schema_loads_and_is_object() {
        let s = oas2_schema();
        assert!(s.is_object(), "OAS 2.0 schema must be a JSON object");
    }

    #[test]
    fn oas3_0_schema_loads_and_is_object() {
        let s = oas3_0_schema();
        assert!(s.is_object(), "OAS 3.0 schema must be a JSON object");
    }

    #[test]
    fn oas3_1_schema_loads_and_is_object() {
        let s = oas3_1_schema();
        assert!(s.is_object(), "OAS 3.1 schema must be a JSON object");
    }
}
