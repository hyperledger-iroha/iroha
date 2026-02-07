---
lang: uz
direction: ltr
source: docs/source/sdk/swift/xcframework_smoke_learnings.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ad642a9d40df3c379eda8f77dccd73462847fbf2159e2502f2e9b9f2c03034d6
source_last_modified: "2025-12-29T18:16:36.085326+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
title: XCFramework Smoke Harness Learnings
summary: Findings, mitigations, and follow-up actions from the Swift XCFramework smoke lanes.
---

# XCFramework Smoke Harness Learnings

The XCFramework smoke harness (Buildkite `ci/xcframework-smoke:*`) is now
running regularly. This note captures what we learned from the first runs and
documents the mitigations so the coordination action ("Share XCFramework smoke
harness learnings") can be marked complete.

## Observations

- **Lane matrix drift:** simulators and StrongBox-capable devices occasionally
  diverged because destination tags were not surfaced in the telemetry. We now
  require every lane to emit a `device_tag` and store it in the anomaly summary
  (`artifacts/xcframework_smoke_anomalies.json`) so dashboards can correlate
  failures with hardware pools.
- **Log redaction:** smoke logs contained bundle identifiers and absolute
  paths. `scripts/swift_smoke_anomalies.py` now strips PII and attaches a
  redaction summary block to the anomaly output.
- **NoritoBridge dependency checks:** early runs flaked when the bridge output
  was missing from `.build/`. The harness preflight now asserts the presence of
  `NoritoBridge.xcframework` before executing the suites and emits a targeted
  error when the artefact is missing.

## Actions Taken

- Updated `scripts/ci/run_xcframework_smoke.sh` to emit per-lane `device_tag`
  metadata and publish a consolidated anomaly summary.
- Refreshed the native bridge instrumentation checklist to require device tags,
  redaction verification, and NoritoBridge presence checks.
- Added a short facilitator note to the QA sync: review the anomaly summary
  weekly and file follow-ups when the same device tag appears more than twice.

## How to Reuse

1. Run the harness locally with `scripts/ci/run_xcframework_smoke.sh` (ensuring
   `NoritoBridge.xcframework` is present under `IrohaSwift/.build`).
2. Inspect `artifacts/xcframework_smoke_anomalies.json` for the device-tagged
   anomaly list and redact/log summaries.
3. Publish the JSON alongside CI logs so dashboards and `status.md` can cite the
   same evidence bundle.
