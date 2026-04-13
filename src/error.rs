use thiserror::Error;

/// All errors that can occur during linting.
#[derive(Error, Debug)]
pub enum LintError {
    #[error("cannot read spec file: {0}")]
    Io(#[from] std::io::Error),

    #[error("cannot parse YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("cannot parse JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid OpenAPI document: {0}")]
    InvalidSpec(String),

    #[error("cannot load ruleset: {0}")]
    Ruleset(String),
}
