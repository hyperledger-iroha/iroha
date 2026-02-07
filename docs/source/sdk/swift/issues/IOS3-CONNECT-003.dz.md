---
lang: dz
direction: ltr
source: docs/source/sdk/swift/issues/IOS3-CONNECT-003.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 32500c1b3ede2614e953ef759dd4a37a6d1bf13749164fc940733ed1d95fde05
source_last_modified: "2025-12-29T18:16:36.075722+00:00"
translation_last_reviewed: 2026-02-07
title: IOS3-CONNECT-003 – Dataspace telemetry diagnostics
summary: Thread Connect queue/backpressure metrics through Swift telemetry exporters and surface public diagnostics publishers.
---

# IOS3-CONNECT-003 – Dataspace telemetry diagnostics

IOS3 requires that Swift expose queue/backpressure telemetry matching Android/JS
and feed OTLP metrics so operators can monitor Connect sessions. This issue
implements the diagnostics publisher referenced in the workshop plan.

## Scope

- Define `ConnectQueueDiagnostics` public struct with:
  - `queueDepth`, `avgLatency`, `resumeAttempts`, `lastHeartbeatAt`.
- Add `ConnectSession.dataspaceTelemetryPublisher(queue:)` that emits the
  diagnostics plus dataspace metadata for charts.
- Export OTLP counters/gauges via `SwiftTelemetryExporter`:
  - `connect.queue_depth`
  - `connect.queue_latency_seconds`
  - `connect.resume_attempts_total`
- Document configuration knobs in `docs/source/references/ios_metrics.md` and
  `docs/source/telemetry/swift_status_feeds.md`.
- Wire metrics into `scripts/swift_status_export.py` so weekly digests can alert
  if telemetry goes dark.

## Deliverables

1. Telemetry struct + publisher API in `ConnectSession.swift`.
2. Metric exporters in `IrohaSwift/Sources/IrohaSwift/Telemetry.swift`.
3. CI/unit tests:
   - `ConnectQueueDiagnosticsTests` verifying metric updates.
   - `scripts/tests/test_swift_status_export.py` covering new gauges.
4. Docs: update Connect workshop outputs + `status.md` once metrics land.

## Dependencies

- Relies on `IOS3-CONNECT-001/002` to surface diagnostics inside publishers.
- Needs OTLP collector credentials (Telemetry TL).
- Fixture pack `IOS3-CONNECT-004` supplies deterministic queue depth scenarios.

## Timeline

| Date | Action |
|------|--------|
| 2026-05-20 | Confirm diagnostics schema at workshop. |
| 2026-05-31 | Implement publishers + metrics. |
| 2026-06-03 | Telemetry smoke + status digest update. |

## Owners

- **Primary:** Telemetry TL.
- **Collaborators:** Swift Connect maintainers (API), SDK Program PM (docs).

## Status

- ✅ Workshop outcome: reuse `ConnectSessionDiagnostics` state/metrics as the
  canonical export (NDJSON + manifest). No additional bridge plumbing required
  for the roadmap gate; metrics flow through the recorder/export helpers already
  in the Connect stack.
- ✅ Fixture bundle + loader tests exercise queue/resume timelines and keep the
  NDJSON schema stable (`docs/source/sdk/swift/readiness/archive/2026-05/connect/`,
  `ConnectFixtureLoaderTests`).
- Follow-up for GA telemetry remains the OTLP collector wiring; not required to
  close the roadmap’s immediate TODOs and tracked separately in the telemetry
  backlog.
