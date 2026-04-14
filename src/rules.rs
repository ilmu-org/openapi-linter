mod contact_properties;
mod info_contact;
mod info_description;
mod info_license;
mod license_url;
mod no_eval_in_markdown;
mod no_script_tags_in_markdown;
mod open_api_tags;
mod openapi_tags_alphabetical;
mod operation_description;
mod operation_operation_id;
mod operation_operation_id_unique;
mod operation_summary;
mod operation_tags;
mod path_params;
pub(crate) mod util;

use crate::model::{OasVersion, Severity, Violation};

/// A lint rule that can check an `OpenAPI` document.
pub trait Rule: Send + Sync {
    /// Unique rule identifier, e.g. `"operation-operationId"`.
    fn id(&self) -> &'static str;
    /// Short human-readable description of what the rule checks.
    fn message(&self) -> &'static str;
    /// Severity used when no override is present in the ruleset config.
    fn default_severity(&self) -> Severity;
    /// Run the rule against `doc` and return all violations found.
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
        // Operations
        Box::new(operation_operation_id::OperationOperationId),
        Box::new(operation_operation_id_unique::OperationOperationIdUnique),
        Box::new(operation_tags::OperationTags),
        Box::new(operation_summary::OperationSummary),
        Box::new(operation_description::OperationDescription),
        // Info
        Box::new(info_contact::InfoContact),
        Box::new(info_description::InfoDescription),
        Box::new(info_license::InfoLicense),
        Box::new(license_url::LicenseUrl),
        Box::new(contact_properties::ContactProperties),
        // Tags
        Box::new(open_api_tags::OpenApiTags),
        Box::new(openapi_tags_alphabetical::OpenApiTagsAlphabetical),
        // Paths
        Box::new(path_params::PathParams),
        // Security / markdown
        Box::new(no_eval_in_markdown::NoEvalInMarkdown),
        Box::new(no_script_tags_in_markdown::NoScriptTagsInMarkdown),
    ]
}
