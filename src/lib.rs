//! refract library — fast OpenAPI linter with Spectral OAS ruleset compatibility.
//!
//! Business logic lives here; `src/main.rs` is the thin CLI entry point.

#![deny(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
// "OpenAPI", "Swagger", "YAML", "JSON" are spec/format names, not code identifiers.
#![allow(clippy::doc_markdown)]

/// Error types for linting operations.
pub mod error;
/// Data model types: [`model::Violation`], [`model::Severity`], [`model::OasVersion`].
pub mod model;
/// Spec file parser — handles YAML, YML, and JSON inputs.
pub mod parser;
/// Two-pass YAML position indexing — maps paths to source line/col.
pub mod position;
/// Output formatting — text, JSON, and SARIF reporters.
pub mod reporter;
/// Built-in lint rules and the [`rules::Rule`] trait.
pub mod rules;
/// Spectral-compatible ruleset loader.
pub mod ruleset;

use std::path::{Path, PathBuf};

use error::LintError;
use model::Violation;

/// Result type returned by [`lint_dir`]: one entry per spec file found.
pub type DirLintResult = Vec<(PathBuf, Result<Vec<Violation>, LintError>)>;

/// Lint an `OpenAPI` spec file and return all violations.
///
/// Applies severity overrides and disabled rules from the optional ruleset file.
/// Violations are sorted by path for stable, deterministic output.
///
/// # Errors
///
/// Propagates [`LintError`] from parsing or ruleset loading.
pub fn lint(spec_path: &Path, ruleset_path: Option<&Path>) -> Result<Vec<Violation>, LintError> {
    // Pass 1: parse to serde_json::Value.
    let doc = parser::parse(spec_path)?;
    let version = model::OasVersion::detect(&doc)?;

    // Pass 2: build position index (YAML only; JSON files get an empty index).
    let pos_index = match spec_path.extension().and_then(|e| e.to_str()) {
        Some("yaml" | "yml") => {
            let content = std::fs::read_to_string(spec_path).unwrap_or_default();
            position::build_yaml(&content)
        }
        _ => position::empty(),
    };

    let config = match ruleset_path {
        Some(path) => ruleset::load(path)?,
        None => ruleset::RulesetConfig {
            severity_overrides: std::collections::HashMap::new(),
        },
    };

    let registry = rules::default_registry();

    // Warn about rule IDs in the ruleset that don't match any built-in rule.
    // This matches Spectral's behaviour of printing a warning for unknown rules.
    for rule_id in config.severity_overrides.keys() {
        if !registry.iter().any(|r| r.id() == rule_id) {
            eprintln!("[warn] unknown rule '{rule_id}' in ruleset — no built-in rule with this ID");
        }
    }

    let mut violations = Vec::new();

    for rule in &registry {
        // Resolve effective severity:
        // - key absent => use rule default
        // - key present, value None => rule is disabled (off)
        // - key present, value Some(s) => override with s
        let effective_severity = match config.severity_overrides.get(rule.id()) {
            None => Some(rule.default_severity()),
            Some(None) => None,
            Some(Some(s)) => Some(s.clone()),
        };

        let Some(severity) = effective_severity else {
            // Rule is disabled.
            continue;
        };

        let mut rule_violations = rule.check(&doc, version);
        for v in &mut rule_violations {
            v.severity = severity.clone();
            if let Some(span) = pos_index.get(&v.path) {
                v.line = Some(span.line);
                v.col = Some(span.col);
            }
        }
        violations.extend(rule_violations);
    }

    // Stable output regardless of iteration order.
    violations.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(violations)
}

/// Lint all `OpenAPI` spec files found by recursively walking `dir_path`.
///
/// Scans for `.yaml`, `.yml`, and `.json` files. Hidden directories (names
/// starting with `.`) are skipped, which excludes `.git/`, `.github/`, etc.
///
/// Per-file parse or lint failures are returned as `Err` entries in the result
/// vec rather than aborting the scan — callers should log them and continue.
///
/// # Errors
///
/// Returns [`LintError::Io`] if `dir_path` cannot be read at all.
pub fn lint_dir(dir_path: &Path, ruleset_path: Option<&Path>) -> Result<DirLintResult, LintError> {
    use walkdir::WalkDir;

    let mut results = Vec::new();

    let walker = WalkDir::new(dir_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            // Skip hidden directories (e.g. .git, .github, .idea).
            let name = entry.file_name().to_string_lossy();
            !(entry.file_type().is_dir() && name.starts_with('.'))
        });

    for entry in walker {
        let entry = entry
            .map_err(|e| {
                e.into_io_error()
                    .unwrap_or_else(|| std::io::Error::other("walkdir error"))
            })
            .map_err(LintError::Io)?;

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "yaml" | "yml" | "json") {
            continue;
        }

        let result = lint(path, ruleset_path);
        results.push((path.to_path_buf(), result));
    }

    Ok(results)
}
