//! Swift Connect workshop agenda and facilitation guide.

# Swift Connect Workshop (IOS3)

This workshop aligns the IOS3 Torii/Nexus coverage milestone with the roadmap
next actions (finalise Connect workshop, scope Combine facades, identify
WebSocket coverage gaps). Facilitators should use this agenda to drive the
session, capture decisions, and leave the room with owned follow-ups. ➜ The
2026-05-20 session is complete; decisions + fixtures live in
`docs/source/sdk/swift/readiness/archive/2026-05/connect/workshop-notes.md`.

## Goals
- Validate the Connect handshake + replay story against the current mock Torii harness.
- Decide the scope for Combine/async facades (`ConnectSession`, queue APIs, telemetry exporters).
- Inventory WebSocket test coverage gaps and agree on fixtures needed for replay/determinism.
- Produce owner-assigned actions that unblock the IOS3 risks tracked in
  `docs/source/sdk/swift/connect_risk_tracker.md`.

## Logistics & schedule
- **When:** 2026-05-20, 15:00–16:15 UTC (75 min slot reserved on the Connect council bridge).
- **Where:** `meet.hyperledger.org/ios3-connect-workshop` (records automatically upload to the
  `artifacts/swift/connect_workshop/2026-05-20/` folder alongside the replay fixtures).
- **Facilitator:** Swift SDK Lead (backup: SDK Program PM).
- **Attendees:** Swift Connect maintainers, JS/Android Connect reps, Telemetry TL, Torii liaison.
- **Pre-work owner:** SDK Program PM confirms fixtures + NoritoBridge artefacts copied the day before;
  Telemetry TL validates OTLP collector credentials.
- **Notes capture:** shared doc seeded with the decision + gap tables below; final copy checked into
  `docs/source/sdk/swift/readiness/archive/2026-05/connect/workshop-notes.md`.

Document any schedule adjustments in this section so roadmap/status references stay evergreen.

## Pre-reads & prerequisites
- `docs/source/sdk/swift/connect_risk_tracker.md` (risk context and week-over-week deltas).
- `docs/source/status/swift_weekly_digest.md` (open issues and alerts for Connect telemetry).
- `docs/source/sdk/swift/readiness/archive/2026-03/connect_fault.md` (prior fault-analysis baseline).
- `docs/source/sdk/swift/connect_replay_fixtures.md` (capture/replay recipe used to collect workshop fixtures).
- `IrohaSwift/Tests/IrohaSwiftTests/` Connect suites:
  `ConnectClientTests.swift`, `ConnectSessionTests.swift`,
  `ConnectQueueDiagnosticsTests.swift`, `ConnectRetryPolicyTests.swift`
  (skim assertions to anchor the demo and gap review).
- Environment: Torii staging endpoint + credentials, `NoritoBridge.xcframework`
  present in `./IrohaSwift/.build`, and the mock harness fixtures from Android/JS
  synced locally for replay.
- Pre-session checks (facilitator runs these 1–2 hours before start):
  - `swift test --package-path IrohaSwift --filter ConnectFramesTests` to ensure the bridge/build artefacts are in place.
  - Verify mock Torii harness fixtures are copied to `artifacts/swift/connect_workshop/<date>/fixtures/`.
  - Keep a copy of the reference bundle at `docs/source/sdk/swift/readiness/archive/2026-05/connect/` handy for dry-runs; replace `session.json` with live Torii values when running the workshop.
  - Stage the decision and gap tracker tables below in a shared doc so owners can edit live.

## Agenda (60–75 minutes)
1. **Context + objectives (5–10 min):** recap IOS3 goals, success criteria, and the
   risks blocking GA (CR-2/CR-3 from the risk tracker).
2. **Protocol & handshake review (15 min):** walk through `ConnectClient`/`ConnectSession`
   flows, Norito frame layout, encryption handshake, and retry/backoff semantics.
   Use current tests to illustrate expected behaviour (`swift test --filter ConnectFramesTests`).
