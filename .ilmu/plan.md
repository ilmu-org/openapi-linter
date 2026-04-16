# Archived milestones: .ilmu/archive/
# v0.1.0: .ilmu/archive/plan-v0.1.0.md (complete)
# v0.2.0: .ilmu/archive/plan-v0.2.0.md (complete)
# v0.3.0: .ilmu/archive/plan-v0.3.0.md (complete)

---

# v0.4.0 Plan

## Scope

4 new rules: `oas3-schema`, `oas2-schema`, `oas3-valid-schema-example`, `oas2-valid-schema-example`.
Cross-file `$ref` resolver infrastructure.
Boon crate integration for JSON Schema evaluation.
Rule trait refactor: `fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation>`.
Optional parallel chore: GHA Node.js 24 migration (ADR-025).

## Out of scope

- Deref'd<'a> newtype: 6 deref-dependent rule files in v0.4.0, threshold >8. Defer to v0.5.0.
- Workspace extraction (refract-core): no consumer trigger. ADR-024 continues ADR-001.
- HTTP $ref support: rejected in ADR-023.
- Spectral custom rule format: out of scope all v0.x.

## Deferred from v0.3.0 (now in scope)

- Cross-file $ref resolution (ADR-020 -> ADR-023)
- JSON Schema validation rules (ADR-019 -> ADR-022)

## ADRs

- ADR-022: Boon crate integration, LintContext, registry lifetime
- ADR-023: Eager pre-pass cross-file $ref resolver
- ADR-024: Workspace structure deferred
- ADR-025: GHA Node.js 24 migration (standalone chore)

---

## Build branches

Integration branch: `build/v0.4.0` off `main`.
Phase branches: `phase1/v0.4.0`, `phase2/v0.4.0`, `phase3/v0.4.0`.
Phase 1 branches from `build/v0.4.0`. Each subsequent phase branches from prior phase branch.
Each phase opens PR targeting `build/v0.4.0`.
Optional chore PR opens against `main` independently (no dependency on build/v0.4.0).

---

## Phase 1: Cross-file $ref resolver

**PR:** `phase1/v0.4.0` -> `build/v0.4.0`

### New files

- `src/resolver.rs`: `pub fn resolve_external_refs(doc: Value, base_path: &Path) -> Result<Value, Vec<ResolveError>>`
- `tests/fixtures/external-refs/`: fixture spec trees for resolver integration tests

### New types

```rust
pub enum ResolveError {
    FileNotFound { path: PathBuf, ref_str: String },
    MalformedFile { path: PathBuf, message: String },
    PointerNotFound { path: PathBuf, pointer: String },
    Cycle { path: PathBuf },
    HttpRefForbidden { ref_str: String },
    DepthExceeded,
}
```

### Algorithm

Depth-first walk of `Value` tree. On object node with `"$ref"` key:
- `#` prefix: skip (internal ref).
- `http://` / `https://` prefix: return `ResolveError::HttpRefForbidden`.
- Otherwise: parse as `path#/pointer`. Resolve path relative to `base_path`. Cache lookup (`HashMap<PathBuf, Value>`). Navigate pointer. Replace node with inlined content. Recurse using target file's directory as new base.
Cycle detection: `HashSet<(PathBuf, String)>` of (canonical_path, json_pointer). Depth limit: 64.

### LintError variants to add (src/error.rs or src/lib.rs)

```rust
LintError::UnresolvableRef { path: PathBuf, ref_str: String }
LintError::RefCycle { path: PathBuf }
LintError::HttpRefNotSupported { ref_str: String }
LintError::RefDepthExceeded
```

### Integration point

`src/lib.rs` `lint()`: call `resolve_external_refs(doc, base_path)` after parse, before rules. Errors prepended to result. Continue on partial resolution (best-effort).

`lint_dir()`: pass each file's path as `base_path` to `lint()`.

### Known limitation

OAS 3.1 `$ref` siblings (`summary`, `description`) lost during inlining. Documented in ADR-023. Not addressed in v0.4.0.

### Windows

Use `dunce` crate for path canonicalization (avoids UNC paths). Add `dunce` to dependencies.

### Tests

Unit tests in `src/resolver.rs`. Integration tests in `tests/` with fixture spec trees covering: basic external ref, nested external ref, cycle, missing file, missing pointer, HTTP ref rejection, depth limit.

### No rule changes in Phase 1

All rule signatures stay unchanged. `LintContext` refactor deferred to Phase 2.

---

## Phase 2: Boon integration + structural schema rules

**PR:** `phase2/v0.4.0` -> `build/v0.4.0`
**Branches from:** `phase1/v0.4.0`

### New files

- `src/schemas.rs`: OAS JSON Schema constants and OnceLock init
- `src/lint.rs` (or `src/lib.rs`): `LintContext<'a>` struct
- `src/rules/oas3_schema.rs`
- `src/rules/oas2_schema.rs`

### LintContext

```rust
pub(crate) struct LintContext<'a> {
    pub doc: &'a serde_json::Value,
    pub version: OasVersion,
    pub schemas: &'a boon::Schemas,
    pub base_path: Option<&'a std::path::Path>,
}
```

