---
lang: am
direction: ltr
source: docs/source/sdk/swift/issues/IOS3-CONNECT-001.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 48f456b55df4db310df6451822e27ec34f1c719a8a1ddc57d8881aa133fec545
source_last_modified: "2025-12-29T18:16:36.074782+00:00"
translation_last_reviewed: 2026-02-07
title: IOS3-CONNECT-001 – Connect balance publisher
summary: Ship Combine + AsyncStream helpers that expose Connect balance frames with queue diagnostics for IOS3 parity.
---

# IOS3-CONNECT-001 – Connect balance publisher

Roadmap milestone **IOS3 — Torii/Nexus API Coverage** requires high-level
Combine/async facades for Connect flows. This issue tracks the balance stream
publisher called out in `docs/source/sdk/swift/connect_workshop.md` and the
IOS3 combine plan.

## Scope

- Add `ConnectSession.balancePublisher(accountID:)` returning
  `AnyPublisher<ConnectBalanceSnapshot, ConnectSessionError>`.
- Provide an `AsyncThrowingStream` factory with the same filtering semantics so
  callers without Combine can still consume balance updates.
- Surface queue diagnostics via a lightweight struct (`ConnectQueueDiagnostics`)
  that exposes `queueDepth`, `latency`, and `resumeAttempts`.
- Emit telemetry counters/gauges:
  - `swift_ios3_connect_balance_events_total{status="ok|error"}`
  - `swift_ios3_connect_queue_depth` (gauge) derived from diagnostics.
- Thread retry/backoff/telemetry knobs from `IrohaConfig.Connect`.

## Deliverables

1. API additions in `IrohaSwift/Sources/IrohaSwift/ConnectSession.swift`.
2. Combine helper tests in
   `IrohaSwift/Tests/IrohaSwiftTests/ConnectSessionBalanceTests.swift`.
3. Async stream coverage inside `ConnectSessionTests.swift`.
4. Doc updates:
   - `docs/source/sdk/swift/index.md` (API surface).
   - `docs/source/connect_architecture_strawman.md` (queue ownership, telemetry).
5. Telemetry wiring + exporter docs (`docs/source/references/ios_metrics.md`).

## Tests

- New Combine-focused test suite verifying publish/receive/cancel semantics.
- Regression test for diagnostics (ensures queue depth and latency propagate).
- Telemetry unit test verifying counters increment (e.g.
  `scripts/tests/test_swift_status_export.py` covers metrics ingestion).

## Dependencies

- `NoritoBridge.xcframework` must include Connect crypto helpers (already true).
- WebSocket replay fixtures from `IOS3-CONNECT-004` provide deterministic queue
  states for Combined tests.

## Milestones & Timeline

| Date | Action |
|------|--------|
| 2026-05-20 | Review API at IOS3 Connect workshop. |
| 2026-05-27 | Land API + tests in main branch (target). |
| 2026-05-30 | Update docs/status with telemetry evidence. |

## Owners

- **Primary:** Swift Connect maintainers.
- **Reviewers:** Telemetry TL (metrics), SDK Program PM (docs/status).

## Status

- ✅ `ConnectSession.balanceStream(accountID:)` and `balancePublisher(accountID:)`
  landed in `IrohaSwift/Sources/IrohaSwift/ConnectSession.swift` with AsyncStream +
  Combine support plus accompanying tests (`ConnectSessionBalanceTests.swift`).
- ✅ Workshop closed (2026-05-20); evidence captured in
  `docs/source/sdk/swift/readiness/archive/2026-05/connect/workshop-notes.md`
  and the fixture bundle guarded by `ConnectFixtureLoaderTests`.
- ✅ Status/roadmap updated to reflect completion; telemetry exports rely on the
  NDJSON/manifest path provided by `ConnectSessionDiagnostics` and exercised by
  the workshop fixtures.
