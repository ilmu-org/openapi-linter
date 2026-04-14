use std::path::Path;

use crate::error::LintError;

/// Parse an `OpenAPI` spec file into a JSON value.
///
/// Accepts `.yaml`, `.yml`, and `.json` files. For unknown extensions YAML is
/// attempted first, then JSON.
///
/// Parse errors include the file path and, for YAML files, the line/column
/// from the parser so CI output is immediately actionable.
///
/// # Errors
///
/// - [`LintError::Io`] — file cannot be read.
/// - [`LintError::InvalidSpec`] — YAML or JSON parse failure (includes path + location).
pub fn parse(path: &Path) -> Result<serde_json::Value, LintError> {
    let content = std::fs::read_to_string(path).map_err(LintError::Io)?;

    let result = match path.extension().and_then(|e| e.to_str()) {
        Some("yaml" | "yml") => parse_yaml(&content),
        Some("json") => parse_json(&content, path),
        _ => parse_yaml(&content).or_else(|_| parse_json(&content, path)),
    };

    result.map_err(|e| match e {
        LintError::Yaml(yaml_err) => {
            let location = yaml_err.location().map_or_else(
                || path.display().to_string(),
                |loc| format!("{}:{}:{}", path.display(), loc.line(), loc.column()),
            );
            LintError::InvalidSpec(format!("{location}: {yaml_err}"))
        }
        other => other,
    })
}

fn parse_yaml(content: &str) -> Result<serde_json::Value, LintError> {
    // Deserialize to serde_yaml::Value first, then normalise to serde_json::Value.
    let yaml_val: serde_yaml::Value = serde_yaml::from_str(content)?;
    let json_val = serde_json::to_value(yaml_val)?;
    Ok(json_val)
}

fn parse_json(content: &str, path: &Path) -> Result<serde_json::Value, LintError> {
    serde_json::from_str(content)
        .map_err(|e| LintError::InvalidSpec(format!("{}: {e}", path.display())))
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

    #[test]
    fn yaml_syntax_error_includes_path_in_message() {
        let f = write_temp("key: :\n  bad", "yaml");
        let err = parse(f.path()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(f.path().to_str().unwrap()),
            "error message should contain file path, got: {msg}"
        );
    }

    #[test]
    fn json_syntax_error_includes_path_in_message() {
        let f = write_temp("{bad json", "json");
        let err = parse(f.path()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(f.path().to_str().unwrap()),
            "error message should contain file path, got: {msg}"
        );
    }
}
