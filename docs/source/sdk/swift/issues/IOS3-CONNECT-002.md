---
title: IOS3-CONNECT-002 – Connect events publisher
summary: Provide async + Combine wrappers for Connect event subscriptions with retry/backoff defaults aligned with Android/JS.
---

# IOS3-CONNECT-002 – Connect events publisher

This issue delivers the Connect event subscription helpers required by IOS3.
Android/JS already expose similar wrappers; Swift must ship parity plus tests
before the roadmap gate can flip to “In Progress”.

## Scope

- Implement `ConnectSession.events(filter:)` returning both:
  - `AnyPublisher<ConnectEvent, ConnectSessionError>`
  - `AsyncThrowingStream<ConnectEvent>`
- Accept optional filters (methods, account IDs) so wallets limit frame volume.
- Share retry/backoff defaults with Android/JS by plumbing
  `ConnectRetryPolicy` into the publisher pipeline.
- Ensure publishers respect fail-close semantics: when the WebSocket closes with
  a policy violation the publisher must terminate with
  `ConnectSessionError.closed(.policyViolation)`.
- Capture queue/backpressure visibility via `ConnectQueueDiagnostics`.

## Deliverables

1. API additions in `ConnectSession.swift` plus supporting types in
   `ConnectEvents.swift` (new file if needed).
2. Tests in `ConnectSessionTests.swift` +
   `ConnectClientTests.swift::testEventsPublisherHandlesClose`.
3. Update `docs/source/sdk/swift/connect_workshop.md` outcomes + status digest.
4. Cookbook/example snippet under `docs/source/sdk/swift/index.md` demonstrating
   hooking events into SwiftUI/Combine.

## Tests

- Publisher happy path (multiple events consumed, cancellation, completion).
- Failure path when stub WebSocket emits `.policyViolation`.
- Retry/backoff behaviour validated via deterministic fixture (depends on
  `IOS3-CONNECT-004`).

## Dependencies

- Replay fixture pack for heartbeat/salt-rotation/resume.
- Telemetry exporters from `IOS3-CONNECT-003` to emit `connect.resume_attempts`.

## Timeline

| Date | Action |
|------|--------|
| 2026-05-20 | Confirm API names in workshop. |
| 2026-05-29 | Merge publisher + async stream. |
| 2026-05-31 | Update docs/telemetry + status digest. |

## Owners

- **Primary:** Swift Connect maintainers.
- **Review:** JS/Android Connect reps for interop parity, Telemetry TL for
  metrics hook.

## Status

- ✅ `ConnectSession.eventStream(filter:)` and `eventsPublisher(filter:)` ship the
  AsyncStream/Combine facades required for IOS3 (see
  `IrohaSwift/Sources/IrohaSwift/ConnectSession.swift` +
  `ConnectSessionEventStreamTests.swift`).
- ✅ Workshop finished and fixtures now live under
  `docs/source/sdk/swift/readiness/archive/2026-05/connect/`; `ConnectFixtureLoaderTests`
  exercise the bundle so publisher replay/telemetry regressions stay guarded.
- ✅ Roadmap/status updated; no further workshop follow-ups remain for this issue.
