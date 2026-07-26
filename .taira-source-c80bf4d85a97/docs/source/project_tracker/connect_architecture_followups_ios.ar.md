---
lang: ar
direction: rtl
source: docs/source/project_tracker/connect_architecture_followups_ios.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a9f6be3b45b8761d414361f35dfe10232aacc366c45c2b429c65dd517c262df
source_last_modified: "2026-01-03T18:08:02.030241+00:00"
translation_last_reviewed: 2026-01-30
---

# Connect Architecture Follow-Up Tickets (Swift / Android / JS)

This tracker records the concrete work items that came out of the Feb 2026
Connect architecture workshop. Each ticket is referenced from
`docs/source/connect_architecture_followups.md`; update both files when owners,
status, or scope change.

| ID | Title | Owners | Target Window | Status |
|----|-------|--------|---------------|--------|
| [IOS-CONNECT-001](#ios-connect-001) | Shared back-off constants | Swift SDK Lead; Android Networking TL; JS Lead | Q2 2026 | Completed |
| [IOS-CONNECT-002](#ios-connect-002) | Ping/pong enforcement | Swift SDK Lead; Android Networking TL; JS Lead | Q2 2026 | Completed |
| [IOS-CONNECT-003](#ios-connect-003) | Offline queue persistence | Swift SDK Lead; Android Data Model TL; JS Lead | Q2 2026 | 🈴 Completed |
| [IOS-CONNECT-004](#ios-connect-004) | StrongBox attestation payload | Android Crypto TL; JS Lead | Q2 2026 | Todo |
| [IOS-CONNECT-005](#ios-connect-005) | Rotation control frame | Swift SDK Lead; Android Networking TL; JS Lead | Q3 2026 | Todo |
| [IOS-CONNECT-006](#ios-connect-006) | Telemetry exporters | Telemetry WG; SDK owners | Q3 2026 | Todo |
| [IOS-CONNECT-007](#ios-connect-007) | Swift CI gating | Swift SDK Lead; Build Infra | Q1 2026 | Todo |
| [IOS-CONNECT-008](#ios-connect-008) | Fallback incident reporting | Swift QA Lead; Build Infra | Q2 2026 | Todo |
| [IOS-CONNECT-009](#ios-connect-009) | Compliance attachments pass-through | Swift SDK Lead; Android Data Model TL; JS Lead | Q2 2026 | Todo |
| [IOS-CONNECT-010](#ios-connect-010) | Error taxonomy alignment | Swift SDK Lead; Android Networking TL; JS Lead | Q2 2026 | Todo |
| [IOS-CONNECT-011](#ios-connect-011) | Workshop decision log | SDK Program Lead | Mar 2026 | Todo |

## IOS-CONNECT-001: Shared back-off constants
<a id="ios-connect-001"></a>

- **Owners:** Swift SDK Lead; Android Networking TL; JS Lead  
- **Target window:** Q2 2026 (pre-IOS3 preview)  
- **Status:** Completed
- **Scope:** Provide a shared crate/module (`connect_retry::policy`) that encodes
  the agreed exponential back-off with jitter (`base=5 s`, `cap=60 s`), exposes
  deterministic sampling, and is consumed by all SDKs.
- **Deliverables:**
  - Rust reference implementation with golden tests for jitter sampling.
  - Swift/Android/JS bindings using the shared constants.
  - Documentation updates in each SDK README explaining retry semantics.
- **Notes:** Delivered via `connect_retry::policy` (Rust) plus `ConnectRetryPolicy` helpers in Swift/Android/JS with deterministic splitmix64 sampling and parity tests across SDKs; status.md updated with the rollout summary.

## IOS-CONNECT-002: Ping/pong enforcement
<a id="ios-connect-002"></a>

- **Owners:** Swift SDK Lead; Android Networking TL; JS Lead  
- **Target window:** Q2 2026  
- **Status:** Completed
- **Scope:** Add configurable heartbeat enforcement to the Connect transports with
  a 30 s default interval, three-miss tolerance, and browser minimum of 15 s.
- **Deliverables:**
  - SDK configuration knobs (`ConnectSessionConfig.pingInterval`, etc.).
  - Metrics emission for `connect.ping_miss_total`.
  - Norito integration tests covering heartbeat disconnect/reconnect.
- **Outcome:** Torii exposes new heartbeat configuration (`ping_interval_ms`, `ping_miss_tolerance`, `ping_min_interval_ms`), enforces the cadence with deterministic disconnects, publishes `connect.ping_miss_total`, and ships regression tests in `iroha_torii`. The JavaScript SDK snapshots surface the new knobs so clients can tune the cadence, and documentation describes the defaults and browser clamp.

## IOS-CONNECT-003: Offline queue persistence
<a id="ios-connect-003"></a>

- **Owners:** Swift SDK Lead; Android Data Model TL; JS Lead  
- **Target window:** Q2 2026  
- **Status:** 🈴 Completed — Swift’s `ConnectQueueJournal` writes hashed session directories plus Norito `.to` records with retention/eviction policies and regression tests, Android mirrors the schema via `ConnectQueueJournal`/`ConnectJournalRecord`, and the JS SDK now ships the IndexedDB adapter + memory fallback required by this milestone.
- **Scope:** Persist Connect queue journals as Norito `.to` blobs across Swift,
  Android, and JS per the shared schema (`sequence`, `direction`, `timestamp_ms`,
  `payload_hash`).
- **Deliverables:**
  - Swift `FileManager`-based writer/reader with unit tests.
  - Android encrypted storage implementation (SQLCipher or EncryptedSharedPrefs).
  - JS IndexedDB adapter with fallback for storage-disabled contexts.
  - Telemetry hook for queue depth and replay success.

- **Latest updates:**
  - `ConnectQueueJournal` encodes deterministic records, ensures SHA-256 session directory names, and enforces retention/byte ceilings (with unit tests covering pop/read/expiry).【IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1】【IrohaSwift/Tests/IrohaSwiftTests/ConnectQueueJournalTests.swift:1】
  - Android’s `ConnectQueueJournal` now mirrors the Norito schema with encrypted storage, and the JS SDK implements the IndexedDB adapter with automated tests plus the documented memory fallback so browsers/private tabs never regress to lossy behaviour.【javascript/iroha_js/src/connectQueueJournal.js:1】【javascript/iroha_js/test/connectQueueJournal.test.js:1】

## IOS-CONNECT-004: StrongBox attestation payload
<a id="ios-connect-004"></a>

- **Owners:** Android Crypto TL; JS Lead  
- **Target window:** Q2 2026  
- **Status:** Todo
- **Scope:** Thread `{platform, evidence_b64, statement_hash}` through wallet
  approvals, expose verification helpers for dApp SDKs, and document failure
  handling.
- **Deliverables:**
  - Android wallet implementation + tests validating StrongBox/TEE output.
  - JS verification code path (browser optional, Node HSM plug-in).
  - Doc updates describing attestation payload consumption.

## IOS-CONNECT-005: Rotation control frame
<a id="ios-connect-005"></a>

- **Owners:** Swift SDK Lead; Android Networking TL; JS Lead  
- **Target window:** Q3 2026  
- **Status:** Todo
- **Scope:** Implement `Control::RotateKeys` / `RotateKeysAck`, expose rotation
  APIs (`rotateKeys`, `cancelRequest(hash)`), and ensure Norito bridge coverage.
- **Deliverables:**
  - Rust bridge updates and golden tests for rotation frames.
  - SDK-level APIs with sample code.
  - Telemetry events for rotation success/failure.

## IOS-CONNECT-006: Telemetry exporters
<a id="ios-connect-006"></a>

- **Owners:** Telemetry WG; SDK owners  
- **Target window:** Q3 2026  
- **Status:** Todo
- **Scope:** Emit the agreed metrics (`connect.queue_depth`,
  `connect.reconnects_total`, `connect.latency_ms`, replay counters) through the
  existing OpenTelemetry exporters.
- **Deliverables:**
  - Metric schemas documented in `docs/source/references/ios_metrics.md` (and
    localized variants).
  - SDK instrumentation with unit/integration test coverage.
  - Dashboard updates referencing the new metrics.

## IOS-CONNECT-007: Swift CI gating
<a id="ios-connect-007"></a>

- **Owners:** Swift SDK Lead; Build Infra  
- **Target window:** Q1 2026  
- **Status:** Todo
- **Scope:** Ensure Connect-related Swift pipelines invoke `make swift-ci` so
  fixture parity, dashboard feeds, and Buildkite metadata stay in sync.
- **Deliverables:**
  - Buildkite pipeline patch calling `make swift-ci` for Connect lanes.
  - CI documentation update noting the gating requirement.
  - Status page entry confirming gating coverage.

## IOS-CONNECT-008: Fallback incident reporting
<a id="ios-connect-008"></a>

- **Owners:** Swift QA Lead; Build Infra  
- **Target window:** Q2 2026  
- **Status:** Todo
- **Scope:** Expose XCFramework smoke harness incidents
  (`xcframework_smoke_fallback`, `xcframework_smoke_strongbox_unavailable`) to
  Connect dashboards for shared visibility.
- **Deliverables:**
  - Telemetry exporter wiring incidents into the dashboard feed.
  - Alert thresholds documented in operations runbooks.
  - Validation logs demonstrating end-to-end incident flow.

## IOS-CONNECT-009: Compliance attachments pass-through
<a id="ios-connect-009"></a>

- **Owners:** Swift SDK Lead; Android Data Model TL; JS Lead  
- **Target window:** Q2 2026  
- **Status:** Todo
- **Scope:** Ensure approval payloads forward optional `attachments[]` and
  `compliance_manifest_id` without loss across SDK boundaries.
- **Deliverables:**
  - Norito decoding/encoding tests covering attachments.
  - SDK updates wiring attachment data to application callbacks.
  - Docs/examples illustrating compliance payload usage.

## IOS-CONNECT-010: Error taxonomy alignment
<a id="ios-connect-010"></a>

- **Owners:** Swift SDK Lead; Android Networking TL; JS Lead  
- **Target window:** Q2 2026  
- **Status:** 🈴 Completed — Swift, Android, and JS SDKs now expose the shared `ConnectError` taxonomy with docs and regression suites.
- **Scope:** Adopt the shared enum (`Transport`, `Codec`, `Authorization`,
  `Timeout`, `QueueOverflow`, `Internal`) across SDK APIs and documentation.
- **Deliverables:**
  - SDK error type implementations with parity tests.
  - Reference documentation & migration notes for app developers.
  - Telemetry mapping aligning error codes with metrics/exporters.
- **Latest updates:**
  - Swift exported `ConnectError`, `ConnectErrorCategory`, and the `ConnectErrorConvertible`
    protocol; all Connect-specific error enums conform and nine regression tests
    cover the mapping (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`).
  - Authored `docs/source/connect_error_taxonomy.md` describing the taxonomy,
    telemetry attributes, and cross-SDK expectations.
  - Android ships `ConnectError`/`ConnectErrors`/`ConnectQueueError` with telemetry attribute helpers and tests (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/connect/ConnectErrorTests.java`); JS mirrors the same taxonomy via `connectError.js`, README docs, and Jest coverage (`javascript/iroha_js/test/connectError.test.js`).

## IOS-CONNECT-011: Workshop decision log
<a id="ios-connect-011"></a>

- **Owners:** SDK Program Lead  
- **Target window:** Mar 2026  
- **Status:** Todo
- **Scope:** Publish the annotated deck/notes summarising the workshop decisions
  to the council archive for traceability.
- **Deliverables:**
  - Markdown/slide export stored alongside council records.
  - Link back to the strawman and feedback docs.
  - Status update in `status.md` acknowledging publication.
