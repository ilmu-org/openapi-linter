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

---

# ADR-011: Position Indexing via yaml-rust2 Two-Pass Scan (v0.2.0)

**Date**: 2026-04-14
**Status**: Accepted
**Supersedes**: none (extends ADR-002 for v0.2.0)

## Context

v0.1.0 deliberately shipped with path-only violation output (no `line`/`col`) because the two-pass scan approach was underspecified at critic-review time. v0.2.0 needs line/col for two reasons:

1. Editor integration (users clicking a CI log jump to the right line).
2. SARIF output (ADR-013) requires `physicalLocation.region.startLine` — it cannot be omitted.

Candidates re-evaluated:

| Crate | Maintained | License | Span support | C FFI | Verdict |
|-------|-----------|---------|--------------|-------|---------|
| `serde_yaml` 0.9 native | Yes | MIT/Apache | No | No (uses pure-Rust `unsafe-libyaml`) | Cannot — no span API |
| `marked-yaml` | No (unmaintained) | — | Yes | No | Rejected per ADR-002 |
| `yaml-rust2` | Yes | MIT/Apache | Yes (`Marker`) | No (pure Rust) | Selected |
| `libyaml-safer` | Yes | MIT | Partial | Wraps C libyaml | Rejected — breaks musl story |
| Custom `serde_json::RawValue` post-scan | N/A | — | Partial (byte offsets only) | No | Complex; no line/col mapping |

`serde_yaml` 0.9 and `yaml-rust2` are **independent parsers that coexist cleanly**. Using both in one binary has no interference: they share no global state and operate on `&str` input. `unsafe-libyaml` (the pure-Rust backend of `serde_yaml`) is not related to C `libyaml` despite the name.

## Decision

For YAML spec files, v0.2.0 uses a two-pass approach:

1. **Pass 1** (existing): `serde_yaml::from_str(content)` produces a `serde_yaml::Value`, which is normalised to `serde_json::Value` for the rule engine. No change to this pass.
2. **Pass 2** (new): `yaml_rust2::parser::Parser::new_from_str(content)` drives an event-based scan. A visitor maintains a stack that tracks:
   - Current JSON Pointer path (built from the alternating key/value state inside `MappingStart`/`MappingEnd` frames, plus a monotonic index inside `SequenceStart`/`SequenceEnd` frames)
   - The `Marker` (line, col) attached to each event
   
   Every time a scalar, mapping-start, or sequence-start event fires, the visitor records `(pointer.clone(), Span { line, col })` into a `HashMap<String, Span>`.

3. `parser::parse()` returns both the `serde_json::Value` document and the `PositionIndex`. Signature becomes:
   ```rust
   pub fn parse(path: &Path) -> Result<(serde_json::Value, PositionIndex), LintError>
   ```
4. `lib::lint()` owns the index and resolves each `Violation`'s `path` → `Span` **after** `rule.check()` returns. Rules do not know about `PositionIndex` — they continue to emit `path: String` only.

For JSON spec files, v0.2.0 ships with `PositionIndex::empty()` (no line/col). SARIF and text output degrade gracefully: text drops the `:line:col` suffix; SARIF emits `region.startLine = 1`. This is explicitly deferred to v0.3.0 (see out_of_scope).

## Consequences

- No new unsafe code, no C FFI, no musl regression. `yaml-rust2` is pure Rust with MIT/Apache licensing.
- `yaml-rust2` adds roughly +300 KB to the binary. Acceptable (budget is ~5 MB).
- The second-pass visitor is the most complex new code in v0.2.0. The implementation must carefully handle:
  - Key-to-value alternation inside mappings (toggle a boolean as each key/value pair completes)
  - Escape of `/` and `~` in JSON Pointer keys (per RFC 6901: `~` → `~0`, `/` → `~1`)
  - Empty document (produces only `StreamStart` and `StreamEnd` events)
  - Anchor/alias resolution — treat an alias node as located where the alias *appears*, not at the anchor
- Ship dedicated unit tests for the position indexer covering nested maps, arrays of objects, JSON Pointer escaping, and missing-path lookups (returns `None`).
- Rules' `check()` signature stays unchanged. This is load-bearing: the rule trait does not need to know about spans.

---

# ADR-012: Flat `line`/`col` Fields on Violation (Not a Location Wrapper)

**Date**: 2026-04-14
**Status**: Accepted
**Supersedes**: none (extends ADR-009 for v0.2.0)

## Context

Adding source position to `Violation` has two possible shapes:

```rust
// Option A (flat):
pub struct Violation {
    pub rule_id: String,
    pub message: String,
    pub severity: Severity,
    pub path: String,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

// Option B (wrapper):
pub struct Location {
    pub path: String,
    pub line: Option<u32>,
    pub col: Option<u32>,
}
pub struct Violation {
    pub rule_id: String,
    pub message: String,
    pub severity: Severity,
    pub location: Location,
}
```

Wrappers justify themselves when either (1) the grouped fields participate in shared invariants, or (2) a type needs to be substituted across multiple contexts. Neither applies:

- The three location fields are independent — `path` is always present, `line`/`col` may be absent (JSON specs, unknown paths).
- There is no second "thing with a location" in the domain.
- SARIF output maps directly: `ruleId ← rule_id`, `level ← severity`, `message.text ← message`, `locations[0].logicalLocations[0].fullyQualifiedName ← path`, `locations[0].physicalLocation.region.startLine ← line`, `startColumn ← col`. A wrapper adds no alignment benefit here.

## Decision

Adopt Option A. Extend `Violation`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct Violation {
    pub rule_id: String,
    pub message: String,
    pub severity: Severity,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
}
```

`#[serde(skip_serializing_if = "Option::is_none")]` keeps the JSON output clean when line/col are absent.

## Consequences

