/// Severity level for a lint violation.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Blocks CI — must fix.
    Error,
    /// Should fix but not blocking.
    Warn,
    /// Informational suggestion.
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
    /// Source line number (1-based), if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Source column number (1-based), if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
}

impl Violation {
    /// Create a violation with `line` and `col` set to `None`.
    #[must_use]
    pub fn new(
        rule_id: impl Into<String>,
        message: impl Into<String>,
        severity: Severity,
        path: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            message: message.into(),
            severity,
            path: path.into(),
            line: None,
            col: None,
        }
    }
}
