# AGENTS Instructions

These guidelines apply to the `integration_tests/` crate.

## Layout
- `src/` hosts reusable harness code (metrics parsing, sandbox helpers, SoraFS gateway coverage) plus the `sorafs_gateway_fixtures` binary (`cargo run -p integration_tests --bin sorafs-gateway-fixtures`) that regenerates gateway fixtures.
- `tests/` is the primary suite of Rust integration tests (genesis, Sumeragi, permissions, Norito streaming, triggers, telemetry, SoraFS, etc.). Cargo now exposes six explicit harness crates:
  - `core_api`
  - `events_and_triggers`
  - `queries_and_proofs`
  - `network_functional`
  - `consensus_and_da`
  - `nexus_and_streaming`
- Scenario files remain under `tests/` and are pulled into those harnesses with `#[path = ...]`; keep a `//!` header on each scenario file.
- `integration_tests/tests/pipeline_block_rejected.rs` is an additional scaffold kept under `#[ignore]` until a deterministic trigger exists. Run it with `IROHA_RUN_IGNORED=1 cargo test -p integration_tests --test core_api pipeline_block_rejected:: -- --ignored`.
- `fixtures/` contains pre-baked inputs (e.g., `ivm/*.to`, `sumeragi_*`, `norito_streaming/rans/*.json`). `build.rs` reuses `crates/ivm` to refresh the `.to` programs automatically during `cargo test`.

## Features and environment
- Feature flags:
  - `telemetry` (default) enables metrics assertions.
  - `fault_injection` opens hooks used by adversarial Sumeragi tests.
  - `norito_streaming_fec` pulls in Reed–Solomon helpers for FEC regression coverage.
  - `js_host_parity` mirrors Kotodama host tests inside JS targets.
- Some tests require optional data:
  - Set `IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR=1` before building if you need the default executor sample in `fixtures/ivm`.
  - Long-running or flaky suites stay `#[ignore]`; run with `IROHA_RUN_IGNORED=1 cargo test -p integration_tests -- --ignored --nocapture`.
- The crate depends on `iroha_test_network` to spin up peers in-process; no external docker-compose environment is expected.

## Development workflow
- Keep new tests deterministic: avoid wall-clock sleeps, unbounded retries, or randomness without a seeded RNG.
- Prefer table-driven tests and explicit timeouts for networking and telemetry polling.
- When adding a new fixture, document it in `README.md`, keep it under `fixtures/`, and add a regeneration helper when possible (for example use `sorafs_gateway_fixtures` for gateway data).
- If a shared wire-format fixture or parity test currently checks Java SDK classes, extend the coverage so the Kotlin SDK is checked against the same fixture corpus too.
- Always run `cargo test -p integration_tests` before sending changes. To target a subset use `cargo test -p integration_tests --test <harness> <filter> -- --nocapture`.
- Prefer `cargo test -p integration_tests --test <harness> <filter> -- --nocapture` for targeted reruns; exact filters are module-qualified inside each grouped harness.
- `scripts/run_full_tests.sh` now reuses the workspace-built binaries for the network suite and provisions an isolated `IROHA_TEST_NETWORK_PERMIT_DIR` unless the caller already set one.
- For local throughput, `scripts/run_full_tests.sh --fast` uses `scripts/cargo_fast.sh`; `--fast-zero-debug` and `--fast-no-incremental` are available when you explicitly want that more aggressive mode.
- If a test requires an external binary or script, assert its presence (e.g., via `which`) and provide a helpful error.

## Documentation
- Each test crate must start with `//!` documentation describing the scenario, assumptions, and required features.
- Update `integration_tests/README.md` when introducing new suites, fixtures, or feature flags so CI and release engineers understand how to exercise them.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
