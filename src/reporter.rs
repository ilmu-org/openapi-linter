use std::io::Write;
use std::path::PathBuf;

use crate::model::{Severity, Violation};

const ANSI_RED: &str = "\x1b[31m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_BLUE: &str = "\x1b[34m";
const ANSI_RESET: &str = "\x1b[0m";

/// Controls whether ANSI colour codes appear in text output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Enable colour only when stdout is a terminal.
    Auto,
    /// Always enable colour.
    Always,
    /// Never enable colour.
    Never,
}

/// Output format for lint results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Human-readable text with one violation per line.
    Text,
    /// Structured JSON (pre-1.0 schema, may change).
    Json,
    /// SARIF 2.1.0 for IDE and CI integrations.
    Sarif,
}

/// Write lint results for one or more files to `out`.
///
/// `files` is a slice of `(path, violations)` pairs — one entry per spec file
/// that was linted. The format and colour mode are applied uniformly across all
/// files.
///
/// # Errors
///
/// Returns [`std::io::Error`] if writing to `out` fails.
pub fn report(
    files: &[(PathBuf, Vec<Violation>)],
    format: Format,
    color: ColorMode,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    match format {
        Format::Text => write_text_batch(files, color, out),
        Format::Json => write_json_batch(files, out),
        Format::Sarif => write_sarif_batch(files, out),
    }
}

fn resolve_color(color: ColorMode) -> bool {
    // Auto cannot inspect the `dyn Write` trait object to detect a terminal;
    // callers (main.rs) resolve Auto to Always/Never before calling report().
    matches!(color, ColorMode::Always)
}

fn write_text_batch(
    files: &[(PathBuf, Vec<Violation>)],
    color: ColorMode,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    let use_color = resolve_color(color);
    for (path, violations) in files {
        let spec_path = path.display().to_string();
        for v in violations {
            let (prefix, suffix) = if use_color {
                let c = match v.severity {
                    Severity::Error => ANSI_RED,
                    Severity::Warn => ANSI_YELLOW,
                    Severity::Info => ANSI_BLUE,
                };
                (c, ANSI_RESET)
            } else {
                ("", "")
            };

            let severity_label = match v.severity {
                Severity::Error => "error",
                Severity::Warn => "warn",
                Severity::Info => "info",
            };

            let location = match (v.line, v.col) {
                (Some(l), Some(c)) => format!("{spec_path}:{l}:{c}"),
                _ => spec_path.clone(),
            };

            writeln!(
                out,
                "{}{}{}  {}{}{}  {}  {}",
                prefix, location, suffix, prefix, v.path, suffix, severity_label, v.rule_id,
            )?;
            writeln!(out, "  {}", v.message)?;
        }
    }
    Ok(())
}

fn write_json_batch(
    files: &[(PathBuf, Vec<Violation>)],
    out: &mut dyn Write,
) -> std::io::Result<()> {
    let mut total_errors: usize = 0;
    let mut total_warnings: usize = 0;
    let mut total: usize = 0;

    let json_files: Vec<serde_json::Value> = files
        .iter()
        .map(|(path, violations)| {
            let file_str = path.display().to_string();
            let json_violations: Vec<serde_json::Value> = violations
                .iter()
                .map(|v| {
                    let severity_str = match v.severity {
                        Severity::Error => "error",
                        Severity::Warn => "warn",
                        Severity::Info => "info",
                    };
                    total += 1;
                    match v.severity {
                        Severity::Error => total_errors += 1,
                        Severity::Warn => total_warnings += 1,
                        Severity::Info => {}
                    }
                    let mut obj = serde_json::json!({
                        "rule": v.rule_id,
                        "severity": severity_str,
                        "message": v.message,
                        "path": v.path,
                    });
                    if let Some(line) = v.line {
                        obj["line"] = serde_json::json!(line);
                    }
                    if let Some(col) = v.col {
                        obj["col"] = serde_json::json!(col);
                    }
                    obj
                })
                .collect();
            serde_json::json!({
                "file": file_str,
                "violations": json_violations,
            })
        })
        .collect();

    let output = serde_json::json!({
        "files": json_files,
        "summary": {
            "errors": total_errors,
            "warnings": total_warnings,
            "total": total,
        }
    });

    serde_json::to_writer_pretty(&mut *out, &output).map_err(std::io::Error::other)?;
    writeln!(out)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SARIF 2.1.0
// ---------------------------------------------------------------------------

fn sarif_level(severity: &crate::model::Severity) -> &'static str {
    match severity {
        crate::model::Severity::Error => "error",
        crate::model::Severity::Warn => "warning",
        crate::model::Severity::Info => "note",
    }
}