### Schema bundling

```rust
// src/schemas.rs
static OAS3_0_SCHEMA: OnceLock<serde_json::Value> = OnceLock::new();
static OAS3_1_SCHEMA: OnceLock<serde_json::Value> = OnceLock::new();
static OAS2_SCHEMA: OnceLock<serde_json::Value> = OnceLock::new();

pub(crate) fn oas3_0_schema() -> &'static serde_json::Value { ... }
// etc.
```

OAS JSON Schema files bundled via `include_str!()`. Stored in `assets/schemas/`.

### Registry lifetime

One `boon::Schemas` per `lint_dir()` invocation, shared across all files. Single-file `lint()`: one per call. OAS schemas pre-registered at construction.

### Rule trait refactor

```rust
pub(crate) trait Rule {
    fn id(&self) -> &'static str;
    fn check(&self, ctx: &LintContext<'_>) -> Vec<Violation>;
}
```

All 32 existing rules: mechanical update from `(doc: &Value, version: OasVersion)` to `ctx: &LintContext<'_>`. No behavior change. `ctx.doc` and `ctx.version` substituted at call sites.

### oas3-schema rule

Validates entire OAS 3.x doc against bundled OAS 3.0 or 3.1 JSON Schema (gated by `ctx.version`). Uses `ctx.schemas.validate(ctx.doc, schema_url)`. One `Violation` per boon leaf output unit (JSON Pointer path + error message). Truncated at 64 per call.

### oas2-schema rule

Validates entire OAS 2.0 doc against bundled OAS 2.0 JSON Schema. Same truncation.

### Error translation

Per leaf boon output unit -> one `Violation`:
- `rule_id`: rule name
- `path`: JSON Pointer from unit's instance location
- `message`: unit's error description
Non-leaf units skipped. If tree exceeds 64 leaf violations per rule call, truncate and append: `"... N more schema violations omitted"`.

### Cargo.toml additions

```toml
boon = "0.6.1"
dunce = "1"   # from Phase 1
```

`assets/schemas/` directory with OAS JSON Schema files (downloaded or committed).

---

## Phase 3: Example validation rules

**PR:** `phase3/v0.4.0` -> `build/v0.4.0`
**Branches from:** `phase2/v0.4.0`

### New files

- `src/rules/oas3_valid_schema_example.rs`
- `src/rules/oas2_valid_schema_example.rs`

### oas3-valid-schema-example

Walk OAS 3.x doc. For each schema object with `example` field (or `examples` map):
1. Locate schema. Call `resolve_ref(ctx.doc, pointer, depth)` if schema is `$ref` (deref-dependent).
2. Register schema in `ctx.schemas` if not already registered. Boon compile failure: `LintError::MalformedSchema { path, message }`.
3. Validate example against schema. One `Violation` per boon leaf output unit. Truncated at 64.

### oas2-valid-schema-example

Same pattern for OAS 2.0 `definitions` and parameter/response schemas with `example` fields.

### Malformed schema handling

Boon compile failure on user schema -> `LintError::MalformedSchema`, not `Violation`. Rule skips example validation for that schema. Subsequent schemas in same doc still validated.

### Deref dependency

Both rules call `resolve_ref` for schema `$ref` resolution. Count: 4 existing + 2 new = 6 deref-dependent rule files. Threshold >8 not triggered. `Deref'd<'a>` newtype deferred.

---

## Optional chore: GHA Node.js 24 migration

**PR:** against `main` (standalone, no dependency on build/v0.4.0)
**Owner:** cicd agent or developer

Update `.github/workflows/ci.yml` and `.github/workflows/release.yml`:
- `actions/checkout@v4` -> `actions/checkout@v6`
- `actions/upload-artifact@v4` -> `actions/upload-artifact@v7`
- `actions/download-artifact@v4` -> `actions/download-artifact@v8`
- `softprops/action-gh-release@v2` -> `softprops/action-gh-release@v3`

`Swatinem/rust-cache@v2` and `dtolnay/rust-toolchain@stable` are composite actions; no change needed.

Before merging: check `softprops/action-gh-release@v3` release notes for breaking input changes. Merge when all CI checks green.

---

## Architect notes

### Phase sequencing rationale

Phase 1 first: resolver pure infrastructure, standalone testable, no rule changes. Isolates resolver bugs from boon bugs.

Phase 2 second: LintContext refactor touches all 32 rule files -- mechanical but broad. Boon integration and oas3/oas2-schema rules added same PR since LintContext prerequisite for both.

Phase 3 third: example validation rules require both resolver (fully-resolved doc) and boon registry (from Phase 2 LintContext). Cannot ship before Phase 2.

### boon::Schemas mutability

`boon::Schemas` built by `boon::Compiler`. User-defined schema registration during `check()` calls requires `&mut Schemas` or interior mutability. If boon API requires `&mut`, wrap in `RefCell` or pre-register all schemas before rule evaluation. Verify boon 0.6.1 API signature before implementation.

### dunce dependency

Add `dunce = "1"` for Windows UNC path normalization in resolver. No impact on Linux/macOS builds.

### Rule count after v0.4.0

32 existing + 4 new = 36 rules total.