# Changelog

All notable changes to refract are documented here.

## [Unreleased] - v0.4.0

### Added

4 new rules and cross-file `$ref` resolution:

| Rule ID | Description | Severity |
|---|---|---|
| `oas3-schema` | Validate OAS 3.x document structure against the bundled OAS JSON Schema (3.0 or 3.1) | error |
| `oas2-schema` | Validate OAS 2.0 document structure against the bundled Swagger JSON Schema | error |
| `oas3-valid-schema-example` | Validate `example`/`examples` values against their enclosing schemas (OAS 3.x) | error |
| `oas2-valid-schema-example` | Validate `example` values against their enclosing schemas (OAS 2.0) | error |

### Changed

- Cross-file `$ref` resolution is now active: external file refs are inlined before linting via an
  eager pre-pass. HTTP refs remain unsupported and emit a warning violation. Cycle detection and
  a depth limit of 64 prevent unbounded traversal.
- `Rule::check` signature changed from `(doc, version)` to `(&LintContext)`, enabling rules to
  access the shared boon schema registry and spec file path.
- OAS 2.0, 3.0, and 3.1 JSON Schemas are now bundled in the binary (via `include_str!`), parsed
  once on first use via `OnceLock`, and pre-registered in a shared boon registry per lint call.

### Notes

- OAS 3.1 `$ref` with sibling keywords: the `no-$ref-siblings` rule still fires for OAS 3.1
  documents. OAS 3.1 formally allows `$ref` siblings but the rule is conservative by default;
  disable it via ruleset config if needed.
- `Deref'd<'a>` newtype and workspace extraction remain deferred (ADR-021/ADR-024).

## [0.3.0] - 2026-04-15

### Added

17 new rules covering paths, enums, operations, parameters, servers, and tags:

| Rule ID | Description | Severity |
|---|---|---|
| `array-items` | Schema with `type: array` must declare an `items` property | error |
| `duplicated-entry-in-enum` | `enum` arrays must not contain duplicate values | error |
| `no-$ref-siblings` | `$ref` objects must not have sibling keys (OAS 2.x/3.0; skipped for OAS 3.1) | error |
| `oas3-api-servers` | OAS 3.x document must define a non-empty `servers` array | warn |
| `oas3-parameter-description` | Every parameter must have a non-empty `description` (OAS 3.x only) | warn |
| `oas3-server-not-example.com` | Server URLs must not point to `example.com` (OAS 3.x only) | warn |
| `oas3-server-trailing-slash` | Server URLs must not end with a trailing slash (OAS 3.x only) | warn |
| `openapi-tags-uniqueness` | Top-level `tags` array must not contain duplicate tag names | error |
| `operation-parameters` | Operation must not define duplicate parameters with the same name and location | warn |
| `operation-success-response` | Each operation must define at least one 2xx response | warn |
| `operation-operationId-valid-in-url` | `operationId` must contain only URL-safe characters | warn |
| `operation-tag-defined` | Tags referenced in operations must be declared in the top-level `tags` array | warn |
| `path-declarations-must-exist` | Path template parameters (`{param}`) must not be empty placeholders | error |
| `path-keys-no-trailing-slash` | Path keys must not end with a trailing slash (root `/` is exempt) | warn |
| `path-not-include-query` | Path keys must not include query string parameters | error |
| `tag-description` | Each top-level tag must have a non-empty `description` | warn |
| `typed-enum` | Each value in an `enum` array must be compatible with the declared schema `type` | warn |

### Changed

- `OasVersion::detect()` now returns `OasVersion::Unknown` instead of an error for unrecognised
  documents. Version-gated rules are skipped silently; a warning is printed to stderr.
- Refactored `$ref` resolution: all deref-dependent rules call `resolve_ref` before reading fields
  on any node that may be a `$ref` object (ADR-021 deref-before-compare contract).

### Notes

- External `$ref` values (URLs or file paths) are treated as opaque and skipped to avoid false
  positives. Cross-file `$ref` resolution is planned for v0.4.0.
- JSON Schema validation rules (keyword coverage via boon) are deferred to v0.4.0 (ADR-019).

## [0.2.0] - 2026-04-14

### Added

- 15 built-in OAS rules (info, operations, tags, path params, security markers)
- Text, JSON, and SARIF output formats
- Directory scan support
- `.spectral.yaml` / `.spectral.yml` ruleset reading with severity overrides
- `OasVersion` detection (V2, V3_0, V3_1)
- `$ref` resolution utility with cycle protection (depth limit 10)
- Line and column reporting for YAML/JSON sources

## [0.1.0] - 2026-04-07

### Added

- Initial release: 8 rules, single static binary, YAML/JSON input
