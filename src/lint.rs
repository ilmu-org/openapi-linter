//! `LintContext` — shared lint state passed to every rule invocation.
//!
//! Rules receive `&LintContext<'_>` instead of raw `(doc, version)` parameters.
//! This decouples rule implementations from the set of lint state they receive
//! and makes it easy to extend the context with new fields without touching
//! every rule's signature again.
//!
//! ADR-022 defines the shape. ADR-023 adds `base_path` for future use.

use std::path::Path;

use boon::Schemas;
use serde_json::Value;

use crate::model::OasVersion;

/// Shared context for a single rule invocation.
///
/// Created once per file in `lint()`, then passed (by reference) to each rule.
// `schemas` and `base_path` are reserved for Phase 3 example-validation rules.
#[allow(dead_code)]
pub struct LintContext<'a> {
    /// The fully-resolved OpenAPI document (after external `$ref` pre-pass).
    pub doc: &'a Value,
    /// Detected OAS version of this document.
    pub version: OasVersion,
    /// Shared boon schema registry for this lint session.
    ///
    /// OAS JSON Schemas are pre-registered before rule dispatch. Rules that
    /// need to register additional user-defined schemas (e.g. example
    /// validation rules in Phase 3) use a separate mutable handle passed
    /// alongside the context.
    pub schemas: &'a Schemas,
    /// Path of the spec file being linted, if available.
    ///
    /// Not used by any v0.4.0 rule; reserved for future deref-dependent rules
    /// that may need to report relative paths (ADR-023 extension point).
    pub base_path: Option<&'a Path>,
}