3. **Combine/async facade scoping (15–20 min):** decide which surfaces ship in IOS3
   (balance/events/dataspace telemetry streams), error propagation strategy, and
   how to expose queue/flow-control state for UI consumption.
4. **WebSocket coverage gaps (15–20 min):** review existing coverage (frame
   decoding, retry, queue/journal tests) and list missing cases (heartbeat loss,
   replay after salt rotation, concurrent observers). Capture fixtures needed for
   deterministic replays.
5. **Action capture & owners (10 min):** assign issues/PRs with due dates; note
   updates required in `roadmap.md` and `status.md`.

## Combine/async facade scope (seed proposal)
Use this scaffold to leave the workshop with concrete API decisions and code
owners. The proposal threads through IOS3 goals (type-safe Torii parity, queue
visibility, determinism) and references the current Connect surfaces so follow-up
PRs are scoped and testable.

- **Balance stream facade:** add `ConnectSession.balancePublisher(accountID:)`
  returning `AnyPublisher<BalanceSnapshot, ConnectSessionError>` powered by the
  existing `/v1/connect/ws` envelopes. The publisher should expose metadata
  (`ConnectQueueDiagnostics.latestLatency`) via Combine `map` so UI consumers can
  render queue state. Tests live in
  `IrohaSwift/Tests/IrohaSwiftTests/ConnectSessionBalanceTests.swift` (new file)
  and reuse the mock Torii fixtures described below.
- **Event subscription facade:** expose `ConnectSession.events(filter:) ->
  AsyncStream<ConnectEvent>` *and* a mirrored Combine publisher. The stream
  yields strongly-typed Norito payloads decoded via `ConnectCodec`. The async
  variant surfaces concurrency ergonomics, while the Combine publisher enables
  parity with Android/JS reactive pipelines. Tests extend
  `ConnectSessionTests.swift` (`testEventStreamPublishesDecodedFrames`).
- **Dataspace telemetry:** introduce `ConnectSession.dataspaceTelemetryPublisher(
  queue: ConnectRetryQueueDiagnostics)` so apps can pipe queue depth/latency into
  charts. Exported values map to the `connect.queue_*` metrics, keeping the
  observability contract consistent across SDKs.
- **Error propagation:** adopt a shared `ConnectSessionError.ExportContext`
  payload that surfaces retry policy decisions. Combine publishers emit failures
  using the same error enum so consumers do not guess when to reconnect.

### Sample publisher snippet
```swift
let cancellable = session
    .balancePublisher(accountID: "<katakana-i105-account-id>")
    .retry(ConnectRetryPolicy.default.maxRetries)
    .timeout(.seconds(ConnectRetryPolicy.default.heartbeatTimeout))
    .sink(receiveCompletion: { completion in
        if case .failure(let error) = completion {
            logger.error("balance stream failed: \(error)")
        }
    }, receiveValue: { snapshot in
        balanceStore.update(snapshot)
    })
```

Document the final scope/owners in the table below during the workshop so
roadmap/status updates can cite concrete API names.

## Demo & exercises
- Run a focused test slice to smoke the current implementation:
  - `swift test --package-path IrohaSwift --filter ConnectClientTests`
  - `swift test --package-path IrohaSwift --filter ConnectSessionTests`
- Optional replay drill: use the mock Torii harness fixtures to replay a
  `/v1/connect/ws` session and confirm telemetry emission (`connect.queue_*`,
  `connect.resume_attempts_total`) is still dark—evidence for the gap list.
- Capture logs/artifacts in `artifacts/swift/connect_workshop/<date>/` so the
  weekly digest can link back to deterministic runs.
- Push notes + incidents to
  `docs/source/sdk/swift/readiness/archive/2026-05/connect/workshop-notes.md`
  (see the template in that file).

## Decision capture scaffolding

### Combine/async facade scope (record during workshop)

