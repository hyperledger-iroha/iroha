---
lang: es
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-18T17:14:31.034360+00:00"
translation_last_reviewed: 2026-01-30
---

# AGENTS Development Workflow

This runbook consolidates the contributor guardrails from the AGENTS roadmap so
new patches follow the same default gates.

## Quickstart targets

- Run `make dev-workflow` (wrapper around `scripts/dev_workflow.sh`) to execute:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test` from `IrohaSwift/`
- `cargo test --workspace` is long-running (often hours). For quick iterations,
  use `scripts/dev_workflow.sh --skip-tests` or `--skip-swift`, then run the full
  sequence before shipping.
- If `cargo test --workspace` stalls on build directory locks, rerun with
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (or set
  `CARGO_TARGET_DIR` to an isolated path) to avoid contention.
- All cargo steps use `--locked` to respect the repository policy of keeping
  `Cargo.lock` untouched. Prefer extending existing crates rather than adding
  new workspace members; seek approval before introducing a new crate.

## Guardrails

- `make check-agents-guardrails` (or `ci/check_agents_guardrails.sh`) fails if a
  branch modifies `Cargo.lock`, introduces new workspace members, or adds new
  dependencies. The script compares the working tree and `HEAD` against
  `origin/main` by default; set `AGENTS_BASE_REF=<ref>` to override the base.
- `make check-dependency-discipline` (or `ci/check_dependency_discipline.sh`)
  diffs `Cargo.toml` dependencies against the base and fails on new crates; set
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` to acknowledge intentional
  additions.
- `make check-missing-docs` (or `ci/check_missing_docs_guard.sh`) blocks new
  `#[allow(missing_docs)]` entries, flags touched crates (nearest `Cargo.toml`)
  whose `src/lib.rs`/`src/main.rs` lack crate-level `//!` docs, and rejects new
  public items without `///` docs relative to the base ref; set
  `MISSING_DOCS_GUARD_ALLOW=1` only with reviewer approval. The guard also
  verifies that `docs/source/agents/missing_docs_inventory.{json,md}` are fresh;
  regenerate with `python3 scripts/inventory_missing_docs.py`.
- `make check-tests-guard` (or `ci/check_tests_guard.sh`) flags crates whose
  changed Rust functions lack unit-test evidence. The guard maps changed lines
  to functions, passes if crate tests changed in the diff, and otherwise scans
  existing test files for matching function calls so pre-existing coverage
  counts; crates without any matching tests will fail. Set `TEST_GUARD_ALLOW=1`
  only when changes are truly test-neutral and the reviewer agrees.
- `make check-docs-tests-metrics` (or `ci/check_docs_tests_metrics_guard.sh`)
  enforces the roadmap policy that milestones move alongside documentation,
  tests, and metrics/dashboards. When `roadmap.md` changes relative to
  `AGENTS_BASE_REF`, the guard expects at least one doc change, one test change,
  and one metrics/telemetry/dashboard change. Set `DOC_TEST_METRIC_GUARD_ALLOW=1`
  only with reviewer approval.
- `make check-todo-guard` (or `ci/check_todo_guard.sh`) fails when TODO markers
  disappear without accompanying docs/tests changes. Add or update coverage
  when resolving a TODO, or set `TODO_GUARD_ALLOW=1` for intentional removals.
- `make check-std-only` (or `ci/check_std_only.sh`) blocks `no_std`/`wasm32`
  cfgs so the workspace stays `std`-only. Set `STD_ONLY_GUARD_ALLOW=1` only for
  sanctioned CI experiments.
- `make check-status-sync` (or `ci/check_status_sync.sh`) keeps the roadmap open
  section free of completed items and requires `roadmap.md`/`status.md` to
  change together so plan/status stay aligned; set
  `STATUS_SYNC_ALLOW_UNPAIRED=1` only for rare status-only typo fixes after
  pinning `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (or `ci/check_proc_macro_ui.sh`) runs the trybuild
  UI suites for derive/proc-macro crates. Run it when touching proc-macros to
  keep `.stderr` diagnostics stable and catch panicking UI regressions; set
  `PROC_MACRO_UI_CRATES="crate1 crate2"` to focus on specific crates.
- `make check-env-config-surface` (or `ci/check_env_config_surface.sh`) rebuilds
  the env-toggle inventory (`docs/source/agents/env_var_inventory.{json,md}`),
  fails if it is stale, **and** fails when new production env shims appear
  relative to `AGENTS_BASE_REF` (auto-detected; set explicitly when needed).
  Refresh the tracker after adding/removing env lookups via
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  use `ENV_CONFIG_GUARD_ALLOW=1` only after documenting intentional env knobs
  in the migration tracker.
- `make check-serde-guard` (or `ci/check_serde_guard.sh`) regenerates the serde
  usage inventory (`docs/source/norito_json_inventory.{json,md}`) into a temp
  location, fails if the committed inventory is stale, and rejects any new
  production `serde`/`serde_json` hits relative to `AGENTS_BASE_REF`. Set
  `SERDE_GUARD_ALLOW=1` only for CI experiments after filing a migration plan.
- `make guards` enforces the Norito serialization policy: it denies new
  `serde`/`serde_json` usage, ad-hoc AoS helpers, and SCALE dependencies outside
  the Norito benches (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Proc-macro UI policy:** every proc-macro crate must ship a `trybuild`
  harness (`tests/ui.rs` with pass/fail globs) behind the `trybuild-tests`
  feature. Place happy-path samples under `tests/ui/pass`, rejection cases under
  `tests/ui/fail` with committed `.stderr` outputs, and keep diagnostics
  non-panicking and stable. Refresh fixtures with
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (optionally with
  `CARGO_TARGET_DIR=target-codex` to avoid clobbering existing builds) and
  avoid relying on coverage builds (`cfg(not(coverage))` guards are expected).
  For macros that do not emit a binary entrypoint, prefer
  `// compile-flags: --crate-type lib` in fixtures to keep errors focused. Add
  new negative cases whenever diagnostics change.
- CI runs the guardrail scripts via `.github/workflows/agents-guardrails.yml`
  so pull requests fail fast when the policies are violated.
- The sample git hook (`hooks/pre-commit.sample`) runs guardrail, dependency,
  missing-docs, std-only, env-config, and status-sync scripts so contributors
  catch policy violations before CI. Keep TODO breadcrumbs for any intentional
  follow-ups instead of deferring large changes silently.
