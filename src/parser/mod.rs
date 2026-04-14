use std::path::Path;

use crate::error::LintError;

/// Parse an `OpenAPI` spec file into a JSON value.
///
/// Accepts `.yaml`, `.yml`, and `.json` files. For unknown extensions YAML is
/// attempted first, then JSON.
///
/// # Errors
///
/// - [`LintError::Io`] — file cannot be read.
/// - [`LintError::Yaml`] — YAML parse failure.
/// - [`LintError::Json`] — JSON parse failure.
pub fn parse(path: &Path) -> Result<serde_json::Value, LintError> {
    let content = std::fs::read_to_string(path).map_err(LintError::Io)?;

    match path.extension().and_then(|e| e.to_str()) {
        Some("yaml" | "yml") => parse_yaml(&content),
        Some("json") => parse_json(&content),
        _ => parse_yaml(&content).or_else(|_| parse_json(&content)),
    }
}

fn parse_yaml(content: &str) -> Result<serde_json::Value, LintError> {
    // Deserialize to serde_yaml::Value first, then normalise to serde_json::Value.
    let yaml_val: serde_yaml::Value = serde_yaml::from_str(content)?;
    let json_val = serde_json::to_value(yaml_val)?;
    Ok(json_val)
}

fn parse_json(content: &str) -> Result<serde_json::Value, LintError> {
    let val: serde_json::Value = serde_json::from_str(content)?;
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_temp(content: &str, ext: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::Builder::new()
            .suffix(&format!(".{ext}"))
            .tempfile()
            .unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parse_yaml_file() {
        let f = write_temp(
            "openapi: \"3.0.3\"\ninfo:\n  title: Test\n  version: \"1\"\n",
            "yaml",
        );
        let val = parse(f.path()).unwrap();
        assert_eq!(val["openapi"], "3.0.3");
    }

    #[test]
    fn parse_json_file() {
        let f = write_temp(r#"{"openapi":"3.0.3"}"#, "json");
        let val = parse(f.path()).unwrap();
        assert_eq!(val["openapi"], "3.0.3");
    }

    #[test]
    fn missing_file_returns_io_error() {
        let result = parse(Path::new("/nonexistent/file.yaml"));
        assert!(matches!(result, Err(LintError::Io(_))));
    }
}
