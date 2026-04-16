mod array_items;
mod contact_properties;
mod duplicated_entry_in_enum;
mod info_contact;
mod info_description;
mod info_license;
mod license_url;
mod no_eval_in_markdown;
mod no_ref_siblings;
mod no_script_tags_in_markdown;
mod oas2_schema;
mod oas2_valid_schema_example;
mod oas3_api_servers;
mod oas3_parameter_description;
mod oas3_schema;
mod oas3_server_not_example_com;
mod oas3_server_trailing_slash;
mod oas3_valid_schema_example;
mod open_api_tags;
mod openapi_tags_alphabetical;
mod openapi_tags_uniqueness;
mod operation_description;
mod operation_operation_id;
mod operation_operation_id_unique;
mod operation_operation_id_valid_in_url;
mod operation_parameters;
mod operation_success_response;
mod operation_summary;
mod operation_tag_defined;
mod operation_tags;
mod path_declarations_must_exist;
mod path_keys_no_trailing_slash;
mod path_not_include_query;
mod path_params;
mod tag_description;
mod typed_enum;
pub(crate) mod util;

use crate::lint::LintContext;
use crate::model::{Severity, Violation};

/// A lint rule that can check an `OpenAPI` document.
pub trait Rule: Send + Sync {
    /// Unique rule identifier, e.g. `"operation-operationId"`.
    fn id(&self) -> &'static str;
    /// Short human-readable description of what the rule checks.
    fn message(&self) -> &'static str;
    /// Severity used when no override is present in the ruleset config.
    fn default_severity(&self) -> Severity;
    /// Run the rule against the lint context and return all violations found.
    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation>;
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
        // v0.3.0 Phase 1: structural rules
        Box::new(path_keys_no_trailing_slash::PathKeysNoTrailingSlash),
        Box::new(path_not_include_query::PathNotIncludeQuery),
        Box::new(path_declarations_must_exist::PathDeclarationsMustExist),
        Box::new(openapi_tags_uniqueness::OpenApiTagsUniqueness),
        Box::new(tag_description::TagDescription),
        Box::new(oas3_server_trailing_slash::Oas3ServerTrailingSlash),
        Box::new(oas3_server_not_example_com::Oas3ServerNotExampleCom),
        Box::new(no_ref_siblings::NoRefSiblings),
        Box::new(oas3_api_servers::Oas3ApiServers),
        Box::new(operation_success_response::OperationSuccessResponse),
        Box::new(operation_operation_id_valid_in_url::OperationOperationIdValidInUrl),
        // v0.3.0 Phase 3: deref-dependent rules
        Box::new(array_items::ArrayItems),
        Box::new(oas3_parameter_description::Oas3ParameterDescription),
        Box::new(operation_parameters::OperationParameters),
        Box::new(operation_tag_defined::OperationTagDefined),
        // v0.3.0 Phase 4: type-aware rules
        Box::new(duplicated_entry_in_enum::DuplicatedEntryInEnum),
        Box::new(typed_enum::TypedEnum),
        // v0.4.0: JSON Schema structural validation rules
        Box::new(oas3_schema::Oas3Schema),
        Box::new(oas2_schema::Oas2Schema),
        // v0.4.0: Example validation rules
        Box::new(oas3_valid_schema_example::Oas3ValidSchemaExample),
        Box::new(oas2_valid_schema_example::Oas2ValidSchemaExample),
    ]
}
