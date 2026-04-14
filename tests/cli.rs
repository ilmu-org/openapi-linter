use std::path::Path;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_openapi-linter")
}

fn fixtures_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

#[test]
fn exit_0_on_valid_spec() {
    let status = Command::new(bin())
        .arg(fixtures_dir().join("valid_oas3.yaml"))
        .status()
        .expect("failed to run binary");
    assert_eq!(status.code(), Some(0));
}

#[test]
fn exit_1_on_spec_with_violations() {
    let status = Command::new(bin())
        .arg(fixtures_dir().join("missing_operationid.yaml"))
        .status()
        .expect("failed to run binary");
    assert_eq!(status.code(), Some(1));
}

#[test]
fn exit_2_on_nonexistent_file() {
    let status = Command::new(bin())
        .arg("/nonexistent/spec.yaml")
        .status()
        .expect("failed to run binary");
    assert_eq!(status.code(), Some(2));
}

#[test]
fn quiet_flag_suppresses_output() {
    let output = Command::new(bin())
        .arg("--quiet")
        .arg(fixtures_dir().join("missing_operationid.yaml"))
        .output()
        .expect("failed to run binary");
    assert!(output.stdout.is_empty(), "expected no stdout in quiet mode");
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn json_format_produces_valid_json() {
    let output = Command::new(bin())
        .args(["--format", "json"])
        .arg(fixtures_dir().join("valid_oas3.yaml"))
        .output()
        .expect("failed to run binary");
    assert_eq!(output.status.code(), Some(0));
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    assert!(parsed["violations"].is_array());
    assert!(parsed["summary"].is_object());
}
