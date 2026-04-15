# refract — Project Standards

Extends org-level standards in `../_ops/CLAUDE.md`. Read that first.

## Project

Fast OpenAPI linter. Ships as single static binary, no Node.js dependency.
Validates OpenAPI 2.x/3.x/3.1 specs against Spectral-compatible OAS ruleset.
Reads existing `.spectral.yaml` / `.spectral.yml` files for zero-migration compatibility.

## Approved idea

ilmu-org/ideas#2

## State

Read `.ilmu/state.md` before any task.
Read `.ilmu/decisions.md` (ADR index) before any architectural choice, then fetch relevant ADR files in `.ilmu/decisions/`. Completed milestone plans archived in `.ilmu/archive/`.
Read `.ilmu/plan.md` before any build task.

## Key constraints (from research)

- Spectral OAS ruleset compatibility = core migration hook — prioritise
- Target: non-Rust CI pipelines (Go, Python, Java) — binary UX matters
- Single static binary: no runtime deps, no install friction
- v0.1.0 scope cut by rust-critic — do not expand
## Commit messages

Plain conventional commits only: `type: what changed`.
No phase labels (`phase 1`, `phase N`), no step labels, no issue/PR references.
No em-dashes.

## Build branching

Integration branch: `build/vX.X.X` off `main`.
Phase branches: `phase{N}/vX.X.X`.
- Phase 1 branches from `build/vX.X.X`
- Phase N (N > 1) branches from `phase{N-1}/vX.X.X`

Each phase opens PR targeting `build/vX.X.X`. Do not wait for human PR approval between phases. Branch next phase from current phase branch immediately after opening its PR.

Merge PRs into `build/vX.X.X` in phase order. Document the upstream branch dependency in each PR description.

Final PR: `build/vX.X.X -> main`. Open after all phase PRs merged and integration check passes. Leave for human review, do not merge.

## Writing .ilmu files and prompts

Any agent writing or updating files in `.ilmu/` (state, plan, decisions, ADRs) or creating a prompt file must run the output through the caveman compress skill before writing it to disk:

```
cd ~/.claude/plugins/cache/caveman/caveman/63e797cd753b/caveman-compress
python3 -m scripts <absolute_path_to_file>
```

Delete the `.original.md` backup after verifying compressed file looks correct.
