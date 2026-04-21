use std::path::Path;

use refract_cli::lint;

fn fixtures_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

#[test]
fn valid_spec_produces_no_violations() {
    let path = fixtures_dir().join("valid_oas3.yaml");
    let violations = lint(&path, None).expect("lint should succeed");
    assert!(
        violations.is_empty(),
        "expected no violations, got: {violations:#?}"
    );
}

#[test]
fn missing_operationid_produces_violation() {
    let path = fixtures_dir().join("missing_operationid.yaml");
    let violations = lint(&path, None).expect("lint should succeed");
    let has_op_id_violation = violations
        .iter()
        .any(|v| v.rule_id == "operation-operationId");
    assert!(
        has_op_id_violation,
        "expected operation-operationId violation, got: {violations:#?}"
    );
}

#[test]
fn missing_summary_produces_violation() {
    let path = fixtures_dir().join("missing_summary.yaml");
    let violations = lint(&path, None).expect("lint should succeed");
    let has_summary_violation = violations.iter().any(|v| v.rule_id == "operation-summary");
    assert!(
        has_summary_violation,
        "expected operation-summary violation, got: {violations:#?}"
    );
}

#[test]
fn nonexistent_file_returns_io_error() {
    let path = Path::new("/nonexistent/spec.yaml");
    let result = lint(path, None);
    assert!(result.is_err(), "expected error for nonexistent file");
}

#[test]
fn ruleset_can_disable_rule() {
    use std::io::Write as _;

    let mut ruleset = tempfile::Builder::new().suffix(".yaml").tempfile().unwrap();
    writeln!(
        ruleset,
        "extends: \"spectral:oas\"\noperation-operationId: off"
    )
    .unwrap();

    let spec_path = fixtures_dir().join("missing_operationid.yaml");
    let violations = lint(&spec_path, Some(ruleset.path())).expect("lint should succeed");
    let has_op_id_violation = violations
        .iter()
        .any(|v| v.rule_id == "operation-operationId");
    assert!(
        !has_op_id_violation,
        "operation-operationId should be disabled by ruleset"
    );
}

// v0.5.0 integration tests

#[test]
fn oas2_parameter_description_triggers_on_missing_description() {
    let path = fixtures_dir().join("v0.5.0/oas2-parameter-description.yaml");
    let violations = lint(&path, None).expect("lint should succeed");
    let has_violation = violations
        .iter()
        .any(|v| v.rule_id == "oas2-parameter-description");
    assert!(
        has_violation,
        "expected oas2-parameter-description violation, got: {violations:#?}"
    );
}

#[test]
fn oas2_api_schemes_triggers_when_absent() {
    let path = fixtures_dir().join("v0.5.0/oas2-api-schemes.yaml");
    let violations = lint(&path, None).expect("lint should succeed");
    let has_violation = violations.iter().any(|v| v.rule_id == "oas2-api-schemes");
    assert!(
        has_violation,
        "expected oas2-api-schemes violation, got: {violations:#?}"
    );
}

#[test]
fn oas2_anyof_triggers_on_anyof_in_schema() {
    let path = fixtures_dir().join("v0.5.0/oas2-anyof.yaml");
    let violations = lint(&path, None).expect("lint should succeed");
    let has_violation = violations.iter().any(|v| v.rule_id == "oas2-anyOf");
    assert!(
        has_violation,
        "expected oas2-anyOf violation, got: {violations:#?}"
    );
}

#[test]
fn oas2_oneof_triggers_on_oneof_in_schema() {
    let path = fixtures_dir().join("v0.5.0/oas2-oneof.yaml");
    let violations = lint(&path, None).expect("lint should succeed");
    let has_violation = violations.iter().any(|v| v.rule_id == "oas2-oneOf");
    assert!(
        has_violation,
        "expected oas2-oneOf violation, got: {violations:#?}"
    );
}

#[test]
fn oas3_valid_media_example_triggers_on_invalid_mediatype_example() {
    let path = fixtures_dir().join("v0.5.0/oas3-valid-media-example.yaml");
    let violations = lint(&path, None).expect("lint should succeed");
    let has_violation = violations
        .iter()
        .any(|v| v.rule_id == "oas3-valid-media-example");
    assert!(
        has_violation,
        "expected oas3-valid-media-example violation, got: {violations:#?}"
    );
}

#[test]
fn oas2_valid_media_example_triggers_on_invalid_response_example() {
    let path = fixtures_dir().join("v0.5.0/oas2-valid-media-example.yaml");
    let violations = lint(&path, None).expect("lint should succeed");
    let has_violation = violations
        .iter()
        .any(|v| v.rule_id == "oas2-valid-media-example");
    assert!(
        has_violation,
        "expected oas2-valid-media-example violation, got: {violations:#?}"
    );
}
