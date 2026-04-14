milestone: v0.1.0
hypothesis: >
  API teams on non-Node CI pipelines adopt a single-binary Spectral-compatible
  linter when it reads their existing .spectral.yaml files and produces comparable
  violations on first run, with no install friction.

scope:
  # --- Foundation ---
  - "Set up Cargo.toml with all workspace dependencies declared: serde = \"1\"
    (features: derive), serde_json = \"1\", serde_yaml = \"0.9\", clap = \"4\"
    (features: derive), thiserror = \"2\", anyhow = \"1\". Set [profile.release]
    with opt-level=\"z\", lto=true, codegen-units=1, strip=true."

  - "Create src/error.rs: define LintError enum with variants Io(#[from] std::io::Error),
    Yaml(#[from] serde_yaml::Error), Json(#[from] serde_json::Error),
    InvalidSpec(String), Ruleset(String). All variants use thiserror::Error.
    No unwrap/expect in library code."

  - "Create src/model/mod.rs: define OasVersion enum (V2, V3_0, V3_1) with a
    detect(doc: &serde_json::Value) -> Result<OasVersion, LintError> function.
    Detection logic: if doc[\"swagger\"] exists and starts with \"2\" => V2;
    if doc[\"openapi\"] starts with \"3.0\" => V3_0; if starts with \"3.1\" => V3_1;
    else => Err(InvalidSpec(\"cannot determine OpenAPI version\"))."

  - "Create src/model/violation.rs: define Violation struct with fields
    rule_id: String, message: String, severity: Severity, path: String (JSON Pointer).
    Define Severity enum (Error, Warn, Info). Derive Debug, Clone, serde::Serialize."

  # --- Parser ---
  - "Create src/parser/mod.rs: implement pub fn parse(path: &Path) ->
    Result<serde_json::Value, LintError>. Detect YAML vs JSON from file extension
    (.yaml/.yml => serde_yaml, .json => serde_json). Read file with
    std::fs::read_to_string. On YAML: deserialize to serde_yaml::Value then convert
    to serde_json::Value via serde_json::to_value. Return LintError::Io on file error,
    LintError::Yaml on parse error, LintError::Json on JSON parse error.
    Note: line/col position tracking is deferred to v0.2.0 — violations report path only."

  # --- Rule engine ---
  - "Create src/rules/mod.rs: define the Rule trait:
      pub trait Rule: Send + Sync {
        fn id(&self) -> &'static str;
        fn message(&self) -> &'static str;
        fn default_severity(&self) -> Severity;
        fn check(&self, doc: &serde_json::Value, version: OasVersion) -> Vec<Violation>;
      }
    Define pub fn default_registry() -> Vec<Box<dyn Rule>> that returns all 8 built-in
    rule structs. Each rule is a zero-size struct implementing Rule."

  - "Implement the 8 built-in OAS rules as structs in src/rules/:
      1. OperationOperationId — every HTTP operation object must have a non-empty operationId
      2. OperationOperationIdUnique — all operationIds across the spec must be unique
      3. OperationTags — every operation must have a non-empty tags array
      4. OperationSummary — every operation must have a non-empty summary string
      5. InfoContact — info object must have a contact field
      6. InfoDescription — info object must have a non-empty description string
      7. OpenApiTags — top-level tags array must exist and be non-empty
      8. OperationDescription — operations should have a non-empty description string
    Each rule traverses the serde_json::Value tree to collect Violations.
    For OAS 2.x: path operations are under paths.{path}.{method} (methods: get/put/post/delete/options/head/patch).
    For OAS 3.x: same structure. Rules 1-4 and 8 iterate all operations; rules 5-7 check
    the top-level info/tags objects."

  # --- Ruleset loading ---
  - "Create src/ruleset/mod.rs: define RulesetConfig struct with field
    severity_overrides: HashMap<String, Option<Severity>> (None = off, Some(s) = override).
    Implement pub fn load(path: &Path) -> Result<RulesetConfig, LintError>.
    Parse the YAML. If extends is present and contains a value other than \"spectral:oas\",
    emit LintError::Ruleset(\"only spectral:oas is supported in v0.1.0\").
    If a rule key is present at top level with value off/warn/error/hint, record it
    in severity_overrides. If any key matches a known custom rule definition pattern
    (given/then/message present), emit a clear LintError::Ruleset explaining custom rules
    are not supported. Ignore unknown keys silently."

  # --- Lint orchestration ---
  - "Create src/lib.rs: implement pub fn lint(spec_path: &Path, ruleset_path: Option<&Path>)
    -> Result<Vec<Violation>, LintError>. Steps:
      1. parse(spec_path) -> doc
      2. OasVersion::detect(&doc) -> version
      3. Load RulesetConfig if ruleset_path provided, else use defaults
      4. Build registry = default_registry()
      5. For each rule: apply severity override from RulesetConfig; skip if severity is None (off)
      6. Run rule.check(&doc, version); collect violations with severity applied
      7. Return sorted violations (by path for stable output)"

  # --- Reporter ---
  - "Create src/reporter/mod.rs: implement two formatters.
    Text formatter: one violation per line, format:
      {file}:{path}  {severity}  {rule_id}  {message}
    (No line/col in v0.1.0 — path is the JSON Pointer location.)
    ANSI color: error=red, warn=yellow, info=blue. Disable when stdout is not a TTY
    (std::io::IsTerminal) or --no-color is set.
    JSON formatter: serialize to the schema defined in ADR-009 (violations array +
    summary object). Use serde_json::to_writer_pretty.
    Both formatters write to dyn Write for testability."

  # --- CLI entry point ---
  - "Create src/main.rs: define Cli struct with clap derive:
      <spec>: PathBuf (required positional)
      --ruleset / -r: Option<PathBuf>
      --format / -f: OutputFormat enum (Text, Json) defaulting to Text
      --no-color: bool flag
      --quiet / -q: bool flag
    main() calls lib::lint(spec, ruleset). On Ok(violations):
      if quiet: exit(if violations.is_empty() { 0 } else { 1 })
      else: format and write violations; exit(if violations.is_empty() { 0 } else { 1 })
    On Err(e): eprintln!(\"{e:#}\"); exit(2).
    Exit codes: 0=no violations, 1=violations found, 2=error."

  # --- Tests ---
  - "Write unit tests for each of the 8 rules in src/rules/tests/. Each test uses a
    minimal inline YAML fixture string (parsed via serde_yaml) that triggers the rule
    (positive case) and a fixture that does not (negative case). Tests use #[test].
    No integration test infra needed — rule tests are pure unit tests on Value input."

  - "Write integration tests in tests/integration.rs: create fixture YAML files in
    tests/fixtures/ (valid_oas3.yaml, missing_operationid.yaml, missing_summary.yaml).
    Each integration test calls lib::lint() and asserts the expected Violation rule_ids
    are present. Verify exit code semantics via process::Command in tests/cli.rs."

architecture_decisions:
  - "Single crate (src/lib.rs + src/main.rs). Module boundaries map to future workspace
    crates if a library consumer emerges in v0.2.0+. (ADR-001)"
  - "serde_yaml 0.9 + serde_json 1.0 for parsing. YAML -> serde_yaml::Value -> serde_json::Value
    normalisation. No line/col position tracking in v0.1.0 — deferred after critic review. (ADR-002)"
  - "Hand-rolled rule engine on serde_json::Value. No external OAS crate — none covers
    2.x/3.x/3.1 with adequate maintenance. (ADR-003)"
  - "clap v4 derive macro. Exit codes 0/1/2. (ADR-004)"
  - "thiserror for library errors, anyhow in main. No unwrap/expect in lib code. (ADR-005)"
  - "8 built-in OAS rules (reduced from 15 per critic review — path-params deferred due to
    cross-reference complexity; 6 cosmetic rules deferred). (ADR-006, critic S2+S3)"
  - "Static binary: musl Linux, macOS system dylib, Windows MSVC. LTO+strip+opt-z. (ADR-007)"
  - "Spectral YAML compatibility: extends:spectral:oas + severity overrides only. MERGE
    semantics — built-in rules always active unless explicitly off. (ADR-008, critic S4)"
  - "Text (default) and JSON output formats. Path-only violation location (no line/col). (ADR-009)"
  - "No async runtime. Synchronous I/O. (ADR-010)"

agent_gaps: []

out_of_scope:
  - "Line and column number in violation output. Reason: two-pass YAML event scanner
    approach was underspecified (critic S1); path-only reporting is sufficient for v0.1.0
    adoption test. Milestone: v0.2.0."
  - "path-params rule. Reason: requires cross-referencing path template tokens with
    parameters array — disproportionate complexity for v0.1.0; has OAS 2.x/3.x
    structural differences that make a single Value traversal error-prone. Milestone: v0.2.0."
  - "6 cosmetic quality rules deferred from original ADR-006 list: info-license,
    no-eval-in-markdown, no-script-tags-in-markdown, openapi-tags-alphabetical,
    contact-properties, license-url. Reason: low CI-blocking urgency; not structurally
    load-bearing. Milestone: v0.2.0."
  - "Custom rule definitions in .spectral.yaml (given/then/message/function). Reason:
    requires a JSON Path engine and JS runtime — antithetical to the no-runtime value prop.
    Milestone: v0.3.0 or never (separate product decision)."
  - "Remote ruleset URL resolution. Reason: adds network dependency and latency; breaks
    offline CI. Milestone: v0.2.0 with caching."
  - "spectral:asyncapi ruleset. Reason: different domain; v0.1.0 hypothesis is OAS-specific.
    Milestone: v0.3.0."
  - "Multi-file $ref resolution. Reason: requires a resolver that follows file paths —
    scope creep for v0.1.0. Inline $refs within a single file are resolved by the JSON
    Value tree naturally. Milestone: v0.2.0."
  - "brew tap and cargo install distribution. Reason: GitHub Releases is sufficient for
    v0.1.0 adoption signal. Milestone: v0.2.0 once traction confirmed."
  - "SARIF output format. Reason: GitHub Advanced Security integration is a v0.2.0 feature.
    Milestone: v0.2.0."
  - "Watch mode and LSP server mode. Reason: editor integration is post-validation scope.
    Milestone: v0.3.0."
  - "Async runtime. Reason: workload is single-file synchronous; no benefit. Milestone: v0.2.0
    if multi-file linting is added."
  - "mod.rs → named module style (src/rules.rs + src/rules/ subfolder, etc.). Reason: pure
    rename refactor; wrong time relative to open PR. Do as first commit on v0.2.0 branch.
    Milestone: v0.2.0."
  - "#![warn(missing_docs)] + doc comments on Rule trait methods (id, message, default_severity,
    check). Reason: touches every rule file; scope creep for v0.1.0 PR. Milestone: v0.2.0."
  - "Collapse reporter::write_text + reporter::write_json into a single report() function with
    a Format enum (Text/Json). Eliminates the bool use_color anti-pattern and the implicit
    format coupling between two parallel functions. Reason: no external consumer yet — API
    shape should be deferred until a caller exists. Milestone: v0.2.0."
  - "Method-vs-loose-function API shaping across all modules (parser::parse, ruleset::load,
    lib::lint, etc.). Reason: premature without a real consumer — natural shape will emerge
    when openapi-linter-core is extracted. One exception: ruleset::load could become
    RulesetConfig::load() (cosmetic, symmetric with RulesetConfig::empty()). Milestone: v0.2.0."