- All 8 existing rules compile unchanged — rules never construct `Violation` with `line`/`col` set. They rely on `Default` or struct update syntax. To keep rules terse, add an inherent constructor `Violation::new(rule_id, message, severity, path)` that leaves `line`/`col` as `None`. `lib::lint()` fills them after the fact from the `PositionIndex`.
- JSON output is backwards-compatible with v0.1.0 consumers when line/col are absent (the fields simply don't appear).
- Text output gains conditional `:line:col` suffix: `{file}:{line}:{col}  ...` when present, `{file}  ...` when absent. Reporter formats the prefix accordingly.
- If a future need arises to pass `Location` around (e.g. pointing into multiple files for multi-file `$ref` resolution), the refactor from flat fields → wrapper is mechanical. YAGNI wins for now.

---

# ADR-013: Reporter API — Batch Signature for Multi-File Correctness

**Date**: 2026-04-14
**Status**: Accepted
**Supersedes**: plan.md proposed `report(violations, spec_path, format, color, out)` signature

## Context

The v0.2.0 plan originally proposed a single-file reporter signature:

```rust
pub fn report(violations: &[Violation], spec_path: &str,
              format: Format, color: ColorMode,
              out: &mut dyn Write) -> std::io::Result<()>
```

This signature works for text and JSON output (caller loops over files, calls `report()` per file). It **breaks for SARIF output**: a SARIF log is a single JSON document containing one `run` with a `results` array that spans all files. Each result's `physicalLocation.artifactLocation.uri` identifies the file. Emitting one SARIF document per file produces invalid SARIF (GitHub Code Scanning expects a single `.sarif` file).

Two resolutions:

- **Option A**: one batch signature that takes all file results at once. Text and JSON iterate internally; SARIF emits one document.
- **Option B**: two entry points — `report()` per-file for text/JSON, `report_sarif()` batch for SARIF.

Option A is cleaner: one function, one signature, one call site in `main.rs`. Single-file linting wraps in a one-element slice. The caller-side complexity of a "sometimes batch, sometimes per-file" API (Option B) is avoided.

## Decision

The v0.2.0 reporter API is:

```rust
pub enum Format { Text, Json, Sarif }
pub enum ColorMode { Auto, Always, Never }

pub fn report(
    files: &[(PathBuf, Vec<Violation>)],
    format: Format,
    color: ColorMode,
    out: &mut dyn Write,
) -> std::io::Result<()>
```

- `ColorMode::Auto` resolves to enabled when `out` points at a TTY **and** `NO_COLOR` env var is unset; otherwise disabled. Resolution happens inside the reporter using `std::io::IsTerminal` on the caller-provided stream (pragmatic limitation: writing to a file handle that wraps stdout still resolves correctly because the caller passes `stdout().lock()` directly).
- **Text output**: per-file iteration; no file-group header (violations already self-identify via the `{file}:{line}:{col}` prefix); trailing summary line after the last file (`N files linted, M violations (E errors, W warnings)`).
- **JSON output**: top-level shape changes to `{ "files": [{"file": "...", "violations": [...]}], "summary": {...} }`. Single-file invocation still produces a one-element `files` array — this is a minor v0.2.0 breaking change vs v0.1.0's `{ "violations": [...], "summary": {...} }`. Acceptable because v0.1.0 is pre-1.0; document in release notes.
- **SARIF output**: one document with one `run`, `tool.driver.rules` derived from the registered rules, `results` spanning all files, `artifacts` array listing each input file.

## Consequences

- This supersedes the plan's proposed signature. Plan architect notes will flag the change.
- `main.rs` calls `report()` exactly once, regardless of single-file or directory-scan invocation. Exit code logic stays in `main.rs`.
- JSON v0.1.0 → v0.2.0 is a minor breaking change. The format bump is documented; consumers who want the v0.1.0 shape can continue on v0.1.x.
- SARIF requires line/col from ADR-011 — not a new dependency, but a hard sequencing constraint: land ADR-011 before ADR-013 in implementation order.
- The `write_text` / `write_json` functions are removed (they were already slated for collapse per v0.1.0 out_of_scope). Internal helpers `write_text_impl`, `write_json_impl`, `write_sarif_impl` live as private functions dispatched from `report()`.

---

# ADR-014: Directory Scanning with walkdir; lint_dir Additive to lint

**Date**: 2026-04-14
**Status**: Accepted

## Context

v0.2.0 adds recursive directory scanning: when the `<spec>` argument is a directory, lint every `.yaml` / `.yml` / `.json` descendant. The architectural questions:

1. Does `lib::lint()` absorb this capability, or is it a new function?
2. What dependency handles traversal?
3. What happens when one file fails to parse — abort or continue?
4. Where does the exit-code logic live?

## Decision

**API shape**:

```rust
// Unchanged — atomic single-file operation.
pub fn lint(spec_path: &Path, ruleset_path: Option<&Path>)
    -> Result<Vec<Violation>, LintError>;

// New — recursive directory scan.
pub fn lint_dir(dir_path: &Path, ruleset_path: Option<&Path>)
    -> Result<Vec<(PathBuf, Result<Vec<Violation>, LintError>)>, LintError>;
```

`lint_dir` returns a per-file `Result` inside the outer `Ok`. Outer `Err` is reserved for directory-level failures (the path is not a directory, the ruleset itself failed to load, I/O error on the directory handle). Per-file parse failures become `Err` entries in the inner tuple — they do **not** abort the scan.

**Dependency**: `walkdir = "2"` — MIT/Apache, pure Rust, no transitive C deps, the de-facto standard for recursive filesystem traversal in Rust. Confirmed musl-safe.

**File selection**: `walkdir` iterates all files; filter by extension (`.yaml`, `.yml`, `.json`, case-insensitive). Symlink behaviour: follow by default (matches `find` and most linters); do not descend into cycles (walkdir handles this). `.git/` and `node_modules/` directories are **not** special-cased in v0.2.0 — users who want to exclude them can point the linter at a subdirectory. Add `--ignore <glob>` only if user feedback demands it in v0.3.0.

**Error resilience**: if a file parses but fails OpenAPI version detection (not a valid OpenAPI spec, just a random YAML/JSON file), emit a stderr warning and record an `Err(LintError::InvalidSpec(...))` for that file. `lint_dir` continues. This matches Spectral's behaviour.

**Exit code logic stays in `main.rs`**:
- `0` — all files clean
- `1` — at least one file had violations
- `2` — at least one file failed to parse OR the directory-level operation failed

Lib returns structured data; main decides policy. This keeps `lib` pure and testable without `std::process::exit` entanglement.

## Consequences

- `main.rs` grows a small dispatch: if `cli.spec` is a directory, call `lint_dir`; else `lint`. Both paths collect results into the `&[(PathBuf, Vec<Violation>)]` shape the reporter expects.
- No async runtime needed — synchronous walkdir traversal completes in sub-second time for typical monorepos (<1000 specs). If benchmarks show a bottleneck in v0.3.0, `rayon::par_bridge` over the walkdir iterator is a drop-in parallelisation (see ADR-010 note).
- Symlink-following is a defensible default but can surprise users with wide-symlinked workspaces. Document in README; revisit if issues arise.
- The `Result<Vec<(PathBuf, Result<...>)>, LintError>` return type is slightly unusual. A dedicated `ScanReport` newtype could replace it in v0.3.0 if ergonomics need it, but the tuple shape is explicit and easy to pattern-match.

---

# ADR-015: Document-Internal `$ref` Resolution Utility (Prerequisite for path-params)

**Date**: 2026-04-14
**Status**: Accepted

## Context

The `path-params` rule must cross-reference path template tokens (e.g. `{petId}` in `/pets/{petId}`) with parameter objects declared in the operation or at the path level. For OAS 2.x parameters live under `paths.{path}.{method}.parameters` (operation) and `paths.{path}.parameters` (path-level). For OAS 3.x the structure is identical.

**The trap**: in real-world OAS 3 specs, parameter arrays overwhelmingly contain JSON Pointer references to shared parameter components:

```yaml
paths:
  /pets/{petId}:
    get:
      parameters:
        - $ref: '#/components/parameters/PetId'
      responses: ...
components:
  parameters:
    PetId:
      name: petId
      in: path
      required: true
```

Without resolving these document-internal `$ref`s, the `path-params` rule will see an object with only `{"$ref": "..."}` and no `name: petId`, firing a false-positive violation on virtually every well-structured spec. This is unacceptable behaviour.

Multi-file `$ref` resolution (following relative file paths across the filesystem) remains out of scope for v0.2.0 (per plan.md out_of_scope). Document-internal `$ref`s (starting with `#/`) are mandatory.

## Decision

Add a shared utility in `src/rules/util.rs`:

```rust
/// Resolve a document-internal JSON Pointer reference.
/// Accepts the form `#/components/parameters/PetId` or `#`.
/// Returns None for external refs (any ref not starting with `#/` or equal to `#`),
/// unresolvable pointers, or malformed input.
pub(crate) fn resolve_internal_ref<'a>(
    doc: &'a serde_json::Value,
    ref_str: &str,
) -> Option<&'a serde_json::Value>;

