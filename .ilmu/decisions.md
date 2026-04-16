# Architecture Decision Records

rust-architect writes. All build team agents read index before any task. Fetch ADR files from `.ilmu/decisions/` for relevant details. Contradict decision = file escalation issue on _ops first.

## Index

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [ADR-001](decisions/ADR-001.md) | Single-Crate Structure for v0.1.0 | Accepted | Single crate, internal module boundaries. `src/lib.rs` owns all bus... |
| [ADR-002](decisions/ADR-002.md) | YAML and JSON Parsing with serde + serde_y... | Accepted | - `serde_yaml = "0.9"` for YAML deserialization to `serde_yaml::Value`. |
| [ADR-003](decisions/ADR-003.md) | OpenAPI Validation via Hand-Rolled Rule En... | Accepted | Parsed OpenAPI docs as `serde_json::Value` (normalised from YA... |
| [ADR-004](decisions/ADR-004.md) | CLI with clap v4 | Accepted | Use `clap = "4"` with `derive` feature. |
| [ADR-005](decisions/ADR-005.md) | Error Handling with thiserror in lib, anyh... | Accepted | - `src/lib.rs` and modules under `src/`: `thiserror` for typed errors... |
| [ADR-006](decisions/ADR-006.md) | Built-in OAS Ruleset — 15 High-Value Spect... | Accepted | 15 rules as `Rule` structs in `src/rules/`. Each rule struct... |
| [ADR-007](decisions/ADR-007.md) | Static Binary Compilation Strategy | Accepted | - Release targets: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`,... |
| [ADR-008](decisions/ADR-008.md) | Spectral Ruleset YAML Compatibility Scope ... | Accepted | v0.1.0 supports: |
| [ADR-009](decisions/ADR-009.md) | Output Formats — Text (Default) and JSON | Accepted | Two formats via `--format text|json`: |
| [ADR-010](decisions/ADR-010.md) | No Async Runtime for v0.1.0 | Accepted | No async. All I/O synchronous (`std::fs::read_to_string`). Rule... |
| [ADR-011](decisions/ADR-011.md) | Position Indexing via yaml-rust2 Two-Pass ... | Accepted | YAML spec files: v0.2.0 two-pass approach: |
| [ADR-012](decisions/ADR-012.md) | Flat `line`/`col` Fields on Violation (Not... | Accepted | Option A. Extend `Violation`: |
| [ADR-013](decisions/ADR-013.md) | Reporter API — Batch Signature for Multi-F... | Accepted | v0.2.0 reporter API: |
| [ADR-014](decisions/ADR-014.md) | Directory Scanning with walkdir; lint_dir ... | Accepted | **API shape**: |
| [ADR-015](decisions/ADR-015.md) | Document-Internal `$ref` Resolution Utilit... | Accepted | Shared utility in `src/rules/util.rs`: |
| [ADR-016](decisions/ADR-016.md) | mod.rs → Named Module Rename Confirms Rust... | Accepted | Rename pattern (applied to `parser`, `model`, `rules`, `ruleset`, `reporter`): |
| [ADR-017](decisions/ADR-017.md) | Project Rename — openapi-linter → refract | Accepted | Rename project from `openapi-linter` to `refract` at pre-v0.3.0 stage... |
| [ADR-018](decisions/ADR-018.md) | v0.3.0 Rule Set, 17 Structural and Correct... | Accepted | v0.3.0 ships exactly 17 class-1 structural/correctness rules; no schema-eval... |
| [ADR-019](decisions/ADR-019.md) | JSON Schema Validation Rules Deferred to v... | Accepted | Defer all four schema-eval rules to v0.4.0. Defer boon (or alt... |
| [ADR-020](decisions/ADR-020.md) | Cross-File $ref Resolution Deferred to v0.4.0 | Accepted | Defer cross-file `$ref` resolution to v0.4.0. Internal-only `deref` util... |
| [ADR-021](decisions/ADR-021.md) | OAS-Version Gating Helper and Deref-Before... | Accepted | **Version gating helper.** Small helper in `src/rules/util.rs`: |
| [ADR-022](decisions/ADR-022.md) | Boon Crate Integration for JSON Schema Validation | Accepted | boon 0.6.1 confirmed. LintContext struct wraps doc, version, schemas. OAS schemas bundled via include_str!, OnceLock. Error tree -> flat Violations, truncated at 64. Malformed user schema = LintError. |
| [ADR-023](decisions/ADR-023.md) | Cross-File $ref Resolution Strategy | Accepted | Eager pre-pass: resolve_external_refs(doc, base_path) -> Result<Value>. HashMap cache per call. Cycle detection via HashSet of (path, pointer) pairs. Failure modes -> LintError. HTTP refs forbidden. |
| [ADR-024](decisions/ADR-024.md) | Workspace Structure Assessment -- v0.4.0 Deferral | Accepted | No extraction in v0.4.0. Triggers: library consumer issue, secondary binary, or >15K LOC. ADR-001 single-crate continues. |
| [ADR-025](decisions/ADR-025.md) | GitHub Actions Node.js 24 Migration | Accepted | Standalone chore PR. checkout@v4->v6, upload-artifact@v4->v7, download-artifact@v4->v8, softprops/action-gh-release@v2->v3. All on node24. Ship before June 2026 cutoff. |