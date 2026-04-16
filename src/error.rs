use thiserror::Error;

/// All errors that can occur during linting.
#[derive(Error, Debug)]
pub enum LintError {
    /// The spec file could not be read from disk.
    #[error("cannot read spec file: {0}")]
    Io(#[from] std::io::Error),

    /// The file content is not valid YAML.
    #[error("cannot parse YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// The file content is not valid JSON.
    #[error("cannot parse JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// The document is valid YAML/JSON but not a recognised OpenAPI spec.
    #[error("invalid OpenAPI document: {0}")]
    InvalidSpec(String),

    /// The `.spectral.yaml` ruleset file is invalid or unsupported.
    #[error("cannot load ruleset: {0}")]
    Ruleset(String),

    /// An external `$ref` could not be resolved (file not found, malformed, or pointer missing).
    #[error("unresolvable $ref '{ref_str}' in {path}")]
    UnresolvableRef {
        /// Path to the file that contained the unresolvable ref.
        path: std::path::PathBuf,
        /// The raw ref string that could not be resolved.
        ref_str: String,
    },

    /// A `$ref` cycle was detected during cross-file resolution.
    #[error("$ref cycle detected involving {path}")]
    RefCycle {
        /// The file at which the cycle was detected.
        path: std::path::PathBuf,
    },

    /// An HTTP(S) `$ref` was encountered; refract does not support network refs.
    #[error("HTTP $refs are not supported: {ref_str}")]
    HttpRefNotSupported {
        /// The HTTP ref string.
        ref_str: String,
    },

    /// Cross-file `$ref` resolution exceeded the maximum recursion depth (64 steps).
    #[error("$ref resolution depth limit (64) exceeded")]
    RefDepthExceeded,
}
