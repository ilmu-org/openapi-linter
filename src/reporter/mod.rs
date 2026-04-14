use std::io::Write;

use crate::model::{Severity, Violation};

const ANSI_RED: &str = "\x1b[31m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_BLUE: &str = "\x1b[34m";
const ANSI_RESET: &str = "\x1b[0m";

/// Write violations in human-readable text format.
///
/// Each violation is formatted as:
/// `{spec_path}:{path}  {severity}  {rule_id}  {message}`
///
/// ANSI colour codes are applied when `use_color` is `true`.
///
/// # Errors
///
/// Returns [`std::io::Error`] if writing to `out` fails.
pub fn write_text(
    violations: &[Violation],
    spec_path: &str,
    use_color: bool,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    for v in violations {
        let (prefix, suffix) = if use_color {
            let color = match v.severity {
                Severity::Error => ANSI_RED,
                Severity::Warn => ANSI_YELLOW,
                Severity::Info => ANSI_BLUE,
            };
            (color, ANSI_RESET)
        } else {
            ("", "")
        };

        let severity_label = match v.severity {
            Severity::Error => "error",
            Severity::Warn => "warn",
            Severity::Info => "info",
        };

        writeln!(
            out,
            "{}{}{}  {}{}{}  {}  {}",
            prefix, spec_path, suffix, prefix, v.path, suffix, severity_label, v.rule_id,
        )?;
        writeln!(out, "  {}", v.message)?;
    }
    Ok(())
}

/// Write violations as a JSON object containing a `violations` array and a `summary`.
///
/// # Errors
///
/// Returns [`std::io::Error`] if writing to `out` fails.
pub fn write_json(
    violations: &[Violation],
    spec_path: &str,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    let errors = violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .count();
    let warnings = violations
        .iter()
        .filter(|v| v.severity == Severity::Warn)
        .count();
    let total = violations.len();

    let json_violations: Vec<serde_json::Value> = violations
        .iter()
        .map(|v| {
            let severity_str = match v.severity {
                Severity::Error => "error",
                Severity::Warn => "warn",
                Severity::Info => "info",
            };
            serde_json::json!({
                "rule": v.rule_id,
                "severity": severity_str,
                "message": v.message,
                "path": v.path,
                "file": spec_path,
            })
        })
        .collect();

    let output = serde_json::json!({
        "violations": json_violations,
        "summary": {
            "errors": errors,
            "warnings": warnings,
            "total": total,
        }
    });

    serde_json::to_writer_pretty(&mut *out, &output).map_err(std::io::Error::other)?;
    writeln!(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_violation(severity: Severity) -> Violation {
        Violation {
            rule_id: "test-rule".to_string(),
            message: "Something is wrong.".to_string(),
            severity,
            path: "/paths/~1foo/get".to_string(),
        }
    }

    #[test]
    fn write_text_no_color() {
        let violations = vec![make_violation(Severity::Error)];
        let mut buf = Vec::new();
        write_text(&violations, "spec.yaml", false, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("spec.yaml"));
        assert!(output.contains("error"));
        assert!(output.contains("test-rule"));
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn write_text_with_color() {
        let violations = vec![make_violation(Severity::Warn)];
        let mut buf = Vec::new();
        write_text(&violations, "spec.yaml", true, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains(ANSI_YELLOW));
        assert!(output.contains(ANSI_RESET));
    }

    #[test]
    fn write_json_structure() {
        let violations = vec![
            make_violation(Severity::Error),
            make_violation(Severity::Warn),
        ];
        let mut buf = Vec::new();
        write_json(&violations, "spec.yaml", &mut buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed["summary"]["errors"], 1);
        assert_eq!(parsed["summary"]["warnings"], 1);
        assert_eq!(parsed["summary"]["total"], 2);
        assert_eq!(parsed["violations"][0]["file"], "spec.yaml");
    }

    #[test]
    fn write_json_empty() {
        let mut buf = Vec::new();
        write_json(&[], "spec.yaml", &mut buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed["summary"]["total"], 0);
        assert!(parsed["violations"].as_array().unwrap().is_empty());
    }
}
