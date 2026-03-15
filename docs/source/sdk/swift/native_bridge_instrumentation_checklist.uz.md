---
lang: uz
direction: ltr
source: docs/source/sdk/swift/native_bridge_instrumentation_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5aa3070830b63a886b2bfe8ee4c941e5493801b0da12cf1a9d5db0358a602d88
source_last_modified: "2025-12-29T18:16:36.076658+00:00"
translation_last_reviewed: 2026-02-07
title: Swift Native Bridge Instrumentation Checklist
summary: Playbook for wiring NoritoBridge load/fallback signals, Connect codec telemetry, and XCFramework smoke feeds ahead of IOS6.
---

# Swift Native Bridge Instrumentation Checklist

This checklist maps the instrumentation requirements for the Swift native bridge
(`IrohaSwift/Sources/IrohaSwift/NativeBridge.swift`) to the telemetry surfaces
consumed by the IOS6 roadmap work. Follow it whenever the bridge surface
changes, the XCFramework harness is updated, or new telemetry sinks are added.

## 1. Signals to Capture

| Signal | Source | Sink / Evidence | Notes |
|--------|--------|-----------------|-------|
| Bridge load status & version | `NoritoNativeBridge.shared` loader (`NativeBridge.swift:13`, path-based candidate probing) | `connect.error` events emitted via `ConnectError.telemetryAttributes` (`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`) and `status.md` excerpts | Emit a `connect.error` event tagged `category=internal code=norito_bridge.load_failure` when the loader falls back to JSON; record version/build metadata when the handle resolves successfully. |
| Connect codec bridge enforcement | `ConnectCodec.encode/decode` (`IrohaSwift/Sources/IrohaSwift/ConnectCodec.swift`) | `connect.error` telemetry + parity feeds | Alert whenever `ConnectCodecError.bridgeUnavailable` is raised so dashboards can confirm the XCFramework ships with every release and fail closed when the bridge is missing. |
| Sorafs orchestrator reports | `sorafsLocalFetch` (`NativeBridge.swift:1982`) and fixtures (`fixtures/sorafs_orchestrator/README.md`) | CI/SDK parity harness (`ci/sdk_sorafs_orchestrator.sh`) and `docs/source/sorafs/reports/orchestrator_ga.md` | Persist `reportJSON` produced by the bridge, include it in the fixture bundle, and ensure it flows into the shared parity tests. |
| XCFramework smoke lanes & device tags | `scripts/ci/run_xcframework_smoke.sh` → `dashboards/mobile_ci.swift` | `artifacts/xcframework_smoke_result.json`, `artifacts/xcframework_smoke_anomalies.json`, `dashboards/data/mobile_ci*.json`, Buildkite metadata `ci/xcframework-smoke:<lane>:device_tag` | Required so dashboards expose StrongBox coverage and macOS fallbacks; validated via `scripts/check_swift_dashboard_data.py`. |

## 2. Prerequisites & Tooling

- `scripts/build_norito_xcframework.sh` – rebuilds the bridge XCFramework before running instrumentation drills.
- `scripts/ci/run_xcframework_smoke.sh` – produces telemetry JSON, anomaly summary, and per-lane logs for IOS6.
- `scripts/check_swift_dashboard_data.py` – schema validator for the CI/parity feeds referenced in `docs/source/references/ios_metrics.md`.
- `ci/sdk_sorafs_orchestrator.sh` – replays the multi-provider fetch parity suite to guarantee consistent `reportJSON` outputs across SDKs.
- Hardware: simulator pool defined in `docs/source/swift_xcframework_device_matrix.md` plus the StrongBox device recorded in `docs/source/swift_xcframework_hardware_plan.md`.

## 3. Checklist

### 3.1 Bridge Load & Fallback Detection

1. **Build and point the bridge.**
   ```bash
   ./scripts/build_norito_xcframework.sh
   # Loader discovers dist/NoritoBridge.xcframework automatically.
   ```
2. **Verify availability telemetry.**
   - Run `swift test --filter TxBuilderTests --package-path IrohaSwift`.
   - Ensure the tests that skip on `!NoritoNativeBridge.shared.isAvailable` now run.
   - Confirm the logger/telemetry hooks emit a `connect.error` with `code=norito_bridge.available` when the bridge loads and `code=norito_bridge.load_failure` when the xcframework is missing or malformed.
