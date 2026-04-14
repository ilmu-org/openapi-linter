---
project: openapi-linter
project_type: cli-tool
target_ecosystem: cross-platform (macOS, Linux, Windows)
build_team: rust_build_team

current_milestone: v0.2.0
phase: pr-open
current_task: v0.2.0 PR open — awaiting review
completed_tasks:
  - planning/rust-architect
  - planning/rust-critic
  - planning/sdd
  - plan-approved
  - v0.2.0-scoping
  - v0.2.0-build

open_questions: []

milestones:
  v0.1.0:
    hypothesis: do API teams on non-Node CI pipelines adopt a single-binary Spectral-compatible linter?
    scope: see .ilmu/plan.md
    status: complete
  v0.2.0:
    hypothesis: >
      CI pipeline users on non-Node stacks see enough Spectral rule parity and actionable
      violation output (with source locations) that they replace Spectral without modifying
      existing .spectral.yaml files.
    scope: see .ilmu/plan.md ## v0.2.0
    status: pr-open
  v1.0.0:
    hypothesis: public launch — promote when feature-complete and stable
    scope: TBD
    status: future

last_updated: 2026-04-14
last_agent: rust-teamlead (v0.2.0 impl)
