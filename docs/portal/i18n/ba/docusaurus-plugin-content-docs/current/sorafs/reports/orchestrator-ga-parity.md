---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
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
matrix without rerunning the suite locally.
