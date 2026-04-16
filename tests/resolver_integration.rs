//! Integration tests for the cross-file $ref resolver (Phase 1, v0.4.0).

use std::path::Path;

use refract_cli::resolver::{ResolveError, resolve_external_refs};

/// Helper: path to a fixture directory.
fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/external-refs")
        .join(name)
}

#[test]
fn basic_external_ref_is_inlined() {
    let main = fixture("basic-external-ref/main.yaml");
    let content = std::fs::read_to_string(&main).unwrap();
    let raw: serde_json::Value = serde_yaml::from_str(&content).unwrap();
    let raw: serde_json::Value = serde_json::to_value(raw).unwrap();

    let base_dir = main.parent().unwrap();
    let (resolved, errors) = resolve_external_refs(raw, base_dir);

    assert!(errors.is_empty(), "expected no errors, got: {errors:?}");

    // After resolution the $ref node should be replaced by the inlined schema.
    let schema = &resolved["paths"]["/pets"]["get"]["responses"]["200"]["content"]["application/json"]
        ["schema"];
    assert!(
        schema.get("$ref").is_none(),
        "external $ref should be inlined, but still present: {schema}"
    );
    assert_eq!(schema["type"], "object", "Pet type should be 'object'");
}

#[test]
fn nested_external_ref_is_inlined() {
    let main = fixture("nested-external-ref/main.yaml");
    let content = std::fs::read_to_string(&main).unwrap();
    let raw: serde_json::Value = serde_yaml::from_str(&content).unwrap();
    let raw: serde_json::Value = serde_json::to_value(raw).unwrap();

    let base_dir = main.parent().unwrap();
    let (resolved, errors) = resolve_external_refs(raw, base_dir);

    assert!(errors.is_empty(), "expected no errors, got: {errors:?}");

    let schema = &resolved["paths"]["/items"]["get"]["responses"]["200"]["content"]["application/json"]
        ["schema"];
    // After nested resolution: a.yaml#/Foo -> b.yaml#/Bar -> { type: string }
    assert!(schema.get("$ref").is_none(), "should be fully inlined");
    assert_eq!(schema["type"], "string");
}

#[test]
fn cycle_produces_error() {
    let main = fixture("cycle/main.yaml");
    let content = std::fs::read_to_string(&main).unwrap();
    let raw: serde_json::Value = serde_yaml::from_str(&content).unwrap();
    let raw: serde_json::Value = serde_json::to_value(raw).unwrap();

    let base_dir = main.parent().unwrap();
    let (_resolved, errors) = resolve_external_refs(raw, base_dir);

    assert!(
        !errors.is_empty(),
        "expected a cycle error but got no errors"
    );
    let has_cycle = errors
        .iter()
        .any(|e| matches!(e, ResolveError::Cycle { .. }));
    assert!(has_cycle, "expected ResolveError::Cycle, got: {errors:?}");
}

#[test]
fn missing_file_produces_error() {
    let main = fixture("missing-file/main.yaml");
    let content = std::fs::read_to_string(&main).unwrap();
    let raw: serde_json::Value = serde_yaml::from_str(&content).unwrap();
    let raw: serde_json::Value = serde_json::to_value(raw).unwrap();

    let base_dir = main.parent().unwrap();
    let (_resolved, errors) = resolve_external_refs(raw, base_dir);

    assert!(
        !errors.is_empty(),
        "expected a file-not-found error but got none"
    );
    let has_not_found = errors.iter().any(|e| {
        matches!(
            e,
            ResolveError::FileNotFound { .. } | ResolveError::MalformedFile { .. }
        )
    });
    assert!(
        has_not_found,
        "expected FileNotFound or MalformedFile, got: {errors:?}"
    );
}

#[test]
fn missing_pointer_produces_error() {
    let main = fixture("missing-pointer/main.yaml");
    let content = std::fs::read_to_string(&main).unwrap();
    let raw: serde_json::Value = serde_yaml::from_str(&content).unwrap();
    let raw: serde_json::Value = serde_json::to_value(raw).unwrap();

    let base_dir = main.parent().unwrap();
    let (_resolved, errors) = resolve_external_refs(raw, base_dir);

    assert!(
        !errors.is_empty(),
        "expected a pointer-not-found error but got none"
    );
    let has_pointer_err = errors
        .iter()
        .any(|e| matches!(e, ResolveError::PointerNotFound { .. }));
    assert!(has_pointer_err, "expected PointerNotFound, got: {errors:?}");
}

#[test]
fn http_ref_produces_error() {
    let main = fixture("http-ref/main.yaml");
    let content = std::fs::read_to_string(&main).unwrap();
    let raw: serde_json::Value = serde_yaml::from_str(&content).unwrap();
    let raw: serde_json::Value = serde_json::to_value(raw).unwrap();

    let base_dir = main.parent().unwrap();
    let (_resolved, errors) = resolve_external_refs(raw, base_dir);

    assert!(
        !errors.is_empty(),
        "expected an HTTP ref error but got none"
    );
    let has_http_err = errors
        .iter()
        .any(|e| matches!(e, ResolveError::HttpRefForbidden { .. }));
    assert!(has_http_err, "expected HttpRefForbidden, got: {errors:?}");
}

#[test]
fn lint_with_external_refs_returns_violations_not_error() {
    // The lint() function must continue when external refs fail to resolve.
    // It should return Ok with resolve errors as Violations (not Err).
    let main = fixture("missing-file/main.yaml");
    let result = refract_cli::lint(&main, None);
    // Should not return Err -- partial resolution continues.
    assert!(
        result.is_ok(),
        "lint() should return Ok even with unresolvable refs, got: {result:?}"
    );
    let violations = result.unwrap();
    // Should have at least one violation from $ref-resolution.
    let has_ref_violation = violations.iter().any(|v| v.rule_id == "$ref-resolution");
    assert!(
        has_ref_violation,
        "expected a $ref-resolution violation, got: {violations:#?}"
    );
}
