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
}
