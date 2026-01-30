---
lang: fr
direction: ltr
source: docs/source/sorafs/reports/orchestrator_ga.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4585f048b02d8dedf86dcca5b647d8c048a15a61055ba4791a23a4aa3bf8a905
source_last_modified: "2026-01-03T18:07:58.627124+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Orchestrator GA Parity Report

Deterministic multi-fetch parity is now tracked per SDK so release engineers can confirm that
payload bytes, chunk receipts, provider reports, and scoreboard outcomes remain aligned across
implementations. Every harness consumes the canonical multi-provider bundle under
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, which packages the SF1 plan, provider
metadata, telemetry snapshot, and orchestrator options.

## Rust Baseline

- **Command:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Scope:** Runs the `MultiPeerFixture` plan twice via the in-process orchestrator, verifying
  assembled payload bytes, chunk receipts, provider reports, and scoreboard outcomes. Instrumentation
  also tracks peak concurrency and effective working-set size (`max_parallel × max_chunk_length`).
- **Performance guard:** Each run must complete within 2 s on CI hardware.
- **Working set ceiling:** With the SF1 profile the harness enforces `max_parallel = 3`, yielding a
  ≤ 196 608 byte window.

Sample log output:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK Harness

- **Command:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Scope:** Replays the same fixture via `iroha_js_host::sorafsMultiFetchLocal`, comparing payloads,
  receipts, provider reports, and scoreboard snapshots across consecutive runs.
- **Performance guard:** Each execution must finish within 2 s; the harness prints the measured
  duration and reserved-byte ceiling (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Example summary line:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK Harness

- **Command:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Scope:** Runs the parity suite defined in `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  replaying the SF1 fixture twice through the Norito bridge (`sorafsLocalFetch`). The harness verifies
  payload bytes, chunk receipts, provider reports, and scoreboard entries using the same deterministic
  provider metadata and telemetry snapshots as the Rust/JS suites.
- **Bridge bootstrap:** The harness unpacks `dist/NoritoBridge.xcframework.zip` on demand and loads
  the macOS slice via `dlopen`. When the xcframework is missing or lacks the SoraFS bindings, it
  falls back to `cargo build -p connect_norito_bridge --release` and links against
  `target/release/libconnect_norito_bridge.dylib`, so no manual setup is required in CI.
- **Performance guard:** Each execution must finish within 2 s on CI hardware; the harness prints the
  measured duration and reserved-byte ceiling (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Example summary line:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python Bindings Harness

- **Command:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope:** Exercises the high-level `iroha_python.sorafs.multi_fetch_local` wrapper and its typed
  dataclasses so the canonical fixture flows through the same API that wheel consumers call. The test
  rebuilds the provider metadata from `providers.json`, injects the telemetry snapshot, and verifies
  payload bytes, chunk receipts, provider reports, and scoreboard content just like the Rust/JS/Swift
  suites.
- **Pre-req:** Run `maturin develop --release` (or install the wheel) so `_crypto` exposes the
  `sorafs_multi_fetch_local` binding before invoking pytest; the harness auto-skips when the binding
  is unavailable.
- **Performance guard:** Same ≤ 2 s budget as the Rust suite; pytest logs the assembled byte count
  and provider participation summary for the release artefact.

Release gating should capture the summary output from every harness (Rust, Python, JS, Swift) so the
archived report can diff payload receipts and metrics uniformly before promoting a build. Run
`ci/sdk_sorafs_orchestrator.sh` to execute every parity suite (Rust, Python bindings, JS, Swift) in
one pass; CI artifacts should attach the log excerpt from that helper plus the generated
`matrix.md` (SDK/status/duration table) to the release ticket so reviewers can audit the parity
matrix without rerunning the suite locally. Hosted option: trigger or reference the
`sorafs-orchestrator-sdk` workflow (`.github/workflows/sorafs-orchestrator-sdk.yml`) which runs the
same script on macOS CI and uploads the parity bundle (summary, matrix, fixture snapshot) ready for
release tickets.

## Binding Evidence Matrix

Use the matrix below to record which commit introduced the GA-critical knobs across each binding.
The goal is to keep observer hooks, telemetry relays, and retry controls aligned before flipping the
default orchestrator switch. When a binding gains another feature, append the commit hash and file
so reviewers can audit the implementation without spelunking through history.

| Binding | Observer support | Telemetry relays | Retry knobs |
|---------|------------------|------------------|-------------|
| Rust CLI / SDK | `f0adaf7b` – `crates/sorafs_orchestrator/src/lib.rs` exposes `Orchestrator::local_proxy` and chunk observers that feed the CLI + `iroha` client wrappers referenced in §8 of the plan. | `f0adaf7b` – `crates/iroha_telemetry/src/metrics.rs` emits the `sorafs.fetch.*` OTEL events (`SorafsFetchOtel`) and Prometheus gauges consumed by the rollout dashboards. | `ae0abecf` – `crates/sorafs_car/src/bin/sorafs_fetch.rs` wires the shared `--max-peers` / `--retry-budget` flags (plus help text) that SDK bindings are required to mirror. |
| JavaScript SDK | `c3bf87d6` – `javascript/iroha_js/src/sorafs.js` normalises `localProxy` / observer options before invoking the native binding so browser/Node clients inherit the same streaming hooks. | `c3bf87d6` – `javascript/iroha_js/src/sorafs.js` forwards telemetry regions, scoreboard persistence, and labels; Torii peer telemetry exposure reuses `f0adaf7b` (`javascript/iroha_js/src/toriiClient.js`). | `c3bf87d6` – `javascript/iroha_js/src/sorafs.js` plus `f0adaf7b` (`javascript/iroha_js/index.d.ts`) export the `maxPeers` / `retryBudget` options with typed definitions for downstream TypeScript consumers. |
| Swift SDK | `f0adaf7b` – `IrohaSwift/Sources/IrohaSwift/NativeBridge.swift` streams `sorafsLocalFetch` through the Norito bridge so observer/parity harnesses share the Rust implementation. | `f0adaf7b` – `IrohaSwift/Sources/IrohaSwift/NativeBridge.swift` preserves `telemetryRegion` and emits the parity telemetry snapshots referenced in `SorafsOrchestratorParityTests`. | `16e47f93` – `IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift` encodes the shared `maxPeers`, `retryBudget`, rollout phase, and policy override knobs exposed by the CLI. |
| Python SDK | `29f5eeaf` – `python/iroha_python/iroha_python_rs/src/lib.rs` reuses the Rust orchestrator for `sorafs_multi_fetch_local_py` and now raises `SorafsMultiFetchError`, keeping observer/state-machine semantics identical to the CLI parity harness. | `29f5eeaf` – `python/iroha_python/src/iroha_python/sorafs.py` preserves the scoreboard/telemetry knobs (`scoreboard_out_path`, `scoreboard_telemetry_label`, `sdk:python` metadata) that feed the GA evidence bundle. | `29f5eeaf` – `python/iroha_python/src/iroha_python/sorafs.py` exposes the same `retry_budget`, `max_parallel`, `max_peers`, and policy overrides surfaced by the CLI, so downstream callers share the GA retry contract. |

> **How to use:** keep the table up to date whenever a binding gains or changes one of these knobs.
> Release tickets should point to the hashes above when claiming GA readiness so reviewers know
> exactly which source files implement the shared behaviour.