3. **Record version info.**
   - Capture the SHA + build date from `dist/NoritoBridge.xcframework/Info.plist` and attach it to the `status.md` update or release note.

### 3.2 Connect Codec Instrumentation

1. **Exercise encode/decode parity.**
   ```bash
   swift test --filter ConnectEnvelopeTests --package-path IrohaSwift
   swift test --filter ConnectSessionTests --package-path IrohaSwift
   ```
2. **Check fail-closed telemetry.**
   - Call `NoritoNativeBridge.shared.overrideConnectCodecAvailabilityForTests(false)` so `ConnectCodec` raises `ConnectCodecError.bridgeUnavailable`.
   - Confirm that the telemetry exporter records a `connect.error` with the new bridge-unavailable code and that dashboards alarm when the XCFramework is missing.
3. **Validate taxonomy alignment.**
   - Run `swift test --filter ConnectErrorTests --package-path IrohaSwift` to ensure the error taxonomy feeding telemetry stays stable (`docs/source/connect_error_taxonomy.md`).

### 3.3 Sorafs Orchestrator Reports

1. **Regenerate fixtures when inputs change.**
   ```bash
   python3 scripts/build_sorafs_orchestrator_fixture.py
   ```
2. **Run the SDK parity harness.**
   ```bash
   ci/sdk_sorafs_orchestrator.sh
   ```
   - Inspect `artifacts/sorafs_orchestrator/*.json` and verify each SDK writes the same `reportJSON`.
3. **Propagate telemetry region tags.**
   - Ensure `SorafsOptions.telemetryRegion` is set in the harness (`IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`) so emitted Norito/JSON include the region metadata that dashboards expect.

### 3.4 XCFramework Smoke & Dashboard Export

1. **Execute the smoke harness on the full device matrix.**
   ```bash
   scripts/ci/run_xcframework_smoke.sh
   ```
   - Confirm `artifacts/xcframework_smoke_result.json` lists every lane (`iphone-sim`, `ipad-sim`, `strongbox`, optional `macos-fallback`) and that Buildkite metadata contains the corresponding `device_tag`. The anomaly summary is written to `artifacts/xcframework_smoke_anomalies.json` for release/incident bundles.
2. **Validate the CI feed.**
   ```bash
   scripts/check_swift_dashboard_data.py artifacts/xcframework_smoke_result.json \
     --ci-lane-threshold ci/xcframework-smoke:iphone-sim>=0.95
   ```
3. **Render the dashboard preview.**
   ```bash
   ./dashboards/mobile_ci.swift artifacts/xcframework_smoke_result.json > artifacts/xcframework_ci_report.txt
   ```
   Attach the text summary to the build artefacts so reviewers can spot regressions without running the script locally.

## 4. Evidence & Reporting

- Archive raw telemetry under `artifacts/xcframework_smoke/` (per-lane logs) and the JSON result file referenced above.
- Capture the anomaly summary (`artifacts/xcframework_smoke_anomalies.json`) alongside the telemetry export so incident responders have a redacted failure digest without digging through full logs.
- Update `status.md` (Latest Updates) with a one-line summary and link back to this checklist whenever instrumentation changes ship.
- In `roadmap.md`, keep the IOS6 “Native bridge instrumentation checklist” row pointing here and update the status emoji when the review completes.
- Cross-link release PRs with the relevant artefacts (bridge version, telemetry JSON, dashboard output) so auditors can replay the evidence.

## 5. Escalation

- If the bridge fails to load on any device, open an incident referencing the `ci/xcframework-smoke:<lane>` metadata key and page the Swift QA on-call.
- For telemetry schema regressions, block merges by running `make swift-dashboards` (which invokes `ci/check_swift_dashboards.sh`) and attach the failing JSON plus validator output to the PR.

Keep this document in sync with `docs/source/references/ios_metrics.md`, the XCFramework hardware plan, and the Sorafs orchestrator fixtures to maintain a single source of truth for bridge instrumentation.
