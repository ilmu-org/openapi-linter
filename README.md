# openapi-linter

Fast OpenAPI linter — Spectral OAS compatible, ships as a single static binary.

## Why

[Spectral](https://github.com/stoplightio/spectral) is the de-facto standard for
OpenAPI linting, but it requires Node.js. openapi-linter reads the same
`.spectral.yaml` ruleset files and produces compatible output, so it drops into
Go, Python, Java, and other non-Node CI pipelines with no migration work.

## Installation

### Download from GitHub Releases

| Platform | Download |
|---|---|
| Linux x86\_64 (musl) | [openapi-linter-x86\_64-unknown-linux-musl.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-unknown-linux-musl.tar.gz) |
| Linux aarch64 (musl) | [openapi-linter-aarch64-unknown-linux-musl.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-aarch64-unknown-linux-musl.tar.gz) |
| macOS x86\_64 | [openapi-linter-x86\_64-apple-darwin.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-apple-darwin.tar.gz) |
| macOS aarch64 (Apple Silicon) | [openapi-linter-aarch64-apple-darwin.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-aarch64-apple-darwin.tar.gz) |
| Windows x86\_64 | [openapi-linter-x86\_64-pc-windows-msvc.zip](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-pc-windows-msvc.zip) |

Extract and place the binary on your `PATH`.

### From source

```sh
cargo install --git https://github.com/ilmu-org/openapi-linter
```

## Quick start

```sh
openapi-linter spec.yaml
```

Example output:

```
spec.yaml  warn  info-contact        Info object must have a contact field.
spec.yaml  warn  info-description    Info object must have a non-empty description.
spec.yaml  error operation-operationId  Operation must have a non-empty operationId.
```

Exit code is `1` when violations are found. Exit code `0` means the spec is clean.

## Usage

```
openapi-linter [OPTIONS] <SPEC>

Arguments:
  <SPEC>  Path to the OpenAPI spec file (YAML or JSON)

Options:
  -r, --ruleset <RULESET>  Path to a .spectral.yaml ruleset file
  -f, --format <FORMAT>    Output format [default: text] [possible values: text, json]
      --no-color           Disable ANSI colour in text output
  -q, --quiet              Suppress output; exit 0 if clean, 1 if violations found
  -h, --help               Print help
  -V, --version            Print version
```

### JSON output

```sh
openapi-linter --format json spec.yaml
```

```json
{
  "source": "spec.yaml",
  "violations": [
    {
      "rule": "info-contact",
      "severity": "warn",
      "message": "Info object must have a contact field.",
      "path": "/info"
    }
  ]
}
```

## Rules

| Rule ID | Description | Default Severity |
|---|---|---|
| `info-contact` | `info.contact` must be present | warn |
| `info-description` | `info.description` must be non-empty | warn |
| `openapi-tags` | Top-level `tags` array must be present and non-empty | warn |
| `operation-description` | Each operation should have a non-empty `description` | info |
| `operation-operationId` | Each operation must have a non-empty `operationId` | error |
| `operation-operationId-unique` | `operationId` values must be unique across all operations | error |
| `operation-summary` | Each operation must have a non-empty `summary` | warn |
| `operation-tags` | Each operation must have a non-empty `tags` array | warn |

All rules are enabled by default. Severity can be overridden per rule via a
`.spectral.yaml` ruleset file.

## Spectral compatibility

openapi-linter reads `.spectral.yaml` and `.spectral.yml` files from the current
directory. The following `extends` values are recognised:

```yaml
extends: [[spectral:oas, recommended]]
# or
extends: spectral:oas
```

Override rule severity or disable rules:

```yaml
extends: [[spectral:oas, recommended]]
rules:
  info-contact: off
  operation-description: warn
  operation-operationId: error
```

Valid severity values: `error`, `warn`, `info`, `off`.

Unknown rule IDs in the `rules` block are ignored.

## CI integration

### GitHub Actions

```yaml
- name: Lint OpenAPI spec
  run: |
    curl -sSL https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-unknown-linux-musl.tar.gz \
      | tar -xz -C /usr/local/bin
    openapi-linter spec.yaml
```

Or with a custom ruleset:

```yaml
- name: Lint OpenAPI spec
  run: openapi-linter --ruleset .spectral.yaml spec.yaml
```

## Exit codes

| Code | Meaning |
|---|---|
| `0` | No violations |
| `1` | One or more violations found |
| `2` | Error (unreadable file, invalid YAML/JSON, etc.) |

## License

MIT
