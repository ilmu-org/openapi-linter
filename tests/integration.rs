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
