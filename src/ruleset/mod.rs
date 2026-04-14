use std::collections::HashMap;
use std::path::Path;

use crate::error::LintError;
use crate::model::Severity;

/// Configuration loaded from a `.spectral.yaml` ruleset file.
///
/// Only severity overrides for built-in rules are supported in v0.1.0.
/// `None` means the rule is disabled (`off`); `Some(s)` overrides the severity.
#[derive(Debug)]
pub struct RulesetConfig {
    pub severity_overrides: HashMap<String, Option<Severity>>,
}

impl RulesetConfig {
    fn empty() -> Self {
        Self {
            severity_overrides: HashMap::new(),
        }
    }
}

/// Load a Spectral-compatible ruleset YAML file.
///
/// Supported fields:
/// - `extends`: must be `["spectral:oas"]` or `"spectral:oas"`.
/// - Per-rule severity keys: `off` / `warn` / `error` / `info`.
///
/// # Errors
///
/// - [`LintError::Io`] — file cannot be read.
/// - [`LintError::Yaml`] — YAML parse failure.
/// - [`LintError::Ruleset`] — unsupported `extends` value or custom rule definition.
pub fn load(path: &Path) -> Result<RulesetConfig, LintError> {
    let content = std::fs::read_to_string(path).map_err(LintError::Io)?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&content)?;

    let Some(mapping) = doc.as_mapping() else {
        return Ok(RulesetConfig::empty());
    };

    let mut config = RulesetConfig::empty();

    for (key, value) in mapping {
        let Some(key_str) = key.as_str() else {
            continue;
        };

        if key_str == "extends" {
            validate_extends(value)?;
        } else {
            // Reject custom rule definitions (objects with "given" or "then").
            if let Some(obj) = value.as_mapping() {
                if obj.contains_key("given") || obj.contains_key("then") {
                    return Err(LintError::Ruleset(
                        "Custom rule definitions are not supported in v0.1.0. \
                         Only severity overrides for built-in rules are supported."
                            .into(),
                    ));
                }
                // Unknown object key — ignore silently.
                continue;
            }

            // Try to parse as a severity string override.
            if let Some(severity) = parse_severity_value(value) {
                config
                    .severity_overrides
                    .insert(key_str.to_string(), severity);
            }
            // Unknown string/number keys — ignore silently.
        }
    }

    Ok(config)
}

fn validate_extends(value: &serde_yaml::Value) -> Result<(), LintError> {
    let is_valid = match value {
        serde_yaml::Value::String(s) => s == "spectral:oas",
        serde_yaml::Value::Sequence(seq) => {
            seq.len() == 1 && seq[0].as_str() == Some("spectral:oas")
        }
        _ => false,
    };

    if !is_valid {
        return Err(LintError::Ruleset(
            "only spectral:oas is supported in v0.1.0".into(),
        ));
    }

    Ok(())
}

/// Parse a YAML value as a severity override. Returns `None` if not a recognized string.
// Option<Option<T>> is intentional: outer None = unrecognised key, inner None = "off" (disabled).
#[allow(clippy::option_option)]
fn parse_severity_value(value: &serde_yaml::Value) -> Option<Option<Severity>> {
    match value.as_str()? {
        "off" => Some(None),
        "error" => Some(Some(Severity::Error)),
        "warn" => Some(Some(Severity::Warn)),
        "info" => Some(Some(Severity::Info)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_temp_yaml(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::Builder::new().suffix(".yaml").tempfile().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn empty_ruleset_returns_empty_config() {
        let f = write_temp_yaml("extends: \"spectral:oas\"\n");
        let config = load(f.path()).unwrap();
        assert!(config.severity_overrides.is_empty());
    }

    #[test]
    fn severity_override_off() {
        let f = write_temp_yaml("extends: \"spectral:oas\"\noperation-operationId: off\n");
        let config = load(f.path()).unwrap();
        assert_eq!(
            config.severity_overrides.get("operation-operationId"),
            Some(&None)
        );
    }

    #[test]
    fn severity_override_warn() {
        let f = write_temp_yaml("extends: \"spectral:oas\"\noperation-summary: warn\n");
        let config = load(f.path()).unwrap();
        assert_eq!(
            config.severity_overrides.get("operation-summary"),
            Some(&Some(Severity::Warn))
        );
    }

    #[test]
    fn unsupported_extends_returns_error() {
        let f = write_temp_yaml("extends: \"spectral:asyncapi\"\n");
        let err = load(f.path()).unwrap_err();
        assert!(matches!(err, LintError::Ruleset(_)));
    }

    #[test]
    fn custom_rule_definition_returns_error() {
        let f = write_temp_yaml(
            r#"
extends: "spectral:oas"
my-custom-rule:
  given: "$.paths"
  then:
    function: truthy
"#,
        );
        let err = load(f.path()).unwrap_err();
        assert!(matches!(err, LintError::Ruleset(_)));
    }

    #[test]
    fn extends_as_array_is_valid() {
        let f = write_temp_yaml("extends:\n  - \"spectral:oas\"\n");
        let config = load(f.path()).unwrap();
        assert!(config.severity_overrides.is_empty());
    }
}
