# Architecture Decision Records

Written by rust-architect only.
All build team agents must read this file before starting any task.
Contradicting a decision requires filing an escalation issue on _ops before proceeding.

---

# ADR-001: Single-Crate Structure for v0.1.0

**Date**: 2026-04-09
**Status**: Accepted

## Context

openapi-linter ships as a single binary. There is no library consumer outside the binary itself in this milestone. A workspace adds indirection (virtual manifest, cross-crate dependency resolution, publish sequencing) without delivering value at this scale.

## Decision

Use a single crate with internal module boundaries. `src/lib.rs` owns all business logic. `src/main.rs` is a thin entry point that calls into lib. Modules:

```
src/
  main.rs          — entry point, process exit codes
  lib.rs           — public API surface (parse, lint, format output)
  parser/          — OpenAPI document loading (YAML + JSON → internal model)
  model/           — internal OpenAPI document representation
  rules/           — built-in OAS ruleset, rule trait, rule registry
  ruleset/         — .spectral.yaml file loading and merging
  reporter/        — violation formatting (text, JSON)
  error.rs         — crate-level error types (thiserror)
```

## Consequences

- Simple build: `cargo build --release` produces the binary directly.
- No cross-crate API surface to maintain — all types are `pub(crate)` by default.
- If a downstream library consumer emerges (v0.2.0+), extracting a `openapi-linter-core` crate is a straightforward refactor — the module boundaries already match crate boundaries.
- Slight risk: if the codebase grows past ~25K LOC, module-level organisation may feel cramped. Acceptable for v0.1.0.

---

# ADR-002: YAML and JSON Parsing with serde + serde_yaml + serde_json

**Date**: 2026-04-09
**Status**: Accepted

## Context

OpenAPI specs are authored in YAML or JSON. The parser must:
1. Detect format from file extension or content sniffing.
2. Deserialise to a generic value tree (preserve all keys, including unknown extension fields like `x-*`).
3. Retain span/location information for line-accurate violation reporting.

Candidates evaluated:

| Crate | Maintained | License | Span support | Notes |
|-------|-----------|---------|--------------|-------|
| `serde_yaml` (dtolnay, v0.9) | Yes | MIT/Apache | No native span | Deserialises to `serde_yaml::Value` with `Mapping` retaining insertion order |
| `serde_yaml` v0.9 + `marked_yaml` | `marked_yaml` unmaintained | — | Yes | Dependency risk |
| `yaml-rust2` | Yes (fork of yaml-rust) | MIT/Apache | Yes (Marker) | Low-level; would need hand-written deserialization |
| `serde_json` | Yes | MIT/Apache | No native span | Needed anyway for JSON specs |
| `libyaml-safer` | Yes | MIT | Partial | Wraps libyaml; C FFI; not static-binary-friendly |

