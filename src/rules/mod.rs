mod info_contact;
mod info_description;
mod open_api_tags;
mod operation_description;
mod operation_operation_id;
mod operation_operation_id_unique;
mod operation_summary;
mod operation_tags;

use crate::model::{OasVersion, Severity, Violation};

/// A lint rule that can check an `OpenAPI` document.
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn message(&self) -> &'static str;
    fn default_severity(&self) -> Severity;
    fn check(&self, doc: &serde_json::Value, version: OasVersion) -> Vec<Violation>;
}

/// All HTTP methods that can carry an operation object.
pub(crate) const HTTP_METHODS: &[&str] = &[
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// Returns all built-in rules in a stable order.
#[must_use]
pub fn default_registry() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(operation_operation_id::OperationOperationId),
        Box::new(operation_operation_id_unique::OperationOperationIdUnique),
        Box::new(operation_tags::OperationTags),
        Box::new(operation_summary::OperationSummary),
        Box::new(info_contact::InfoContact),
        Box::new(info_description::InfoDescription),
        Box::new(open_api_tags::OpenApiTags),
        Box::new(operation_description::OperationDescription),
    ]
}
