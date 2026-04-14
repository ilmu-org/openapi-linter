# Sprint Review: v0.3.0 Scoping

## Delivered

- ADR-018: 17 rules confirmed in scope for v0.3.0 (structural correctness, path hygiene, tag validation, parameter dedup, enum integrity)
- ADR-019: JSON Schema validation rules (oas3-schema, oas2-schema, oas3-valid-schema-example, oas2-valid-schema-example) deferred to v0.4.0 with boon assessed as leading evaluator candidate
- ADR-020: Cross-file $ref resolution deferred to v0.4.0; external $ref nodes treated as opaque (false negatives only, no false positives)
- ADR-021: OasVersion enum + detect_oas_version() utility; deref-before-compare doc-comment contract on resolve_ref(); typed-enum coercion semantics frozen
- plan.md v0.3.0 section: 17 rules with per-rule severity, applicability, fixture requirements; 4 phases as separate PRs; all critic findings resolved
- state.md updated: current_milestone v0.3.0, phase planning, v0.3.0 scoping complete

## Deviated from spec

- Architect referenced a deref() wrapper function that does not exist in the codebase. The actual function is resolve_ref(doc, pointer, depth) -> Option<&Value>. Teamlead resolved this without an architect re-run: plan.md and ADR notes use resolve_ref throughout; no deref() wrapper is added in v0.3.0.
- no-$ref-siblings was in the version-gated rule list per ADR-021 draft, but OAS 3.1 permits $ref siblings under JSON Schema 2020-12. Teamlead resolved: rule is format-gated inside its own check() and skips V3_1; it is not in the shared version-gated table.

## Deferred (reason)

- oas3-schema, oas2-schema, oas3-valid-schema-example, oas2-valid-schema-example: require a JSON Schema evaluator (boon assessed, architectural friction beyond binary size justifies isolation to v0.4.0)
- Cross-file $ref resolution: pairs naturally with schema validation in v0.4.0; bundled-spec workflows (majority) fully covered by v0.3.0
- Deref'd newtype for compile-time deref-before-compare enforcement: re-evaluate if v0.4.0 deref-dependent rule count exceeds 8
- refract-core workspace crate extraction: no external library consumer yet; v0.4.0 or later
- Strict typed-enum mode: possible v0.4.0 config flag if demand emerges

## Technical decisions made

- OasVersion::Unknown emits one-time stderr diagnostic per lint run to prevent silent coverage gaps
- operation-parameters merge: operation-level entries override path-level on (name, in); overridden path-level copy dropped from comparison set to prevent false-positive violations
- path-declarations-must-exist uses regex \{\s*\} for path parameter detection (avoids tokenizer complexity)
- detect_oas_version checks swagger field first, then openapi field; behavior frozen per ADR-021
- Unknown rule IDs in .spectral.yaml already emit warnings in v0.2.0; no change needed

## Open issues for next sprint

- Implementation sprint (v0.3.0 build): 4 phases, 4 separate PRs, starting on a feature branch
- Phase 1 (11 structural rules), Phase 2 (util.rs additions), Phase 3 (4 deref-dependent rules), Phase 4 (2 type-aware rules)
- CHANGELOG.md and README.md updates for cross-file $ref gap and schema-validation deferral
- PR template update: add resolve_ref None-handling reviewer checklist line
- Binary size verification after all 17 rules land (~5 MB budget)