/// If `value` is an object of exactly `{"$ref": "#/..."}`, resolve it.
/// Otherwise return `value` unchanged. Follows chains up to a fixed depth
/// (default: 16) to prevent cycles.
pub(crate) fn deref<'a>(
    doc: &'a serde_json::Value,
    value: &'a serde_json::Value,
) -> &'a serde_json::Value;
```

`resolve_internal_ref` implements RFC 6901 JSON Pointer semantics (handling `~0` and `~1` escapes). `deref` is the primary API that rules call.

`path-params` uses `deref` on every parameter object before reading `name` and `in`. If `deref` returns an object that still contains `$ref` (external ref, unresolvable), the rule treats it as opaque and skips matching against it (avoids false positives on external refs). If no parameter in the resolved set matches a path token with `in: path`, the rule emits a violation.

The utility is `pub(crate)` — not part of the public API. It is available to future rules (e.g. `reference-components`, `no-unresolved-refs`) without forcing a crate-boundary decision now.

## Consequences

- `path-params` correctness becomes tractable. Without this utility, the rule ships broken.
- `deref` does not mutate the document — it returns a reference. No clone cost on the hot path.
- Cycle detection via fixed-depth recursion is pragmatic; OpenAPI specs almost never chain `$ref` more than 2-3 levels.
- External `$ref` handling is intentionally permissive: skip, don't error. Spectral's behaviour is to follow external refs when possible; since v0.2.0 does not support multi-file resolution, skipping is the only correct choice.
- Future `$ref`-heavy rules inherit the utility for free. If v0.3.0 adds multi-file `$ref`, this utility evolves (or is joined by a sibling) — the API shape is small and easy to extend.

---

# ADR-016: mod.rs → Named Module Rename Confirms Rust 2018+ Pattern

**Date**: 2026-04-14
**Status**: Accepted

## Context

v0.1.0 out_of_scope flagged the `mod.rs` → named module rename as a first-commit task on the v0.2.0 branch. Confirming the rename is mechanically correct before implementation begins.

## Decision

Rename pattern (applied to `parser`, `model`, `rules`, `ruleset`, `reporter`):

```
Before:                          After:
src/rules/mod.rs                 src/rules.rs
src/rules/operation_tags.rs  →   src/rules/operation_tags.rs  (unchanged)
src/rules/info_contact.rs        src/rules/info_contact.rs    (unchanged)
...
```

Rust 2018+ (and Edition 2024) supports `src/rules.rs` coexisting with a `src/rules/` directory that holds submodules. No change to `mod` declarations inside `src/rules.rs` (which was formerly `mod.rs`). No change to `src/lib.rs` module declarations (`pub mod rules;` works identically).

`src/error.rs` is already a named module (not `src/error/mod.rs`) — no change needed.

The rename is a single mechanical commit, landed first on the v0.2.0 branch before any feature work. It touches five files (`mod.rs` → `<name>.rs`). `cargo build` must pass identically before and after.

## Consequences

- Removes the `mod.rs` ambiguity as the codebase grows (v0.2.0 adds `src/rules/util.rs` and at least 7 more rule files; the named-module style makes file navigation in editors materially clearer).
- No user-visible change. No API change. No binary-size change.
- First-commit discipline: the rename should not be bundled with feature work. A failing test after feature work commits must not be chased through a rename diff.


---

# ADR-017: Project Rename — openapi-linter → refract

**Date**: 2026-04-14
**Status**: Accepted

## Context

The project shipped v0.1.0 and v0.2.0 under the name `openapi-linter`. The name is descriptive but generic, indistinguishable from dozens of other linters, and does not convey brand identity. The crate has never been published to crates.io, so the rename window is still clean with no downstream breakage.

`refract` was chosen because it evokes the physical act of bending a spectrum — a deliberate nod to Spectral, the Node.js tool this project replaces — and it reads cleanly as a CLI: `refract lint api.yaml`. The name is short, memorable, and domain-appropriate.

## Decision

Rename the project from `openapi-linter` to `refract` at the pre-v0.3.0 stage while reference debt is small (two shipped releases, no crates.io publication).

The `refract` name on crates.io is held by a 0.0.0 placeholder whose description explicitly invites contact ("If you want this package name please contact me."). To avoid blocking the rename, the **crate name** on crates.io is `refract-cli`; the **binary name** users invoke remains `refract`. An explicit `[[bin]]` section in `Cargo.toml` decouples the two. The owner can be contacted in parallel; if the name transfers before first publish we can change the crate name to plain `refract`.

## Migration path for existing users

- GitHub auto-redirects `ilmu-org/openapi-linter` to `ilmu-org/refract` after the repo rename.
- Local git remotes must be updated manually: `git remote set-url origin git@github.com:ilmu-org/refract.git`
- CI configs referencing the old binary name (`openapi-linter`) must update to `refract`. The binary name change is a breaking change; v0.2.0 is pre-1.0 so semver permits it.
- The `refract-cli` crate name on crates.io is the first-time publish name — no existing downstream depends on it.

## Consequences

- Brand identity established before public launch.
- Binary name changes: `openapi-linter` → `refract`. Breaking for existing CI pipelines, acceptable at pre-1.0.
- Crate name `refract-cli` differs from binary name `refract`. Minor friction for Rust library consumers; users installing via `cargo install refract-cli` get the `refract` binary as expected.
- README carries a "Renamed from openapi-linter" breadcrumb until v1.0.0.

---

<!-- v0.3.0 scope summary
In scope (17 rules): operation-success-response, array-items, no-$ref-siblings, oas3-api-servers, path-keys-no-trailing-slash, path-not-include-query, path-declarations-must-exist, operation-tag-defined, openapi-tags-uniqueness, tag-description, duplicated-entry-in-enum, typed-enum, oas3-server-trailing-slash, oas3-server-not-example.com, oas3-parameter-description, operation-operationId-valid-in-url, operation-parameters.
Deferred to v0.4.0: oas3-schema, oas2-schema, oas3-valid-schema-example, oas2-valid-schema-example (full JSON Schema validation), cross-file $ref resolution.
Architectural additions: OAS-version gating helper, deref-before-compare invariant for all cross-reference rules.
-->

# ADR-018: v0.3.0 Rule Set, 17 Structural and Correctness Rules

**Date**: 2026-04-14
**Status**: Accepted

## Context

v0.2.0 shipped 15 rules covering operation metadata, info block, tags, path parameters, and markdown safety. The remaining Spectral OAS gap is split between two classes of rule:

1. Structural and correctness rules that need only `serde_json::Value` traversal plus the existing internal `$ref` deref utility from ADR-015. Examples: `array-items` requires `type: array` to declare `items`; `no-$ref-siblings` enforces an OAS spec invariant; tag and parameter sanity checks fall here.
2. Schema-evaluation rules (`oas3-schema`, `oas2-schema`, `oas3-valid-schema-example`, `oas2-valid-schema-example`) that require a JSON Schema evaluator capable of running the bundled OAS JSON Schema (or fragments of it) against an input document.

Class 1 rules are mechanically similar to v0.2.0 work: visit nodes, evaluate predicate, emit violation. Class 2 rules introduce a new dependency, a new failure mode (schema evaluation errors vs lint violations), and significant binary growth (see ADR-019).

A single milestone covering both classes would be a 30-plus rule release with a dependency-introduction risk in the same window. Splitting the milestones isolates the dependency decision and keeps v0.3.0 reviewable.

## Decision

v0.3.0 ships exactly the 17 class-1 rules listed below. No schema-evaluation rules. No cross-file `$ref` resolution (see ADR-020).

| Rule | Severity | Scope notes |
|------|----------|-------------|
| operation-success-response | warn | Every operation has at least one 2xx or 3xx response. Default response counts as a non-success fallback, not a success. |
| array-items | error | Any schema with `type: array` must declare `items`. Apply after deref. Skip schemas behind unresolvable external `$ref`. |
| no-$ref-siblings | error | An object containing `$ref` must contain no other keys. OAS 3.1 relaxes this for some keywords; v0.3.0 enforces the strict rule (matches Spectral default). |
| oas3-api-servers | warn | OAS 3.x only. Top-level `servers` array is present and non-empty. |
| path-keys-no-trailing-slash | warn | Path keys do not end in `/` except the root key `/`. |
| path-not-include-query | warn | Path keys do not contain `?`. |
| path-declarations-must-exist | warn | No empty `{}` placeholders in path templates. |
| operation-tag-defined | warn | Every tag string used on an operation appears in the global `tags` array. |
| openapi-tags-uniqueness | error | Global tag `name` values are unique. |
| tag-description | warn | Every global tag has a non-empty `description`. |
| duplicated-entry-in-enum | warn | `enum` arrays contain no duplicate values (deep equality). |
| typed-enum | warn | Each `enum` value matches the declared schema `type`. See ADR-021 for type-coercion semantics. |
| oas3-server-trailing-slash | warn | OAS 3.x only. Server `url` does not end with `/`. |
| oas3-server-not-example.com | warn | OAS 3.x only. Server `url` host is not `example.com`. |
| oas3-parameter-description | warn | OAS 3.x only. Every parameter (after deref) has a non-empty `description`. |
| operation-operationId-valid-in-url | warn | `operationId` matches `^[A-Za-z0-9-._~:/?#[\]@!$&'()*+,;=]+$` (RFC 3986 unreserved + sub-delims plus path-safe set). |
| operation-parameters | warn | Within a single operation, no two parameters share the same `(name, in)` pair after deref. Path-level + operation-level parameters are merged before comparison. |

Of these, 5 rules (oas3-api-servers, oas3-server-trailing-slash, oas3-server-not-example.com, oas3-parameter-description, no-$ref-siblings on certain OAS 3.1 keywords) are version-gated. ADR-021 covers the gating helper.

Of these, 4 rules consume `$ref` deref (array-items, oas3-parameter-description, operation-parameters, operation-tag-defined when tags are referenced indirectly). ADR-021 codifies the deref-before-compare invariant.

## Consequences

- v0.3.0 ships 32 total built-in rules (15 from v0.2.0 plus 17 here). Strong Spectral-replacement story for structural correctness.
- No new direct dependency required. Binary size stays approximately flat.
- The four schema-evaluation rules and cross-file `$ref` are the next milestone's frontier (ADR-019, ADR-020). Users needing those today must run a Spectral fallback step for now; document this in README.
- Rule registration follows the v0.2.0 module-per-rule pattern. Rule count growth is linear with file count, no compile-time impact.
- 17 rules is at the upper edge of a reviewable milestone. Implementation order should pull the structural-only rules first (path-keys-no-trailing-slash, path-not-include-query, path-declarations-must-exist, openapi-tags-uniqueness, tag-description, oas3-server-trailing-slash, oas3-server-not-example.com, no-$ref-siblings, oas3-api-servers) before the deref-dependent ones (array-items, oas3-parameter-description, operation-parameters, operation-tag-defined) and the type-aware ones (typed-enum, duplicated-entry-in-enum). Sequencing details belong in plan.md.

---

# ADR-019: JSON Schema Validation Rules Deferred to v0.4.0

**Date**: 2026-04-14
**Status**: Accepted

## Context

Four Spectral rules require a JSON Schema evaluator:

- `oas3-schema`, `oas2-schema`: validate the entire OAS document against the official OAS JSON Schema (a 2000-plus-line schema document for OAS 3.0, plus separate variants for 3.1 and 2.0).
- `oas3-valid-schema-example`, `oas2-valid-schema-example`: validate every `example` value against its declaring schema.

These rules deliver substantial value: they catch entire classes of bug (a field declared `format: int32` with example `"abc"`; a request body schema missing a required keyword) that the structural rules cannot. They also introduce architectural cost.

**Library options assessed:**

1. **boon** (current latest 0.6.x, MIT, pure Rust). Implements JSON Schema drafts 4, 6, 7, 2019-09, 2020-12. Pure Rust, no native dependencies, musl-clean. Adds approximately 250 KB to the stripped release binary plus the schema documents themselves (the OAS 3.0 schema is approximately 60 KB JSON, OAS 3.1 approximately 80 KB, OAS 2.0 approximately 90 KB). Binary grows by approximately 0.5 MB total. Maintained, stable API.
2. **jsonschema** (most popular crate). Pulls `fancy-regex` and `regex` for `pattern` keyword support, plus `url` for `format: uri`. Dependency surface roughly 3x boon. Has had musl build issues historically. Larger binary impact.
3. **valico** (legacy). Drafts 4 only, no 2020-12. Out.
4. Hand-roll. Implementing JSON Schema 2020-12 evaluation is a multi-month project. Out.

**Architectural friction beyond binary size:**

- Schema evaluation produces nested error trees (a single `oas3-schema` failure on a malformed document yields dozens of leaf errors). Mapping these to flat `Violation`s with sensible `path` and `message` fields requires a dedicated translator. Spectral's translator is non-trivial; refract would need its own.
- The OAS JSON Schema must be bundled as a compile-time asset. Choosing how (a `static` byte string per OAS version, lazy-parsed; or an `include_bytes!` plus `Lazy<Value>`; or a build-script-generated module) is a real design choice with binary-size implications.
- `oas3-valid-schema-example` and `oas2-valid-schema-example` apply schema evaluation to user-defined sub-schemas, not the OAS schema. The evaluator must accept arbitrary schemas at runtime, which boon supports but the integration shape (one `boon::Schemas` registry per `lint()` call vs per rule invocation) is a non-trivial decision.
- Error-vs-violation distinction: a malformed schema in the input document that boon refuses to compile is neither a clean lint pass nor a structural violation. The reporter contract must define this state.

Doing all four rules and resolving the four design questions in v0.3.0 doubles the milestone size and concentrates risk: if the schema-evaluation integration slips, it blocks the 17 structural rules that are otherwise ready.

## Decision

Defer all four schema-evaluation rules to v0.4.0. Defer the boon (or alternative) dependency choice to that milestone. v0.4.0's first ADR will be the evaluator selection and integration shape.

v0.3.0 ships no JSON Schema dependency. The v0.3.0 README and CHANGELOG must call out that `oas3-schema` and `*-valid-schema-example` are not yet implemented; users needing them today should chain a Spectral pre-check or wait for v0.4.0.

## Consequences

- v0.3.0 binary size stays approximately flat versus v0.2.0.
- v0.3.0 ships in a reviewable window. Schema-evaluation work is isolated to v0.4.0 where it can have full architect attention.
- Users who depend on full document validation today have a clear gap to plan around. Docs must be honest about this.
- The boon assessment recorded above is the starting point for v0.4.0's first ADR. If a better library appears in the interim (or the OASIS-blessed JSON Schema 2020-12 reference Rust impl matures), v0.4.0 reassesses.
- Once schema validation lands, refract crosses a capability threshold where it can replace Spectral for the majority of Stoplight-style use cases. v0.4.0 is therefore the milestone that earns a 1.0.0 candidate label.

---

# ADR-020: Cross-File $ref Resolution Deferred to v0.4.0

**Date**: 2026-04-14
**Status**: Accepted

## Context

ADR-015 shipped document-internal `$ref` resolution (refs of the form `#/components/...`). Cross-file `$ref` (refs of the form `./shared/parameters.yaml#/PetId` or `../common.json`) remained out of scope for v0.2.0 and is reconsidered now.

**What cross-file resolution would require:**

- A resolver that, given a base file path and a `$ref` string, loads the target file (parsing YAML or JSON), caches it, and returns a `serde_json::Value` plus the parsed JSON Pointer fragment.
- Cycle detection across file boundaries (file A refs file B refs file A).
- Path semantics: relative path resolution against the current file, handling `..`, symlinks, Windows path separators, and case-sensitivity differences across filesystems.
- A loader cache shared across the lint run (multiple rules and multiple operations may deref to the same external file).
- Failure modes: missing file, malformed file, pointer-not-found, cycle exceeded. Each maps to either a lint violation (when the rule's invariant is broken) or a `LintError` (when the document graph itself is broken).
- HTTP `$ref` (`https://example.com/shared.yaml#/...`) is not in scope at any milestone for refract (no network in a CI linter). Resolver must reject these explicitly.

**Impact on v0.3.0 rules:**

Of the 17 v0.3.0 rules, 4 consume `$ref` deref. With internal-only deref:

- `array-items` on a schema behind an external `$ref` is treated as opaque, no violation, no false positive. Acceptable: most arrays-without-items mistakes are in inline schemas.
- `oas3-parameter-description` on a parameter behind an external `$ref` is skipped. Acceptable: the parameter component file itself can be linted directly when scanned.
- `operation-parameters` (uniqueness of (name, in)) compares deref'd parameters. External-ref'd parameters compare by their `$ref` string, which means two operations referencing the same external parameter are correctly seen as identical, but two different external refs that happen to resolve to the same name+in would not be caught. Acceptable: rare in practice.
- `operation-tag-defined` compares string tag values directly, no deref needed.

The degradation is bounded: rules produce no false positives on external-ref'd nodes, only false negatives. The lint signal stays trustworthy.

**Why defer:**

The user population most affected by external `$ref` is teams with bundled spec workflows (Redocly, Stoplight Studio, swagger-cli bundle). Those teams typically lint the bundled output, where every ref is internal. Teams that lint un-bundled multi-file specs are a minority. Shipping cross-file resolution is a multi-week effort for a minority use case in v0.3.0; it competes for the same architect and developer attention as the 17 new rules.

Pairing cross-file `$ref` with schema validation (v0.4.0) is also natural: `oas3-schema` cannot validate a document containing unresolved external refs without first resolving them.

## Decision

Defer cross-file `$ref` resolution to v0.4.0. The internal-only `deref` utility from ADR-015 remains the contract for v0.3.0 rules.

External `$ref` handling stays permissive in v0.3.0: when `deref` encounters a non-internal `$ref` (any string not starting with `#/` or equal to `#`), it returns the original `{"$ref": "..."}` object unchanged, and the calling rule treats it as opaque (no violation, no inspection of fields it would expect post-deref).

The v0.3.0 README must document this behaviour explicitly: "Cross-file `$ref` is not resolved in v0.3.0. Bundle multi-file specs with `redocly bundle` (or equivalent) before linting for full coverage." A v0.4.0 milestone link should accompany.

## Consequences

- No new resolver, loader cache, or filesystem-traversal code in v0.3.0. Risk surface stays small.
- Bundled-spec workflows (the majority) are fully covered.
- Multi-file workflows hit a documented limitation, not a silent failure or false positive.
- v0.4.0 owns the resolver design alongside schema validation. Both depend on a "fully resolved document" abstraction; building both in the same milestone lets the abstraction emerge from real use, not speculation.
- The deref contract (return-unchanged on external) is now a two-version-stable invariant. Rules written in v0.3.0 will not need to change when v0.4.0 lands cross-file resolution: deref will simply succeed on more inputs.

---

# ADR-021: OAS-Version Gating Helper and Deref-Before-Compare Invariant

**Date**: 2026-04-14
**Status**: Accepted

## Context

v0.3.0 introduces two cross-cutting structural concerns that span multiple rules and need to be settled once rather than per rule.

**Concern 1: OAS-version gating.** Five v0.3.0 rules apply only to OAS 3.x specs (oas3-api-servers, oas3-server-trailing-slash, oas3-server-not-example.com, oas3-parameter-description, plus the strict form of no-$ref-siblings). Existing v0.2.0 rules already navigate the 2.x vs 3.x shape difference internally per rule, which has worked but creates duplicated detection logic. Adding 5 more 3.x-only rules without a shared helper would compound the duplication.

**Concern 2: Deref-before-compare.** Four v0.3.0 rules (array-items, oas3-parameter-description, operation-parameters, operation-tag-defined) cross-reference nodes that may be `$ref` objects. The correctness invariant is: every comparison or field access on a potentially-ref'd node must call `deref` first. Forgetting to deref produces false positives. This is the single most likely correctness bug class in v0.3.0.

**Concern 3 (related): typed-enum coercion semantics.** `typed-enum` checks that each enum value matches the declared `type`. JSON has six primitive types (string, number, integer, boolean, null, array, object). Strict comparison rejects `enum: [1, 2, 3]` under `type: integer` because `serde_json::Value::Number` is not split on integer-vs-float. The rule needs documented coercion semantics or it ships broken.

## Decision

**Version gating helper.** Add a small helper to `src/rules/util.rs`:

```rust
pub(crate) enum OasVersion {
    V2,
    V3_0,
    V3_1,
    Unknown,
}

pub(crate) fn detect_oas_version(doc: &serde_json::Value) -> OasVersion;
```

Detection logic: if `doc.swagger == "2.0"`, return `V2`. If `doc.openapi` starts with `"3.0"`, return `V3_0`. If `doc.openapi` starts with `"3.1"`, return `V3_1`. Otherwise `Unknown`.

OAS 3.x-only rules guard their `check()` body with `matches!(detect_oas_version(doc), OasVersion::V3_0 | OasVersion::V3_1)` and return early on mismatch. Existing v0.2.0 rules that internally distinguish 2.x vs 3.x are not refactored in v0.3.0 (separate cleanup PR if appetite exists).

The Rule trait stays unchanged. Version gating is a per-rule guard, not a registration-time filter, because some rules apply to both versions with version-specific branches inside.

**Deref-before-compare invariant.** Codify in `src/rules/util.rs` as a doc-comment contract on `deref`:

> Any rule that cross-references a node which may legally be a `$ref` object MUST call `deref` before reading non-`$ref` fields or comparing the node against another. Skipping `deref` produces false positives on every well-structured spec.

Each of the four affected rules carries an inline comment identifying it as deref-dependent and links to ADR-021. Reviewer checklist (in PR template) gains a line: "Does this rule access fields on a node that could be a `$ref`? If yes, is `deref` called first?"

No type-system enforcement is added in v0.3.0. A `Deref'd<'a>(&'a Value)` newtype was considered but rejected: it forces a wrapper through the entire rule body for marginal safety, complicates the existing rule signatures, and the invariant is enforceable by review for 4 rules. Revisit if v0.4.0 (with cross-file `$ref` and schema evaluation) brings the deref-dependent rule count above 8.

**typed-enum coercion semantics.** `typed-enum` treats `type: integer` and `type: number` as compatible with any `serde_json::Value::Number`, with one extra check for `type: integer`: the number must satisfy `n.is_i64() || n.is_u64() || (n.as_f64().map_or(false, |f| f.fract() == 0.0))`. Rationale: YAML and JSON both round-trip integers as numbers, and rejecting `1.0` under `type: integer` would surprise users. This matches Spectral's lenient behaviour and avoids JSON-vs-YAML representation gotchas.

`type: string` matches `Value::String`. `type: boolean` matches `Value::Bool`. `type: null` matches `Value::Null`. `type: array` matches `Value::Array`. `type: object` matches `Value::Object`. Multi-type (`type: ["string", "null"]`, OAS 3.1) passes if any listed type matches.

## Consequences

- 5 version-gated rules share one detection path. Future OAS 3.2 (when ratified) extends `OasVersion` and rules opt in by pattern match.
- Deref-before-compare contract is documented in code, in ADR, and in PR template. No type-system cost. Re-evaluated if rule count grows.
- typed-enum lands with documented coercion semantics, no surprise rejection of integer-shaped floats. Test fixtures cover `[1, 2, 3]` and `[1.0, 2.0]` under `type: integer`, plus mixed-type and OAS 3.1 multi-type cases.
- `src/rules/util.rs` grows from one (`deref`, `resolve_internal_ref`) to two concerns (add `detect_oas_version`, `OasVersion`). Still single-file. If util.rs exceeds approximately 300 lines, split in v0.4.0 along concern boundaries.

---

<!-- Critic Review, v0.3.0 Scope
Author: rust-critic
Date: 2026-04-14
Subject: ADR-018 through ADR-021, rule scope and cross-cutting invariants
Verdict: Approve with required addresses on CRITICAL issues before plan.md finalization.
-->

# Critic Review, v0.3.0 Scope

## Critical Issues (must block scope finalization)

### C1. `no-$ref-siblings` scope under OAS 3.1 is ambiguous and likely wrong
ADR-018 row: "OAS 3.1 relaxes this for some keywords; v0.3.0 enforces the strict rule (matches Spectral default)." ADR-021 repeats "the strict form of no-$ref-siblings" as a version-gated concern.

Two problems:
1. Spectral's upstream `no-$ref-siblings` is `formats: [oas2, oas3_0]`, i.e. it is not applied to OAS 3.1 at all. Claiming "matches Spectral default" while enforcing strict on 3.1 contradicts Spectral.
2. OAS 3.1 uses JSON Schema 2020-12, which explicitly permits `$ref` alongside other keywords inside Schema Objects. Even Reference Objects in OAS 3.1 allow `summary` and `description` siblings. A strict rule applied to a 3.1 schema tree produces false positives on virtually every real 3.1 spec.

Ambiguity: the ADR does not say where the rule traverses. Does it run on every object containing `$ref`? Only Path Item references? Only Reference Objects outside schemas? Until that surface is defined, plan.md cannot write a fixture matrix.

Required action: decide explicitly (a) whether the rule is skipped on OAS 3.1 outright (Spectral parity), and (b) which object positions it scans on 2.x and 3.0. The answer belongs in ADR-018 or a revision, not plan.md.

### C2. ADR-code drift: `deref(doc, value)` function does not exist as described
ADR-021 places a doc-comment contract on `deref`. ADR-015 describes `deref(doc, value) -> &Value` as the primary API. The actual code in `src/rules/util.rs` ships `resolve_ref(doc, pointer, depth) -> Option<&Value>`, no `deref` wrapper, different signature, different return type (Option vs bare reference).

Consequences:
- Developer implementing v0.3.0 rules cannot locate the function the ADR points to.
- "Inline comments on affected rules linking ADR-021" has nothing to link against since the call-site API is different.
- The "return the original $ref object unchanged" contract from ADR-020 does not match `resolve_ref` which returns `None` on external refs.

Required action: pick one. Either (a) ADR-021 clarifies the invariant applies to `resolve_ref` and documents the Option-based contract, or (b) v0.3.0 adds a thin `deref` wrapper matching the ADR text. Plan.md must know which before phase 2.

## Significant Issues (architect or SDD must address)

### S1. Unsupported rule IDs in user `.spectral.yaml`, behavior undefined
v0.3.0 claims Spectral-compatible rulesets but does not implement `oas3-schema`, `oas2-schema`, `oas3-valid-schema-example`, `oas2-valid-schema-example`. Most real `.spectral.yaml` files extend `spectral:oas`, which pulls these rules in by default.

Behavior when the user's ruleset references a rule refract does not know:
- Hard error: every existing Spectral user gets a broken lint until they edit their ruleset. Unacceptable migration friction.
- Silent drop: user believes full Spectral coverage ran, gets false confidence.
- Warn-and-continue: probably correct. But this is a UX decision that belongs in an ADR, not left to discovery during implementation.

Required action: pick a behavior. Document in ADR (revise ADR-019 or add a new one). Affects README migration notes too.

### S2. `path-declarations-must-exist` detection is unspecified
ADR-018: "No empty `{}` placeholders in path templates." The user-facing question is exactly the detection logic: how does the rule tell `{}` from `{petId}`? Edge cases:
- `/pets/{ }` (whitespace only): empty or not?
- `/pets/{petId{inner}}` (malformed, unclosed nested): regex choice matters.
- `/pets/{petId` (unclosed): rule behavior unspecified.
- Path keys containing literal `{`/`}` outside template positions.

Architect may argue this is plan.md territory. Counter: if the rule is defined as "scan for `\{\s*\}`" versus "tokenize path template and inspect each segment," the two approaches disagree on the edge cases above. That is a scoping decision, not a plan detail.

Required action: pick regex or tokenizer and record the decision (can be a short note in ADR-018, not a new ADR).

### S3. Deref-before-compare enforced by PR template alone is fragile
Architect acknowledges the type-safety tradeoff and defers a `Deref'd<'a>` newtype. Defensible at 4 rules. Two concerns:
- PR template checklists are ignored when PRs are AI-drafted (common on this project). The "reviewer catches it" assumption assumes human reviewer in the loop.
- v0.4.0 brings schema validation and cross-file $ref, both deref-heavy. The 8-rule re-evaluation trigger is reached suddenly, not gradually.

Cheaper mitigation than a newtype: add a single unit test in `rules/util.rs` that loads a fixture with an internal `$ref` and asserts each deref-tagged rule produces the expected violation count. If a developer forgets deref, the count diverges, test fails. Zero runtime cost, faster feedback than PR review.

Not blocking v0.3.0 scope, but worth capturing in the ADR as the escalation path before the newtype.

### S4. `typed-enum` coercion fixtures incomplete for stated semantics
`n.is_i64() || n.is_u64() || n.as_f64().map_or(false, |f| f.fract() == 0.0)` covers the common case but has surface area:
- `1e30`: `fract() == 0.0` is true, but value is not i64-representable. Passes. Defensible (matches Spectral) but worth an explicit fixture so behavior is frozen.
- `-0.0`: `fract() == 0.0` is true. Treated as integer. Fine, worth a fixture.
- `f64::NAN`: `NaN.fract() == NaN`, comparison with 0.0 yields false, rejected. Fine.
- `f64::INFINITY`: `INFINITY.fract() == NaN`. Rejected. Fine.

Plan.md should add these four fixtures to the typed-enum matrix explicitly, not rely on `[1, 2, 3]` and `[1.0, 2.0]` alone.

### S5. `OasVersion::Unknown` silently disables version-gated rules
`detect_oas_version` returns `Unknown` for anything that isn't exactly `"2.0"`, `"3.0.*"`, `"3.1.*"`. When OAS 3.2 or a pre-release variant (`"3.1.0-rc1"`) appears, all 5 version-gated rules skip silently. User lints a new-version spec, sees clean output, assumes coverage.

Required action: decide whether `Unknown` should log a diagnostic (one-time per lint run) so users know gating fired. Cheap to add, prevents the silent-coverage-gap failure mode. Can be a note in ADR-021.

### S6. `operation-parameters` dedup semantics across internal + external $ref mix
Architect addresses this partially in ADR-020: external refs compare as opaque, "two different external refs that happen to resolve to the same name+in would not be caught." OK. Two cases still unspec'd:
- An array mixing `[{$ref: '#/components/parameters/X'}, {name: 'x', in: 'query'}]` where `X` resolves to the inline object. Deref makes both yield `{name: 'x', in: 'query'}`, dedup fires correctly. Confirm in fixture.
- Path-level + operation-level param merge: ADR-018 says "merged before comparison." OAS rule is operation-level overrides path-level with matching `(name, in)`. Does refract's merge emit the overridden path-level copy (leading to a false-positive duplicate) or drop it?

Required action: state the merge semantics in ADR-018 (one line) so plan.md knows what to fixture.

## Minor Issues

### M1. `operation-operationId-valid-in-url` regex is permissive to the point of triviality
The char class `[A-Za-z0-9-._~:/?#[\]@!$&'()*+,;=]+` permits `?`, `#`, `/`, `[`, `]`. In practice the rule fires only on whitespace and non-ASCII. This matches Spectral verbatim, so compat is preserved, but users will be surprised that `opId/with/slashes?and=query#frag` passes. Worth a one-line doc note: "matches Spectral's permissive default; catches whitespace and non-URL-safe characters only."

### M2. `detect_oas_version` ambiguity when both `swagger` and `openapi` fields present
Malformed doc with both fields: first-match wins (checks `swagger == "2.0"` first). Real-world rare but worth one line in ADR-021 so the behavior is frozen.

### M3. 17-rule milestone size
Architect acknowledges and sequences. One extra guardrail for plan.md: each of the 4 phases should be a separate PR, not a single mega-PR. Single-commit phases are fine within a PR, but collapsing all 17 rules into one review is the failure mode the architect flagged.

## Strengths

- **Class-split rationale is clean.** Separating structural rules from schema-evaluation rules puts the boon dependency decision in its own milestone. Reviewable chunks, one risk at a time.
- **Boon assessment is evidence-based.** Three candidates evaluated, rejection reasons explicit, binary cost quantified, architectural friction beyond binary size called out. This is exactly the depth a deferral decision needs.
- **Bounded-degradation argument on external $ref is rigorous.** "No false positives, only false negatives" is the right framing. Lint signal stays trustworthy, which is more important than coverage breadth for a linter.
- **Deref-before-compare escalation plan is named.** Architect resisted the newtype at 4 rules but committed to re-evaluate at 8. The trigger is concrete, not hand-waved.
- **typed-enum coercion rationale is documented with why.** YAML round-trip explanation is the right level of detail; prevents relitigation.
- **4-phase sequencing derisks the 17-rule count.** Structural first, then util.rs additions, then deref-dependent, then type-aware. If a phase stalls, earlier phases still ship value.
- **Version gating via `matches!` pattern is idiomatic and cheap.** No Rule-trait surgery, no registration-time filter, extends naturally to OAS 3.2.

## Questions for Architect

1. **C1 resolution:** does `no-$ref-siblings` run on OAS 3.1 at all? If yes, on which object positions? If no, ADR-018 row should say "2.x and 3.0 only" and drop from ADR-021's version-gated list (it would be format-gated, not version-gated).
2. **C2 resolution:** does v0.3.0 add a `deref(doc, value) -> &Value` wrapper over `resolve_ref`, or does ADR-021 rewrite to target `resolve_ref(doc, pointer, depth) -> Option<&Value>` directly? Plan.md phase 2 depends on the answer.
3. **S1 resolution:** on unknown rule IDs in user ruleset, does refract error, warn, or silently drop? Affects `.spectral.yaml` loading code path which is not in this ADR cycle.
4. **S2 resolution:** regex or tokenizer for `path-declarations-must-exist`? One-line note in ADR-018 suffices.
5. **S6 resolution:** path-level + operation-level parameter merge, which copy wins and which is kept for dedup comparison?
6. **Rule trait dispatch:** is `check()` called once per document or once per node? Affects the cost model of `detect_oas_version` when called inside every gated rule.

<!-- End Critic Review -->

