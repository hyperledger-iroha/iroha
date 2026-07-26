---
lang: ka
direction: ltr
source: docs/source/testing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d8d93d59657ba18054401ed63266869195e1366097fc5435bb3d3502b6230817
source_last_modified: "2026-01-28T09:46:24.515832+00:00"
translation_last_reviewed: 2026-02-07
---

# Testing and Troubleshooting Guide

This guide explains how to reproduce integration scenarios, what infrastructure needs to be online, and how to collect actionable logs. Refer back to the project-wide [status report](../../status.md) before starting so you know which components are currently green.

## Reproduction Steps

### Integration tests (`integration_tests` crate)

1. Ensure the workspace dependencies are built: `cargo build --workspace`.
2. Run the integration test suite with full logs: `cargo test -p integration_tests -- --nocapture`.
3. If you need to rerun a specific scenario, use its module path, e.g. `cargo test -p integration_tests settlement::happy_path -- --nocapture`.
4. Capture serialized fixtures with Norito to guarantee consistent inputs across nodes:
   ```rust
   use norito::json;

   let genesis_payload = json::to_string_pretty(&json::json!({
       "chain" : "testnet",
       "peers" : ["127.0.0.1:1337"],
       "accounts" : [{
           "id" : "<i105-account-id>",
           "public_key" : "ed0120..."
       }]
   }))?;
   ```
   Store the resulting Norito JSON alongside the test artifacts so that peers can replay the same state.

### Python client tests (`pytests` directory)

1. Install the Python requirements with `pip install -r pytests/requirements.txt` in a virtual environment.
2. Export the Norito-compliant fixtures generated above via a shared path or environment variable.
3. Run the suite with verbose output: `pytest -vv pytests`.
4. For targeted debugging, run `pytest -k "Query" pytests/tests/test_queries.py --log-cli-level=INFO`.

## Required Ports and Services

The following services must be reachable before executing either suite:

- **Torii HTTP API**: default `127.0.0.1:1337`. Override via `torii.address` in your config (see `docs/source/references/peer.template.toml`).
- **Torii WebSocket notifications**: default `127.0.0.1:8080` for client subscriptions used by `pytests`.
- **Telemetry exporter**: default `127.0.0.1:8180`. Integration tests expect metrics to stream here for health assertions.
- **PostgreSQL** (when enabled): default `127.0.0.1:5432`. Ensure credentials align with the compose profile in [`defaults/docker-compose.local.yml`](../../defaults/docker-compose.local.yml).

Check the [telemetry troubleshooting guide](telemetry.md) if any endpoint is unavailable.

### Embedded peer stability

`NetworkBuilder::start()` now enforces a five-second post-genesis liveness window for every
embedded peer. If a process exits during this guard period the builder aborts with a detailed
error that points to the cached stdout/stderr logs. On resource-constrained machines you can
extend the window (in milliseconds) by setting `IROHA_TEST_POST_GENESIS_LIVENESS_MS`; lowering it
to `0` disables the guard entirely. Ensure your environment leaves enough CPU headroom during the
first few seconds of every integration suite so peers can reach block 1 without tripping the
watchdog.

### Unit-test networking (`IROHA_TEST_REAL_NETWORK`)

Unit tests in `iroha_core` default to closed P2P handles to avoid cross-test network races.
Opt in to real networking only when a unit test must exercise OS-level socket behavior that
cannot be covered by integration tests or mocked channels.

Use real P2P in unit tests only if all of the following are true:

- The behavior depends on actual socket binding/accept/backpressure or TLS handshakes.
- The test runs with explicit timeouts and cleans up sockets; no reliance on shared ports.
- The test doc comment explains why integration tests are insufficient for the scenario.

Otherwise, keep unit tests hermetic and cover network flows in `integration_tests` or Izanami
scenarios. To run opt-in tests locally:

```bash
IROHA_TEST_REAL_NETWORK=1 cargo test -p iroha_core <test_name> -- --nocapture
```

## Log Collection and Analysis

Start from a clean run directory so previous artifacts do not hide new issues. The scripts below collect logs in formats that downstream Norito tooling can consume.

- Use [`scripts/analyze_telemetry.sh`](../../scripts/analyze_telemetry.sh) after test execution to aggregate node metrics into timestamped Norito JSON snapshots.
- When investigating networking issues, run [`scripts/run_iroha_monitor_demo.py`](../../scripts/run_iroha_monitor_demo.py) to stream Torii events into `monitor_output.norito.json`.
- Integration test logs are stored under `integration_tests/target/`; compress them with [`scripts/profile_build.sh`](../../scripts/profile_build.sh) for sharing with other teams.
- Python client logs are written to `pytests/.pytest_cache`. Export them alongside captured telemetry with:
  ```bash
  ./scripts/report_red_team_failures.py --tests pytests --artifacts out/logs
  ```

Collect a full bundle (integration, Python, telemetry) before opening an issue so the maintainers can replay the Norito traces.

### Izanami validation timing logs

When profiling validation latency in Izanami runs, enable main-loop debug logs so each block
emits a `block validation timings` line with `stateless_ms`, `execution_ms`, and `total_ms`.
Use `RUST_LOG=iroha_core::sumeragi::main_loop=debug` (or append it to `--log-filter`) and capture
the peer logs for analysis.

## Next Steps

For release-specific checklists, see [pipeline](pipeline.md). If you uncover regressions or failures, document them in the shared [status tracker](../../status.md) and cross-reference any relevant [sumeragi troubleshooting](sumeragi.md) entries.
