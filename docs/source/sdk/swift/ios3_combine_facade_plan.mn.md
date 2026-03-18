---
lang: mn
direction: ltr
source: docs/source/sdk/swift/ios3_combine_facade_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 84346a53b2be725d7b9e14cc7822acc11eb8fceddf91828c0d607f3ae592c1c0
source_last_modified: "2025-12-29T18:16:36.073079+00:00"
translation_last_reviewed: 2026-02-07
title: Swift IOS3 Combine & WebSocket Coverage Plan
summary: Breaks down the Combine/async facade work and WebSocket test coverage required by IOS3 in the roadmap.
---

# Swift IOS3 Combine & WebSocket Coverage Plan

Roadmap milestone **IOS3 — Torii/Nexus API Coverage** calls out a P1 deliverable
for high-level Combine/async facades and explicit WebSocket coverage. This
document captures the surfaces that need wrappers, the sequencing of the work,
and the evidence we have to produce before IOS3 can close. Use it as the working
agreement across Swift, Torii, and Connect maintainers.

## Goals

- Export idiomatic Combine publishers and `AsyncSequence` views for the Torii
  streaming endpoints already implemented in `ToriiClient.swift` (verifying key
  events, proof events, governance snapshots, explorer metrics, etc.).
- Layer high-level Combine/async helpers on top of the Connect WebSocket stack
  (`ConnectClient`, `ConnectSession`, and `ConnectAsyncSequence`) so wallets can
  consume Connect frames without manual task management.
- Define the minimum WebSocket/SSE coverage (unit + integration tests) needed
  before IOS3’s “typed WebSocket support” gate can be marked green.
- Keep telemetry, docs, and CI evidence aligned with roadmap expectations
  (reference: `roadmap.md` §IOS3 Next Actions).

## Facade Breakdown

| Surface | Current entry points | Planned Combine/async facade | Observability/Test hooks |
|---------|---------------------|------------------------------|--------------------------|
| **Verifying key events** | `ToriiClient.streamVerifyingKeyEvents` in `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift` with async sequence helpers tested under `ToriiClientTests.swift:1640-1724`. | Add `ToriiClient.verifyingKeyEventsPublisher(filter:lastEventId:) -> AnyPublisher<ToriiVerifyingKeyEventMessage, ToriiClientError>` and an `AsyncThrowingStream` factory so callers can choose Combine, async/await, or manual iteration. Publishers fan out on a shared `PassthroughSubject` so multiple subscribers reuse the same HTTP stream. | Emit `swift_ios3_stream_events_total` (`status=ok|error`) in the publisher wrapper, recorded via `SwiftStatusMetrics.shared`. Extend `ToriiClientTests.swift` with Combine-specific expectations (publisher completes on cancellation, propagates headers). |
| **Proof/telemetry streams** | `ToriiClient.streamProofEvents`, `streamGovernanceTelemetry`, and explorer metrics readers. | Mirror the publisher/async helpers above plus a `ToriiStreamSubscription` helper that accepts `RetryPolicy`/`Backoff` structs. The helper owns automatic resubscription when `lastEventId` is available. | Update the mock Torii harness used in unit tests to inject reconnect + backoff timings so we can assert retry counts. Record coverage via `swift test --filter ToriiClientStreamCombineTests`. |
| **Connect WebSocket session** | `ConnectClient`, `ConnectSession`, `ConnectAsyncSequenceTests.swift`, and `ConnectClientTests.swift` already wrap URLSession tasks but require manual `receive()` loops. | Provide `Combine` publishers for authenticated frame streams and high-level `AsyncSequence`s that expose `ConnectEnvelope` objects with automatic ack/fail-close behaviour. Introduce `ConnectSessionPublisher` that gates commands behind `CombineLatest` of session state + telemetry toggles. | Telemetry counter `swift_ios3_connect_frames_total{direction=send|recv}` increments from the publisher. Add tests in `ConnectClientTests.swift` verifying publisher completion, error propagation, and fail-close. Update the Norito demo CI recipe to exercise the Combine-based Connect handshake. |
| **Dataspace/event facades** | Balance/account helpers live in `ToriiClient.swift` but return arrays/futures only. | Build thin wrappers that expose `AnyPublisher<ToriiAccountBalance, Never>` backed by `AsyncStream` polling so wallets can bind to UI components. Provide `AsyncThrowingStream` for dataspace telemetry (Lane snapshots). | Add snapshot tests in `IrohaSwift/Tests/IrohaSwiftTests/ToriiDataspaceCombineTests.swift` verifying deduplication and cancellation. Surface `swift_ios3_dataspace_refresh_duration_seconds` histogram via the polling helper. |