fn write_sarif_batch(
    files: &[(PathBuf, Vec<Violation>)],
    out: &mut dyn Write,
) -> std::io::Result<()> {
    // Collect all unique rule IDs for the rules array.
    let mut rule_ids: Vec<&str> = files
        .iter()
        .flat_map(|(_, vs)| vs.iter().map(|v| v.rule_id.as_str()))
        .collect();
    rule_ids.sort_unstable();
    rule_ids.dedup();

    let rules: Vec<serde_json::Value> = rule_ids
        .iter()
        .map(|id| {
            serde_json::json!({
                "id": id,
            })
        })
        .collect();

    // Build results array — one entry per violation across all files.
    let results: Vec<serde_json::Value> = files
        .iter()
        .flat_map(|(path, violations)| {
            let uri = path.display().to_string();
            violations.iter().map(move |v| {
                let mut result = serde_json::json!({
                    "ruleId": v.rule_id,
                    "level": sarif_level(&v.severity),
                    "message": { "text": v.message },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": { "uri": uri },
                        }
                    }]
                });

                // Add region only when line/col are available.
                if let (Some(line), Some(col)) = (v.line, v.col) {
                    result["locations"][0]["physicalLocation"]["region"] = serde_json::json!({
                        "startLine": line,
                        "startColumn": col,
                    });
                }

                result
            })
        })
        .collect();

    let sarif = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "openapi-linter",
                    "rules": rules,
                }
            },
            "results": results,
        }]
    });

    serde_json::to_writer_pretty(&mut *out, &sarif).map_err(std::io::Error::other)?;
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
            line: None,
            col: None,
        }
    }

    fn single_file(path: &str, violations: Vec<Violation>) -> Vec<(PathBuf, Vec<Violation>)> {
        vec![(PathBuf::from(path), violations)]
    }

    #[test]
    fn text_no_color() {
        let files = single_file("spec.yaml", vec![make_violation(Severity::Error)]);
        let mut buf = Vec::new();
        report(&files, Format::Text, ColorMode::Never, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("spec.yaml"));
        assert!(output.contains("error"));
        assert!(output.contains("test-rule"));
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn text_with_color() {
        let files = single_file("spec.yaml", vec![make_violation(Severity::Warn)]);
        let mut buf = Vec::new();
        report(&files, Format::Text, ColorMode::Always, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains(ANSI_YELLOW));
        assert!(output.contains(ANSI_RESET));
    }

    #[test]
    fn json_structure() {
        let files = single_file(
            "spec.yaml",
            vec![
                make_violation(Severity::Error),
                make_violation(Severity::Warn),
            ],
        );
        let mut buf = Vec::new();
        report(&files, Format::Json, ColorMode::Never, &mut buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed["summary"]["errors"], 1);
        assert_eq!(parsed["summary"]["warnings"], 1);
        assert_eq!(parsed["summary"]["total"], 2);
        assert_eq!(parsed["files"][0]["file"], "spec.yaml");
        assert_eq!(parsed["files"][0]["violations"][0]["rule"], "test-rule");
    }

    #[test]
    fn json_empty() {
        let files = single_file("spec.yaml", vec![]);
        let mut buf = Vec::new();
        report(&files, Format::Json, ColorMode::Never, &mut buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed["summary"]["total"], 0);
        assert!(
            parsed["files"][0]["violations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn sarif_structure() {
        let mut v = make_violation(Severity::Error);
        v.line = Some(10);
        v.col = Some(3);
        let files = single_file("spec.yaml", vec![v]);
        let mut buf = Vec::new();
        report(&files, Format::Sarif, ColorMode::Never, &mut buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed["version"], "2.1.0");
        let results = &parsed["runs"][0]["results"];
        assert_eq!(results[0]["ruleId"], "test-rule");
        assert_eq!(results[0]["level"], "error");
        let region = &results[0]["locations"][0]["physicalLocation"]["region"];
        assert_eq!(region["startLine"], 10);
        assert_eq!(region["startColumn"], 3);
    }

    #[test]
    fn sarif_no_region_when_no_line() {
        let files = single_file("spec.yaml", vec![make_violation(Severity::Warn)]);
        let mut buf = Vec::new();
        report(&files, Format::Sarif, ColorMode::Never, &mut buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        let phys = &parsed["runs"][0]["results"][0]["locations"][0]["physicalLocation"];
        assert!(phys.get("region").is_none());
        assert_eq!(parsed["runs"][0]["results"][0]["level"], "warning");
    }

    #[test]
    fn json_multi_file() {
        let files = vec![
            (
                PathBuf::from("a.yaml"),
                vec![make_violation(Severity::Error)],
            ),
            (
                PathBuf::from("b.yaml"),
                vec![make_violation(Severity::Warn)],
            ),
        ];
        let mut buf = Vec::new();
        report(&files, Format::Json, ColorMode::Never, &mut buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed["summary"]["total"], 2);
        assert_eq!(parsed["files"].as_array().unwrap().len(), 2);
    }
}
