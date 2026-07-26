<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Torii Mock Harness Toolkit

This toolkit packages the Android-developed Torii mock harness so every SDK can
exercise the same deterministic fixtures, retry policies, and telemetry hooks.
It wraps the harness implementation that already exists under
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/mock/`
and exposes a CLI for CI pipelines.

## Components

| Path | Purpose |
|------|---------|
| `cargo run -p xtask --bin torii-mock-harness` | Rust CLI that orchestrates the Java harness, records metrics, and computes fixture hashes. |
| `fixtures/` | Symlink to the Norito fixture bundle produced by `scripts/android_fixture_regen.sh`. |
| `config/default.toml` | Sample config mapping scenarios → endpoints, retry knobs, TLS material. |

## Usage

```bash
# Ensure fixtures are current (Tue/Fri 09:00 UTC cadence).
./scripts/android_fixture_regen.sh

# Run the "submit" scenario with telemetry enabled (recommended)
cargo run -p xtask --bin torii-mock-harness -- \
  --sdk android \
  --scenario submit

```

Flags:
- `--sdk <name>`: label metrics with the SDK under test (default: `android`).
- `--scenario <name>`: logical scenario name (documented in `config/default.toml`).
- `--tests <comma-separated-class-list>`: advanced option to override the default Java entrypoints.
- `--config <path>`: alternate scenario definition file (TOML).
- `--metrics-path <path>`: custom output for the Prometheus snippet (defaults to `mock-harness-metrics-<sdk>.prom` in the repo root).
- `--repo-root <path>` / `TORII_MOCK_HARNESS_REPO_ROOT`: override automatic repo-root detection.

Current scenarios: `submit` (baseline happy path), `status-retry` (404 status
backoff), and `governance` (status + governance ballot transport tests) running
against the Android harness; `submit-swift` exercises the Swift integration
tests, and `submit-js` runs the JavaScript smoke suite.

Use the `--runner` flag or the per-scenario `runner` key to point the CLI at the
appropriate harness script (e.g., `scripts/swift/mock_harness_smoke.sh` for
Swift or `javascript/iroha_js/scripts/mock_harness_smoke.sh` for JavaScript).

The launcher currently shells out to:

```bash
java/iroha_android/run_tests.sh \
  --tests org.hyperledger.iroha.android.client.HttpClientTransportHarnessTests
```

CI jobs should treat a non-zero exit code as a failure and publish the OTLP
metrics described below.

## Telemetry

Every run must emit the following metrics (also documented in
`docs/source/torii/norito_rpc_brief.md`):

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `torii_mock_harness_retry_total` | Counter | `sdk`, `scenario` | Number of retries triggered by injected failures. |
| `torii_mock_harness_duration_ms` | Gauge/summary | `sdk`, `scenario` | Wall-clock duration per scenario. |
| `torii_mock_harness_fixture_version` | Gauge | `sdk`, `fixture_version` | Tracks the Norito fixture hash used for the run. |

Sample OTLP exporters live under `scripts/telemetry/` (see `check_redaction_status.py`
for reference). SRE dashboards expect the metrics to show up in the staging stack
before the AND4 preview.

## CI Integration Checklist

1. Add a dedicated “Mock Harness Smoke” job to your SDK pipeline.
2. Invoke the CLI with the appropriate scenarios (`submit`, `status-retry`, `governance`).
3. Export OTLP metrics with the correct labels.
4. Upload logs/artifacts referencing the fixture version.
5. Report status in `status.md` until the job remains green for two consecutive weeks.

## Contact

Questions? Ping `#sdk-council` with the `torii-mock-harness` tag or reach out to
@android-net / @sdk-program directly.
