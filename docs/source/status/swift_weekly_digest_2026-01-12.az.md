---
lang: az
direction: ltr
source: docs/source/status/swift_weekly_digest_2026-01-12.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0397703543d74df5cee7f2fe93f57f5103f58877cb8319a9fd391dedbea32f62
source_last_modified: "2025-12-29T18:16:36.215048+00:00"
translation_last_reviewed: 2026-02-07
title: Swift SDK Weekly Digest (2026-01-12)
summary: Weekly Swift parity/CI update for the week of 2026-01-12.
---

# Swift SDK Weekly Digest — Week of 2026-01-12

## Metrics Snapshot

## Swift Norito Parity Snapshot (2026-01-12T10:00:00Z)

### Health Summary

- Parity: WARN (1 outstanding diffs)
- CI: WARN (ci/xcframework-smoke:iphone-sim success=93%)

- Outstanding diffs: 1 (oldest 12.0h)
- Regen SLA: last success 2026-01-12T08:30:00Z; 1.5h since success; breach=no
- Pipeline: status=green; last_run=2026-01-12T09:45:00Z; failed_tests=none
  Pipeline metadata: job=ci/xcode-swift-parity; duration=1425.5s
    Tests:
      - swift test: 1085.2s
      - make swift-dashboards: 94.4s
      - swift-status-export: 38.7s
  Buildkite metadata artifact: artifacts/mobile/parity/pipeline_metadata.json
- Acceleration: metal: enabled=True, parity=pass, perf_delta=-12.5%; neon: enabled=True, parity=pass; strongbox: enabled=False, parity=pending
- Telemetry: salt_epoch=2026Q1; rotation_age=96.0h; overrides=0; profile_alignment=ok; schema=ios_metrics/v2
  Notes:
    - Salt matches Rust/Android rotation.
    - No pending redaction overrides.
- Alerts: none

### Fixture Drift Summary

| Instruction | Age (h) | Owner |
| --- | ---: | --- |
| ContractRegister | 12.0 | swift-parity-oncall |

### CI Signals

- swift_parity_success_total: metrics state unavailable
- ci/xcode-swift-parity (parity): success=96% | last_failure=2026-01-11T23:12:00Z | flakes=1 | mttr=10.5h

### Swift CI Snapshot (2026-01-12T10:05:00Z)

- Queue depth: 2; avg runtime: 34.2 min
- Consecutive failures: 0; incidents: none
- Acceleration bench: metal_vs_cpu_merkle_ms: cpu=8.4 metal=3.6 (delta=-57.1%); neon_crc64_throughput_mb_s=920.5
- Device results: emulators 14/0 | strongbox 3/0
- Lane summary (last 14 runs):
- ci/xcode-swift-parity (parity): success=96% | last_failure=2026-01-11T23:12:00Z | flakes=1 | mttr=10.5h
- ci/xcframework-smoke:iphone-sim (iphone-sim): success=93% | last_failure=2026-01-10T18:02:00Z | flakes=1 | mttr=8.0h
- ci/xcframework-smoke:ipad-sim (ipad-sim): success=100% | last_failure=n/a | flakes=0 | mttr=0.0h
- ci/xcframework-smoke:strongbox (strongbox): success=100% | last_failure=n/a | flakes=0 | mttr=0.0h
- ci/xcframework-smoke:macos-fallback (mac-fallback): success=100% | last_failure=n/a | flakes=0 | mttr=0.0h

## Highlights

- Fixture rotation stayed on schedule; the sole outstanding diff (`ContractRegister`) remains within the 48 h SLA pending Norito exporter sign-off.
- CI lanes held ≥93% success over the trailing 14 runs, and the StrongBox pool continues to report clean passes with telemetry in lock-step.

## Risks & Mitigations

| Risk | Owner | Status | Notes |
|------|-------|--------|-------|
| Pending ContractRegister fixture regen could breach SLA if exporter hand-off slips | Swift Lead | Monitoring | Regen scheduled for the next Wednesday slot; fallback mirror ready if exporter delay persists. |
| Parity lane flake count trending upward | Swift QA Lead | In progress | Investigating `ci/xcode-swift-parity` retry logs; candidate fix to land with next Torii mock harness update. |

## Governance Watchers

| Item | Owner | Status | Notes |
|------|-------|--------|-------|
| `/v1/pipeline` adoption | Swift Lead | Green | Routing parity validated in staging; Torii backlog review scheduled for 2026-01-19 to approve the final interface lock. |
| Governance vote / readiness review | Program PM | On deck | GOV-2026-01-22 vote brief circulated; council to confirm `/v1/pipeline` gating metrics before beta cut. |
| Risk owner spotlight | SDK Parity WG | Tracking | Fixture drift table now auto-generated via `scripts/swift_status_export.py`; Buildkite lane links added per governance request. |

## Upcoming Actions

1. Regenerate the ContractRegister fixture once the exporter job lands so parity returns to zero outstanding diffs.
2. Root-cause the parity lane flake and backport fixes to the mock harness before the next weekly report.

## Links & Artefacts

- Parity feed: `dashboards/data/mobile_parity.sample.json`
- CI feed: `dashboards/data/mobile_ci.sample.json`
- Dashboards: `dashboards/mobile_parity.swift`, `dashboards/mobile_ci.swift`
