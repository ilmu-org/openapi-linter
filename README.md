# refract

> **Renamed from `openapi-linter`** — the GitHub repo URL redirects automatically. This note will be removed after v1.0.0.

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
but it requires Node.js. refract reads the same `.spectral.yaml` ruleset files and produces
compatible output, so it drops into Go, Python, Java, and other non-Node CI pipelines with no
migration work — download the binary, run it.

**When to use what:**

- Use **Spectral** if you are already on a Node.js stack.
- Use **[vacuum](https://github.com/daveshanley/vacuum)** if you need a Go binary with a dashboard UI and deep rule coverage.
- Use **refract** if you need a single static binary with no runtime dependencies and existing `.spectral.yaml` files you do not want to touch.

## Features

- Validates OpenAPI 2.x, 3.0, and 3.1 specs (YAML and JSON)
- Reads existing `.spectral.yaml` / `.spectral.yml` — no config migration
- 15 built-in rules covering info, operations, tags, and security
- Text, JSON, and SARIF output formats
- Non-zero exit code on violations — works natively in CI
- Single static binary — no Node.js, no Docker, no package manager

## Installation

### Linux and macOS

```sh
# Linux x86_64
curl -sSL https://github.com/ilmu-org/openapi-linter/releases/download/v0.2.0/refract-x86_64-unknown-linux-musl.tar.gz \
  | tar -xz -C /usr/local/bin

# macOS Apple Silicon
curl -sSL https://github.com/ilmu-org/openapi-linter/releases/download/v0.2.0/refract-aarch64-apple-darwin.tar.gz \
  | tar -xz -C /usr/local/bin

# macOS Intel
curl -sSL https://github.com/ilmu-org/openapi-linter/releases/download/v0.2.0/refract-x86_64-apple-darwin.tar.gz \
  | tar -xz -C /usr/local/bin
```

### All platforms

| Platform | Download |
|---|---|
| Linux x86\_64 (musl) | [refract-x86\_64-unknown-linux-musl.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.2.0/refract-x86_64-unknown-linux-musl.tar.gz) |
| Linux aarch64 (musl) | [refract-aarch64-unknown-linux-musl.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.2.0/refract-aarch64-unknown-linux-musl.tar.gz) |
| macOS Apple Silicon | [refract-aarch64-apple-darwin.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.2.0/refract-aarch64-apple-darwin.tar.gz) |
| macOS Intel | [refract-x86\_64-apple-darwin.tar.gz](https://github.com/ilmu-org/openapi-linter/releases/download/v0.2.0/refract-x86_64-apple-darwin.tar.gz) |
| Windows x86\_64 | [refract-x86\_64-pc-windows-msvc.zip](https://github.com/ilmu-org/openapi-linter/releases/download/v0.2.0/refract-x86_64-pc-windows-msvc.zip) |

### From source

```sh
cargo install --git https://github.com/ilmu-org/openapi-linter
```

## Quick start

```sh
refract spec.yaml
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
refract [OPTIONS] <SPEC>

Arguments:
  <SPEC>  Path to the OpenAPI spec file or directory (YAML or JSON)

Options:
  -r, --ruleset <RULESET>  Path to a .spectral.yaml ruleset file
  -f, --format <FORMAT>    Output format [default: text] [possible values: text, json, sarif]
      --no-color           Disable ANSI colour in text output
      --color              Always enable ANSI colour (overrides --no-color)
  -q, --quiet              Suppress output; exit 0 if clean, 1 if violations found
  -h, --help               Print help
  -V, --version            Print version
```

### JSON output

```sh
refract --format json spec.yaml
```

### SARIF output (GitHub Code Scanning)

```sh
refract --format sarif spec.yaml
```

### Directory scan

```sh
refract --format sarif ./api/
```

## Rules

| Rule ID | Description | Default Severity |
|---|---|---|
| `contact-properties` | `info.contact` fields should include `name`, `url`, or `email` | warn |
| `info-contact` | `info.contact` must be present | warn |
| `info-description` | `info.description` must be non-empty | warn |
| `info-license` | `info.license` must be present | warn |
| `license-url` | `info.license` must include a `url` | warn |
| `no-eval-in-markdown` | Descriptions and summaries must not contain `eval(` | error |
| `no-script-tags-in-markdown` | Descriptions and summaries must not contain `<script>` | error |
| `openapi-tags` | Top-level `tags` array must be present and non-empty | warn |
| `openapi-tags-alphabetical` | Top-level `tags` must be in alphabetical order | warn |
| `operation-description` | Each operation should have a non-empty `description` | info |
| `operation-operationId` | Each operation must have a non-empty `operationId` | error |
| `operation-operationId-unique` | `operationId` values must be unique across all operations | error |
| `operation-summary` | Each operation must have a non-empty `summary` | warn |
| `operation-tags` | Each operation must have a non-empty `tags` array | warn |
| `path-params` | Path parameters defined in the URL must have a matching `parameters` entry | error |

All rules are enabled by default. Severity can be overridden per rule via a `.spectral.yaml` file.

## Spectral compatibility

refract reads `.spectral.yaml` and `.spectral.yml` from the current directory automatically,
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
- name: Install refract
  run: |
    curl -sSL https://github.com/ilmu-org/openapi-linter/releases/download/v0.2.0/refract-x86_64-unknown-linux-musl.tar.gz \
      | tar -xz -C /usr/local/bin

- name: Lint OpenAPI spec
  run: refract spec.yaml
```

With a custom ruleset:

```yaml
- name: Lint OpenAPI spec
  run: refract --ruleset .spectral.yaml spec.yaml
```

### SARIF upload to GitHub Code Scanning

```yaml
- name: Lint OpenAPI spec
  run: refract --format sarif spec.yaml > results.sarif

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

## Exit codes

| Code | Meaning |
|---|---|
| `0` | No violations |
| `1` | One or more violations found |
| `2` | Error (unreadable file, invalid YAML/JSON, etc.) |

## License

MIT
