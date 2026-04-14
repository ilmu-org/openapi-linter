# refract — Project Standards

Extends org-level standards in `../_ops/CLAUDE.md`. Read that file first.

## Project

A fast OpenAPI linter that ships as a single static binary with no Node.js dependency.
Validates OpenAPI 2.x/3.x/3.1 specs against a Spectral-compatible OAS ruleset.
Reads existing `.spectral.yaml` / `.spectral.yml` ruleset files for zero-migration compatibility.

## Approved idea

ilmu-org/ideas#2

## State

Always read `.ilmu/state.md` before starting any task.
Always read `.ilmu/decisions.md` before making any architectural choice.
Always read `.ilmu/plan.md` before starting any build task.

## Key constraints (from research)

- Spectral OAS ruleset compatibility is the core migration hook — prioritise this
- Target audience is non-Rust CI pipelines (Go, Python, Java) — binary UX matters
- Single static binary: no runtime dependencies, no install friction
- v0.1.0 scope is cut by rust-critic — do not expand it
