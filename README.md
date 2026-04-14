# openapi-linter

A single-binary OpenAPI linter for teams not running Node.js — Spectral-compatible, zero install friction.

[![CI](https://github.com/ilmu-org/openapi-linter/actions/workflows/ci.yml/badge.svg)](https://github.com/ilmu-org/openapi-linter/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Contents

- [Why](#why)
- [Features](#features)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Usage](#usage)
- [Rules](#rules)
- [Spectral compatibility](#spectral-compatibility)
- [CI integration](#ci-integration)
- [Exit codes](#exit-codes)

## Why

[Spectral](https://github.com/stoplightio/spectral) is the de-facto standard for OpenAPI linting,
but it requires Node.js. openapi-linter reads the same `.spectral.yaml` ruleset files and produces
compatible output, so it drops into Go, Python, Java, and other non-Node CI pipelines with no
migration work — download the binary, run it.

**When to use what:**

- Use **Spectral** if you are already on a Node.js stack.
- Use **[vacuum](https://github.com/daveshanley/vacuum)** if you need a Go binary with a dashboard UI and deep rule coverage.
- Use **openapi-linter** if you need a single static binary with no runtime dependencies and existing `.spectral.yaml` files you do not want to touch.

## Features

- Validates OpenAPI 2.x, 3.0, and 3.1 specs (YAML and JSON)
- Reads existing `.spectral.yaml` / `.spectral.yml` — no config migration
- 8 built-in rules covering info, operations, and tags
- Text and JSON output formats
- Non-zero exit code on violations — works natively in CI
- Single static binary — no Node.js, no Docker, no package manager

> **Coming in v0.2.0:** violation output will include source file line and column numbers,
> directory scanning, and SARIF output for GitHub Code Scanning annotations.

## Installation

### Linux and macOS

```sh
# Linux x86_64
curl -sSL https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-unknown-linux-musl.tar.gz \
  | tar -xz -C /usr/local/bin

# macOS Apple Silicon
curl -sSL https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-aarch64-apple-darwin.tar.gz \
  | tar -xz -C /usr/local/bin

# macOS Intel
curl -sSL https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-apple-darwin.tar.gz \
  | tar -xz -C /usr/local/bin
```

### All platforms

| Platform | Download |
|---|---|
| Linux x86\_64 (musl) | [openapi-linter-x86\_64-unknown-linux-musl.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-unknown-linux-musl.tar.gz) |
| Linux aarch64 (musl) | [openapi-linter-aarch64-unknown-linux-musl.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-aarch64-unknown-linux-musl.tar.gz) |
| macOS Apple Silicon | [openapi-linter-aarch64-apple-darwin.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-aarch64-apple-darwin.tar.gz) |
| macOS Intel | [openapi-linter-x86\_64-apple-darwin.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-apple-darwin.tar.gz) |
| Windows x86\_64 | [openapi-linter-x86\_64-pc-windows-msvc.zip](https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-pc-windows-msvc.zip) |

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
spec.yaml  warn   info-contact           Info object must have a contact field.
spec.yaml  warn   info-description       Info object must have a non-empty description.
spec.yaml  error  operation-operationId  Operation must have a non-empty operationId.
```

Exit code is `1` when violations are found, `0` when the spec is clean.

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

All rules are enabled by default. Severity can be overridden per rule via a `.spectral.yaml` file.

## Spectral compatibility

openapi-linter reads `.spectral.yaml` and `.spectral.yml` from the current directory automatically,
or you can pass a ruleset file explicitly with `--ruleset`.

The following `extends` values are recognised:

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

## CI integration

### GitHub Actions

```yaml
- name: Install openapi-linter
  run: |
    curl -sSL https://github.com/ilmu-org/openapi-linter/releases/download/v0.1.0/openapi-linter-x86_64-unknown-linux-musl.tar.gz \
      | tar -xz -C /usr/local/bin

- name: Lint OpenAPI spec
  run: openapi-linter spec.yaml
```

With a custom ruleset:

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
