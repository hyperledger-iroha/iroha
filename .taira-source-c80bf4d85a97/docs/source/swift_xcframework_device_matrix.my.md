---
lang: my
direction: ltr
source: docs/source/swift_xcframework_device_matrix.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a145ff89a65ed28adb82897935c4d3088d7894a9c2243015945a4084321a2fe
source_last_modified: "2025-12-29T18:16:36.222681+00:00"
translation_last_reviewed: 2026-02-07
---

# Swift XCFramework Smoke Harness Device Matrix

This document captures the canonical device targets for the IOS6 XCFramework smoke
tests. Buildkite jobs and local verification scripts must adhere to this matrix so that
Swift and cross-SDK teams share identical coverage expectations.

## Key principles

1. **Deterministic coverage.** Every run exercises the same simulator/physical device
   set to minimise non-deterministic failures.
2. **StrongBox parity.** At least one lane executes on Secure Enclave/StrongBox capable
   hardware to validate key handling paths that simulators cannot emulate.
3. **Graceful degradation.** When the primary simulators are unavailable (e.g., Xcode
   upgrade lag or CI pool saturation), the harness falls back to running on the host
   macOS Catalyst target while emitting telemetry that the fallback was used.

The canonical defaults live in `configs/swift/xcframework_device_matrix.json`. The smoke
harness (`scripts/ci/run_xcframework_smoke.sh`) reads that file by default
(`IOS6_SMOKE_MATRIX_PATH` can override it for ad-hoc runs), and
`scripts/check_xcframework_device_pool.py` lints that the harness and Buildkite pipeline
stay aligned with the matrix.

## Required lanes

| Lane ID | Destination | Purpose | Notes |
|---------|-------------|---------|-------|
| `xcframework-smoke/iphone-sim` | `platform=iOS Simulator,name=iPhone 15` | Baseline SIMD/Metal smoke tests on the latest iPhone simulator. | Ensures Merkle/CRC benchmarks run with Metal acceleration enabled. |
| `xcframework-smoke/ipad-sim` | `platform=iOS Simulator,name=iPad (10th generation)` | Tablet form-factor coverage and larger memory footprint checks. | Verifies multi-window UI harness and attachment streaming. |
| `xcframework-smoke/strongbox` | `platform=iOS,name=iPhone 14 Pro,udid=<device-udid>` | StrongBox/Secure Enclave validation on physical hardware. | Requires hardware pool reservation; lane blocks until device available. |
| `xcframework-smoke/macos-fallback` | `platform=macOS,arch=arm64,variant=Designed for iPad` | Catalyst fallback path when simulators are missing. | Automatically scheduled only when prerequisite simulators cannot boot. |

### Destination helper variables

Buildkite jobs should populate the following environment variables so the harness and
fallback logic can be reused locally:

```bash
export IOS6_SMOKE_DEST_IPHONE_SIM="platform=iOS Simulator,name=iPhone 15"
export IOS6_SMOKE_DEST_IPAD_SIM="platform=iOS Simulator,name=iPad (10th generation)"
export IOS6_SMOKE_DEST_STRONGBOX="platform=iOS,name=iPhone 14 Pro,udid=$IPHONE_14P_UDID"
export IOS6_SMOKE_DEST_MAC_FALLBACK="platform=macOS,arch=arm64,variant=Designed for iPad"
```

The Swift harness script (`scripts/ci/run_xcframework_smoke.sh`) should
respect overrides supplied via the environment so local developers can test with
alternative destinations.

## Buildkite job definition

- Pipeline file: `.buildkite/xcframework-smoke.yml`
- Step key: `xcframework-smoke`
- Artifacts: `artifacts/xcframework_smoke_result.json`, `artifacts/xcframework_smoke_report.txt`
- Validation: `scripts/check_swift_dashboard_data.py` runs against the generated telemetry feed before uploading/annotating to guarantee schema compliance.
- The pipeline annotates the build with the rendered dashboard summary and fails the
  step if any lane reports a failure or incident.
- `scripts/ci/run_xcframework_smoke.sh` records Buildkite metadata keys of the form
  `ci/xcframework-smoke:<lane>:device_tag` (e.g., `iphone-sim`, `strongbox`) whenever
  the harness runs under Buildkite so downstream tooling can attribute metrics without
  parsing destination strings. The same tag is emitted in the telemetry JSON for
  dashboards/mobile_ci.swift.

## Fallback strategy

The harness resolves the requested simulator UDID via `simctl list`, issues up
to two `simctl boot` attempts while waiting on `bootstatus`, and captures
per-lane build/test logs under `artifacts/xcframework_smoke/<lane>/` before
queueing a fallback. This keeps simulator boot flakes from masquerading as test
failures and makes macOS Catalyst fallback decisions auditable.

1. Attempt to run the iPhone simulator lane. If Xcode reports that the runtime is
   unavailable or booting fails twice, log the error and enqueue the macOS fallback lane.
2. Attempt the iPad simulator lane under the same retry policy; fallback to macOS run.
3. Always queue the StrongBox lane; if the device pool is busy, the job waits up to 30
   minutes before marking the step as `soft_failed` and notifying the hardware channel.
4. When any fallback is triggered, emit a telemetry event (`connect.queue_depth` style)
   with `swift_ci_fallback=1` so dashboards can highlight degraded coverage.

## Telemetry expectations

The CI job must append the following fields to the `mobile_ci` feed:

| Field | Description |
|-------|-------------|
| `devices.emulators.passes` / `.failures` | Aggregated results of simulator lanes. |
| `devices.strongbox_capable.passes` / `.failures` | Outcomes from the physical device lane. |
| `alert_state.open_incidents[]` | Include `"xcframework_smoke_fallback"` when the macOS lane substitutes a simulator. |

Buildkite step metadata should include `device_tag=iphone-sim`, `device_tag=ipad-sim`,
`device_tag=strongbox`, or `device_tag=mac-fallback` so dashboard tooling can attribute
results without relying on destination strings.

## Ownership & maintenance

- **Swift QA Lead** owns updates to this matrix and ensures new device generations are
  added each autumn.
- **Build Infra** maintains the hardware pool and pipeline configuration, revisiting the
  fallback timeouts quarterly.
- **Android/JS observers** rely on the StrongBox lane for cross-platform parity metrics;
  notify the Swift QA lead if the lane reports persistent failures (>3 consecutive runs).

Please update this document whenever the matrix changes and reflect the new destinations
in `status.md` and `roadmap.md` within the same change.
