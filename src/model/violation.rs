/// Severity level for a lint violation.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warn,
    Info,
}

/// A single lint violation emitted by a rule.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Violation {
    /// The rule that produced this violation.
    pub rule_id: String,
    /// Human-readable description of the problem.
    pub message: String,
    /// How severe the problem is.
    pub severity: Severity,
    /// JSON Pointer location within the document (e.g. `/paths/~1foo/get`).
    pub path: String,
}