## Implementation Phases

### Phase 1 — Streaming Publishers (May 2026)
- Ship Combine + `AsyncStream` helpers for verifying-key and proof streams.
- Add `ToriiStreamSubscription` abstraction with retry/backoff knobs sourced
  from `IrohaConfig.Telemetry`.
- Extend `ToriiClientTests.swift` with Combine-specific tests exercising cancel,
  error, and retry paths using the existing mock HTTP harness.
- Deliverables: helper APIs, unit tests, doc snippet in `docs/source/sdk/swift/index.md`.

### Phase 2 — Connect Publishers (June 2026)
- Layer Combine publishers on top of `ConnectSession` so UI flows no longer have
  to poll `receive()` manually.
- Document the bridging rules (frame ownership, telemetry hooks, fail-close) in
  `docs/source/connect_architecture_strawman.md`.
- Expand `ConnectAsyncSequenceTests.swift` to cover mixed publisher/async usage,
  injecting stub WebSocket tasks (`ConnectTestUtilities.swift`) to simulate
  close codes, timeouts, and partial frames.

### Phase 3 — Dataspace & Telemetry Facades (July 2026)
- Introduce Combine-based adapters for account balances, event feeds, and lane
  telemetry. Each adapter wraps the existing REST endpoints, includes caching,
  and emits Norito audit records on failure.
- Wire metrics into `scripts/swift_status_export.py` so weekly digests can
  highlight Combine adoption progress.
- Produce cookbook examples (Swift Playgrounds + README snippets) showing how to
  bind the publishers to SwiftUI views.

## WebSocket & SSE Test Coverage Matrix

| Scenario | Test location | Harness requirements | Notes |
|----------|---------------|----------------------|-------|
| Normal streaming over HTTP (SSE) | `ToriiClientStreamCombineTests.swift` | Reuse mock Torii server harness; inject headers + chunked bodies. | Validates Combine publisher emits the same ordering as the async iterator. |
| Network blip with retry/backoff | `ToriiClientStreamCombineTests.swift::testPublisherRetriesWithLastEventId` | Harness sends two events, drops connection, expects wrapper to resume with `Last-Event-Id`. | Must assert retry delay respects config and telemetry increments `retry_total`. |
| Connect WebSocket handshake success | `ConnectClientTests.swift::testPublisherCompletesHandshake` | Stub WebSocket task returns `connected` state, handshake frame, ack, close. | Ensures publisher completes gracefully and surfaces telemetry counters. |
| Connect fail-close path | `ConnectClientTests.swift::testPublisherPropagatesCloseCode` | Stub WebSocket set to `.policyViolation`, ensures publisher terminates with error. | Needed for roadmap Connect risk mitigation. |
| Dataspace polling cancellation | `ToriiDataspaceCombineTests.swift::testCancellingStopsPolling` | Mock REST endpoint increments request counter. | Verifies Combine wrapper stops hitting Torii once the subscriber cancels. |

## Evidence & Tracking

1. **APIs & Docs:** Update `docs/source/sdk/swift/index.md` and the Connect
   strawman with the new facade descriptions once each phase lands.
2. **CI:** Add `swift test --filter ToriiClientStreamCombineTests` and
   `--filter ToriiDataspaceCombineTests` to the `ci/swift_status_export.sh`
   smoke job so regressions block IOS3.
3. **Telemetry:** Capture the publisher metrics via
   `swift_ios3_stream_events_total`, `swift_ios3_connect_frames_total`, and
   `swift_ios3_dataspace_refresh_duration_seconds` so observability can chart
   adoption. The exporter wiring will live in `IrohaSwift/Sources/IrohaSwift/Telemetry.swift`.
4. **Status Tracking:** Each phase completion must be logged in
   `status.md` (IOS3 section) with links to the relevant commits/tests and the
   metrics snapshot that proves the wrappers ran in CI.
