// openapi-linter library root.
// Business logic lives here; src/main.rs is the thin CLI entry point.

#![deny(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod error;
pub mod model;
pub mod parser;
pub mod reporter;
pub mod rules;
pub mod ruleset;

use std::path::Path;

use error::LintError;
use model::Violation;

/// Lint an `OpenAPI` spec file and return all violations.
///
/// Applies severity overrides and disabled rules from the optional ruleset file.
/// Violations are sorted by path for stable, deterministic output.
///
/// # Errors
///
/// Propagates [`LintError`] from parsing or ruleset loading.
pub fn lint(spec_path: &Path, ruleset_path: Option<&Path>) -> Result<Vec<Violation>, LintError> {
    let doc = parser::parse(spec_path)?;
    let version = model::OasVersion::detect(&doc)?;

    let config = match ruleset_path {
        Some(path) => ruleset::load(path)?,
        None => ruleset::RulesetConfig {
            severity_overrides: std::collections::HashMap::new(),
        },
    };

    let registry = rules::default_registry();
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
        }
        violations.extend(rule_violations);
    }

    // Stable output regardless of iteration order.
    violations.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(violations)
}
