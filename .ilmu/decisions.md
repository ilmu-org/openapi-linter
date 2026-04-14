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