| Surface | Decision to record | Owner | Due |
|---------|--------------------|-------|-----|
| Balance stream facade | Ship `balancePublisher(accountID:)` w/ Combine + AsyncStream parity | Swift Connect maintainers | Workshop +2d |
| Event subscription facade | Ship `events(filter:)` async + Combine wrappers, align retry defaults with Android | Swift Connect maintainers | Workshop +2d |
| Dataspace telemetry stream | Expose queue/backpressure metrics via `dataspaceTelemetryPublisher` | Swift Connect + Telemetry reps | Workshop +5d |
| Queue metrics exposure | Surface `connect.queue_depth`/`connect.queue_latency` via diagnostics struct in public API | Telemetry + SDK program | Workshop +5d |
| Error propagation model | Consolidate error payloads via `ConnectSessionError.ExportContext` shared by async/Combine | Swift Connect maintainers | Workshop |

### WebSocket coverage gap tracker (fill live; start with common gaps)

| Gap | Fixture needed | Owner | Status | Notes |
|-----|----------------|-------|--------|-------|
| Heartbeat loss and reconnect | Replay trace with dropped heartbeats (`docs/source/sdk/swift/readiness/archive/2026-05/connect/heartbeat_loss.json`) → expect resumable session + OTLP emission | Swift Connect maintainers | Planned | Capture hashes + OTLP payload for `ConnectHeartbeatRecoveryTests` |
| Salt rotation + resume | Salt rotation mid-stream fixture (`salt_rotation_session.bin`); verify resume + counter resets | Swift Connect + Security | Planned | Align with CR-2 telemetry requirement + add test to `ConnectSessionTests.swift` |
| Concurrent observers/backpressure | Multi-observer replay using Android fixtures to stress queue depth | SDK Connect working group | Planned | Note queue depth thresholds exercised + ensure `ConnectQueueDiagnosticsTests` asserts limits |
| Telemetry dark counters | Inject fixture where metrics exporter disabled; confirm `connect.queue_*`/`connect.resume_attempts_total` surfaces via new diagnostics publisher | Telemetry + Swift Connect maintainers | Planned | Decide owners for exporter work + add regression in `scripts/tests/test_swift_status_export.py` |

## Outputs & follow-ups
- Agreed list of Combine/async facade commitments for IOS3.
- WebSocket coverage gap tracker populated with owners, due dates, and fixture
  needs; link back to the risk tracker.
- Updates filed for `docs/source/status/swift_weekly_digest.md` and
  `roadmap.md` reflecting decided scope and owners.
- Workshop notes (decisions + evidence bundle pointers) captured in
  `docs/source/sdk/swift/readiness/archive/2026-05/connect/workshop-notes.md`.
- Workshop notes (deck + decisions) stored alongside artefacts; share with Android/JS
  Connect owners for cross-SDK alignment.

### Issue tracking snapshot
| Issue ID | Scope | Owner | Status | Notes |
|----------|-------|-------|--------|-------|
| [IOS3-CONNECT-001](issues/IOS3-CONNECT-001.md) | Implement `balancePublisher(accountID:)` + async stream | Swift Connect maintainers | Drafting | Pull request template staged; targets `ConnectSessionBalanceTests`. |
| [IOS3-CONNECT-002](issues/IOS3-CONNECT-002.md) | Ship `events(filter:)` async/Combine wrappers | Swift Connect maintainers | Drafting | Depends on IOS3-CONNECT-004 fixtures. |
| [IOS3-CONNECT-003](issues/IOS3-CONNECT-003.md) | Dataspace telemetry diagnostics + OTLP exporter | Telemetry TL | Planned | Requires queue metric schema from IOS3-CONNECT-001. |
| [IOS3-CONNECT-004](issues/IOS3-CONNECT-004.md) | WebSocket fixture pack (heartbeat, salt rotation, multi-observer) | SDK Program PM | In progress | Fixtures referenced in the coverage gap table; stored under `docs/source/sdk/swift/readiness/archive/2026-05/connect/`. |