YAML span information is needed for line-accurate violation reporting. `serde_yaml::Value` does not expose spans. The pragmatic solution for v0.1.0: parse with `serde_yaml` to get the document tree, then do a second-pass with `yaml-rust2` (or `serde_yaml`'s internal scanner) to build a position index keyed on JSON Pointer paths.

For v0.1.0 the position index approach is sufficient: most OAS rules fire on structural paths (e.g. `paths./foo.get.responses.200`) where a path-to-line lookup table is accurate enough.

## Decision

- `serde_yaml = "0.9"` for YAML deserialization to `serde_yaml::Value`.
- `serde_json = "1"` for JSON deserialization to `serde_json::Value`.
- Internal `model::PositionIndex` built during parsing: a `HashMap<JsonPointer, Span>` mapping each node's JSON Pointer path to its (line, col) in the source. Populated by a single-pass YAML/JSON event scanner run after `serde_yaml`/`serde_json` deserialization.
- `Span` is `pub struct Span { pub line: u32, pub col: u32 }` — simple, allocation-free.

## Consequences

- Two-pass parsing adds a small constant overhead (~5–10% on large specs). Acceptable.
- `serde_yaml` 0.9 uses `unsafe-libyaml` under the hood (pure Rust YAML parser). Static binary friendly — no C dependency.
- `serde_json` is the standard; zero risk.
- The position index covers structural nodes only. For v0.1.0 that is sufficient. Inline value spans (e.g. column offset within a string) deferred to v0.2.0.

---

# ADR-003: OpenAPI Validation via Hand-Rolled Rule Engine, Not an External OAS Crate

**Date**: 2026-04-09
**Status**: Accepted

## Context

Candidates for OpenAPI document handling:

| Crate | Status | Notes |
|-------|--------|-------|
| `openapiv3` | Maintained (but slow-moving) | Deserialises OAS 3.0 only; no 2.x (Swagger); no 3.1 |
| `oas3` | Archived/unmaintained | Unsafe; no active maintenance |
| `openapi` (softprops) | Unmaintained | |
| `utoipa` | Maintained | Code-generation focused; not a linter substrate |
| Hand-rolled `serde_yaml::Value` traversal | N/A | Full control; supports 2.x/3.x/3.1 uniformly |

The core user value is running Spectral-compatible rules across OpenAPI 2.x, 3.0, and 3.1. No existing Rust crate covers all three versions with adequate maintenance. Using `openapiv3` would couple us to its type system for OAS 3.0 only, requiring a separate path for 2.x and 3.1.

## Decision

Represent parsed OpenAPI documents as `serde_json::Value` (normalised from YAML or JSON). Implement a `Rule` trait:

```rust
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn message(&self) -> &'static str;
    fn check(&self, doc: &Value, index: &PositionIndex) -> Vec<Violation>;
}
```

Rules traverse the `Value` tree directly using JSON Pointer paths. The rule engine iterates the registry and collects `Violation` structs:

```rust
pub struct Violation {
    pub rule_id: String,
    pub message: String,
    pub path: String,      // JSON Pointer
    pub span: Option<Span>,
}
```

Built-in OAS rules are implemented as `Rule` structs in the `rules/` module. For v0.1.0, implement the 15 highest-value Spectral OAS rules (see ADR-006 for rule list).

## Consequences

- Full control over 2.x/3.x/3.1 support — no crate forces a type system on us.
- `serde_json::Value` traversal is verbose but explicit. Rule logic is easy to test in isolation.
- No external OAS crate dependency means no version lock-in and no abandoned-crate risk.
- Downside: we own the OpenAPI structure knowledge. Mitigated by: rules are unit-testable with small fixture YAML files; the Spectral OAS ruleset is well-documented.

---

# ADR-004: CLI with clap v4

**Date**: 2026-04-09
**Status**: Accepted

## Context

CLI argument parsing options:

| Crate | Notes |
|-------|-------|
| `clap v4` | Industry standard; derive macro; MIT/Apache; actively maintained |
| `argh` | Minimal; Google-maintained; less flexible |
| `pico-args` | Zero-alloc minimal; no subcommands; too bare for a tool with multiple output formats |
| Hand-rolled | No value added |

## Decision

Use `clap = "4"` with the `derive` feature.

CLI surface for v0.1.0:

```
openapi-linter [OPTIONS] <spec>

Arguments:
  <spec>              Path to OpenAPI spec file (YAML or JSON)

Options:
  -r, --ruleset <FILE>    Custom Spectral ruleset YAML (overrides built-in rules)
  -f, --format <FORMAT>   Output format: text (default), json
      --no-color          Disable ANSI color in text output
  -q, --quiet             Suppress output; exit code only
  -h, --help              Print help
  -V, --version           Print version
```

Exit codes: 0 = no violations, 1 = violations found, 2 = error (unparseable spec, missing file, etc.).

## Consequences

- `clap` is the right tool for this job. No surprises.
- The derive macro adds ~100K to binary size. Acceptable for a linter binary.
- Exit code contract (0/1/2) is documented and stable — CI users depend on it.

---

# ADR-005: Error Handling with thiserror in lib, anyhow in main

**Date**: 2026-04-09
**Status**: Accepted

## Context

The project follows the org-level Rust workspace standard: `thiserror` for library error types, `anyhow` for application-level error propagation. This project is a CLI tool with a library core in `src/lib.rs`.

## Decision

- `src/lib.rs` and all modules under `src/`: use `thiserror` to define typed error enums. Errors are specific and matchable.
- `src/main.rs`: use `anyhow::Result` for top-level propagation. Errors from lib are wrapped with context using `.context()`.
- No `unwrap()` or `expect()` in library code. Panics are acceptable only in test assertions.

Error types:

```rust
// src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum LintError {
    #[error("cannot read spec file: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot parse YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("cannot parse JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid OpenAPI document: {0}")]
    InvalidSpec(String),
    #[error("cannot load ruleset: {0}")]
    Ruleset(String),
}
```

## Consequences

- `thiserror` errors are testable and matchable. Library consumers (future v0.2.0+) can pattern-match error variants.
- `anyhow` in main provides ergonomic error display without defining output error types.
- Consistent with org-level `[workspace.dependencies]` standard.

---

# ADR-006: Built-in OAS Ruleset — 15 High-Value Spectral-Compatible Rules for v0.1.0

**Date**: 2026-04-09
**Status**: Accepted

## Context

The migration hook is Spectral ruleset compatibility. Users have existing `.spectral.yaml` governance configs. For v0.1.0 the goal is: run the most commonly-triggered Spectral OAS rules so that users see comparable violations on their specs.

Spectral's `@stoplight/spectral-rulesets` OAS ruleset contains ~40 rules. The 15 highest-value rules (by frequency of violation in typical API specs) are:

1. `operation-operationId` — every operation must have an operationId
2. `operation-operationId-unique` — operationIds must be unique
3. `operation-tags` — every operation must have at least one tag
4. `operation-summary` — every operation must have a summary
5. `operation-description` — operations should have a description
6. `info-contact` — info object must have a contact
7. `info-description` — info object must have a description
8. `info-license` — info object must have a license
9. `no-eval-in-markdown` — no `eval()` in description fields
10. `no-script-tags-in-markdown` — no `<script>` in description fields
11. `openapi-tags` — top-level tags object must exist
12. `openapi-tags-alphabetical` — tags should be alphabetically sorted
13. `path-params` — path parameters must be defined
14. `contact-properties` — contact object should have name, url, email
15. `license-url` — license object should have a url

## Decision

Implement these 15 rules as `Rule` structs in `src/rules/`. Each rule struct is zero-size (no fields). The rule registry is a `Vec<Box<dyn Rule>>` built at startup.

Ruleset file support (`.spectral.yaml`): for v0.1.0, support `extends: [spectral:oas]` and per-rule severity overrides (`off`, `warn`, `error`). Do not support custom JavaScript functions — this is a Rust-only binary; JS function rules are out of scope.

## Consequences

- 15 rules covers the most common Spectral OAS violations. Users with existing specs will see meaningful output on first run.
- Custom JS functions are explicitly not supported — this is a feature, not a gap. The binary's value proposition is no runtime dependencies.
- Rule list is expandable in v0.2.0 by adding new structs to `src/rules/` and registering them. No architectural change required.

---

# ADR-007: Static Binary Compilation Strategy

**Date**: 2026-04-09
**Status**: Accepted

## Context

"Single static binary, no runtime dependencies" is the core user promise. Across platforms:

- **macOS**: `x86_64-apple-darwin` and `aarch64-apple-darwin` binaries link against `libSystem.dylib` (always present). This is acceptable — macOS has no musl target and the system dylib is not a user-installed dependency. Cross-compile with `cargo build --target aarch64-apple-darwin` on x86_64 macOS or use GitHub Actions matrix.
- **Linux**: Must be fully static. Use `x86_64-unknown-linux-musl` target. Cross-compile with `cross` or a musl Docker image in CI.
- **Windows**: `x86_64-pc-windows-msvc` — links MSVC runtime which ships with Windows. Acceptable. Build in GitHub Actions Windows runner.

No C FFI dependencies in the selected crate set (serde_yaml uses `unsafe-libyaml`, a pure-Rust YAML parser — not a C binding). This enables musl builds without a C toolchain.

## Decision

- Release targets: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.
- CI builds all five targets in GitHub Actions matrix.
- Set in `Cargo.toml`:
  ```toml
  [profile.release]
  opt-level = "z"       # Optimise for binary size
  lto = true            # Link-time optimisation
  codegen-units = 1     # Single codegen unit for maximum LTO
  strip = true          # Strip debug symbols
  ```
- Distribute via GitHub Releases as compressed tarballs (`.tar.gz` for Unix, `.zip` for Windows). Add `brew` tap and `cargo install` as secondary distribution in v0.2.0.

## Consequences

- musl builds require the musl toolchain in CI. `cross` (a Docker-based cross-compilation tool) handles this without host configuration.
- LTO + strip + opt-level "z" reduces binary size significantly. Expected binary size: ~3–5MB stripped.
- No CGO, no libc, no system-installed runtime on Linux. This is the differentiating property vs Spectral.

---

# ADR-008: Spectral Ruleset YAML Compatibility Scope for v0.1.0

**Date**: 2026-04-09
**Status**: Accepted

## Context

Spectral ruleset YAML files can reference:
1. `extends` — inherit from built-in rulesets (`spectral:oas`, `spectral:asyncapi`) or remote URLs
2. Per-rule severity overrides (`off`, `warn`, `error`, `hint`)
3. Custom rule definitions with `given` (JSON Path), `then.function` (built-in or JS), and `message`
4. Remote `$ref` resolution for shared rulesets

For v0.1.0, the migration use case is: a team has `extends: [spectral:oas]` with a few severity overrides. This is 80% of Spectral users.

## Decision

v0.1.0 supports:
- `extends: [spectral:oas]` — loads the built-in OAS ruleset (the 15 rules from ADR-006)
- Per-rule severity overrides: `{rule-id}: off | warn | error`
- Nothing else — no custom rules, no remote URL resolution, no JS functions, no `asyncapi` ruleset

Explicitly OUT OF SCOPE for v0.1.0:
- Custom rule definitions (`given`, `then.function`, `message`)
- Remote ruleset URL resolution
- `spectral:asyncapi` ruleset
- JavaScript function rules
- Ruleset inheritance chains (multiple `extends` entries)
- OpenAPI spec `$ref` resolution across files (only inline `$ref` within a single file is resolved)

## Consequences

- Zero-migration for the 80% use case: teams with `extends: [spectral:oas]` + severity overrides get identical behaviour.
- Teams with custom rules see a clear error: "Custom rule definitions are not supported in v0.1.0. Only severity overrides for built-in rules are supported."
- This is a deliberate capability boundary, not a bug. It is documented in the README.

---

# ADR-009: Output Formats — Text (Default) and JSON

**Date**: 2026-04-09
**Status**: Accepted

## Context

CI pipelines consume linter output in two ways: human-readable text (for PR comments, local dev) and machine-readable JSON (for downstream tooling, GitHub Actions annotations, dashboards).

## Decision

Two output formats selected by `--format text|json`:

**Text format** (default):
```
spec.yaml:42:5  error  operation-operationId  Operation must have an operationId.
spec.yaml:18:1  warn   info-contact           Info object should have a contact.
```
Format: `{file}:{line}:{col}  {severity}  {rule-id}  {message}`

ANSI color: `error` = red, `warn` = yellow, `info` = blue. Disabled with `--no-color` or when stdout is not a TTY.

**JSON format**:
```json
{
  "violations": [
    {
      "rule": "operation-operationId",
      "severity": "error",
      "message": "Operation must have an operationId.",
      "path": "/paths/~1foo/get",
      "file": "spec.yaml",
      "line": 42,
      "col": 5
    }
  ],
  "summary": {
    "errors": 1,
    "warnings": 1,
    "total": 2
  }
}
```

## Consequences

- Text format is grep-friendly and compatible with most CI log renderers.
- JSON format enables GitHub Actions problem matchers and downstream dashboards.
- ANSI detection using `std::io::IsTerminal` (stable since Rust 1.70). No additional crate needed.
- `serde_json` (already a dependency) handles JSON serialisation.

---

# ADR-010: No Async Runtime for v0.1.0

**Date**: 2026-04-09
**Status**: Accepted

## Context

The tool's workload is: read one file from disk, parse it, run rules, write output. This is a sequential, CPU-bound workload with a single I/O operation. Adding Tokio or async-std would:
- Add ~500K to binary size
- Add compile time
- Add no user-visible benefit (linting a single spec is not parallelisable in a useful way for v0.1.0)

## Decision

No async runtime. All I/O is synchronous (`std::fs::read_to_string`). The rule engine is synchronous. `main.rs` is a plain synchronous function.

## Consequences

- Binary stays small. Compile times stay short.
- If v0.2.0 adds concurrent multi-file linting or remote `$ref` resolution, Tokio can be added then. The module boundaries are async-ready (all functions are pure, no global state).
