---
lang: he
direction: rtl
source: docs/source/sdk/swift/connect_replay_fixtures.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38f5a6365f99ca3f7a8ffff0f67003772ad8af4cf9c3b86fdc16cae5c0cba77f
source_last_modified: "2026-01-05T18:01:08.592145+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

//! Capture and replay Connect WebSocket fixtures for IOS3 readiness work.

# Connect WebSocket Replay Fixtures (IOS3)

This guide codifies the “codify replay fixtures” roadmap item for IOS3: capture a
Connect preview session, export queue/journal evidence, and replay ciphertext
frames deterministically when debugging or proving coverage gaps.

## Goals & Scope
- Produce portable bundles (`manifest.json`, `metrics.ndjson`, queue files) that
  any SDK team can replay without bespoke scripts.
- Capture the same artefacts expected by `iroha connect queue inspect` so
  governance reviewers can audit storage/telemetry changes.
- Keep fixtures deterministic: stable session IDs, frame sequences, and hashed
  ciphertext payloads recorded via `ConnectQueueJournal`.

## Prerequisites
- Norito bridge built (`NoritoBridge.xcframework` present) so `ConnectCodec` can
  encode/decode frames.
- Torii endpoint + Connect policy snapshot recorded (SID/deeplink generated via
  `bootstrapConnectPreviewSession`).
- Shared storage root for diagnostics: use
  `ConnectSessionDiagnostics.defaultRootDirectory()` (defaults to
  `~/Library/Application Support/IrohaConnect/queues` on Apple platforms or
  `~/.iroha/connect` elsewhere) so CLI/SDK tooling read the same files.
- Destination folder for artefacts: `artifacts/swift/connect_replay/<date>/`
  (creates `manifest.json`, queues, metrics, and notes).

## Capture Workflow
1. **Stage a preview session.** Use the JS helper to mint a deterministic SID and
   WebSocket URL:
   ```js
   const { preview } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   // persist preview.sidBase64Url + preview.webSocketUrl into the artefact folder
   ```
2. **Wire queue instrumentation.** Use `ConnectReplayRecorder` to persist frames
   and metrics to the shared diagnostics root while the session runs:
   ```swift
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let diagnosticsRoot = ConnectSessionDiagnostics.defaultRootDirectory()
   let recorder = ConnectReplayRecorder(sessionID: sid, diagnosticsRoot: diagnosticsRoot)

   func record(frame: ConnectFrame) {
       try? recorder.record(frame: frame)
   }
   ```
   Call `record(frame:)` after every `ConnectClient.receiveFrame()` and before
   sending frames from the app/wallet so sequence numbers line up.
3. **Run the flows.** Exercise open/approve/reject, heartbeat loss, and resume
   paths. When testing queue faults, shrink the limits via
   `ConnectQueueJournal.Configuration(maxRecordsPerQueue: …)` to force drops.
   Journals stream-parse their files with the default 32-record/1 MiB cap per
   direction; oversize or malformed bundles surface `ConnectQueueError` instead
   of loading into memory.
4. **Export the bundle.** After the session finishes, copy files into the
   artefact directory:
   ```swift
   let diagnostics = ConnectSessionDiagnostics(sessionID: sid,
                                               configuration: .init(rootDirectory: diagnosticsRoot))
   let bundleDir = URL(fileURLWithPath: "artifacts/swift/connect_replay/2026-05-09")
   try diagnostics.exportJournalBundle(to: bundleDir)
   try diagnostics.exportQueueMetrics(to: bundleDir.appendingPathComponent("metrics.ndjson"))
   ```
   This writes `manifest.json`, `metrics.ndjson`, and any queue files produced by
   the journal. Add `notes.md` with Torii policy hash, device/OS, and test
   commands.

## Reference bundle
- A synthetic baseline lives at
  `docs/source/sdk/swift/readiness/archive/2026-05/connect/` (mock Torii values,
  deterministic metrics). Use it for workshop dry-runs or as a template when
  exporting live sessions; replace `session.json` endpoints/hashes with real
  values while keeping the manifest/metrics structure unchanged.
- Scenario coverage for the IOS3 workshop ships under
  `.../fixtures/{heartbeat_loss,salt_rotation,multi_observer}.json` with
  matching OTLP-friendly samples in `metrics.ndjson`. `notes.md` describes the
  timeline (heartbeat-loss → resume, salt rotation, backpressure drain) and
  mirrors what was presented live.

### Fixture loader utility
`ConnectFixtureLoaderTests` exercises the bundle in CI and provides a helper you
can reuse when inspecting fixture updates:

```swift
let bundle = try ConnectFixtureLoader().loadBundle()
XCTAssertEqual(bundle.manifest.snapshot.schemaVersion, 1)
XCTAssertEqual(bundle.scenarios.map(\.scenario).sorted(),
               ["heartbeat_loss", "multi_observer", "salt_rotation"])
XCTAssertGreaterThan(bundle.metrics.count, 0)
```

The loader expects the bundle layout above; keep filenames stable when swapping
in live Torii captures so the regression test keeps guarding the evidence.

## Replay & Validation
- **CLI check:** `iroha connect queue inspect --sid <sid> --root <diagnosticsRoot> --manifest <bundleDir>/manifest.json`
  should report the recorded state (direction counts, watermarks, drop reasons).
- **Swift harness replay:** iterate over `records(direction:)` and feed ciphertext
  frames back through your wallet/dapp harness:
  ```swift
  let records = try journal.records(direction: .appToWallet)
  for record in records {
      let frame = ConnectFrame(sessionID: sid,
                               direction: record.direction,
                               sequence: record.sequence,
                               kind: .ciphertext(.init(payload: record.ciphertext)))
      try await client.send(frame: frame)
  }
  ```
- **Evidence archive:** stash `manifest.json`, queue files, metrics, session
  inputs (SID, deeplink), and Grafana/Prometheus exports under
  `docs/source/sdk/swift/readiness/archive/<date>/connect/` and link them from
  `docs/source/sdk/swift/connect_workshop.md` + `status.md`.

## Quality Gates
- Run `swift test --package-path IrohaSwift --filter ConnectQueueDiagnosticsTests`
  after modifying instrumentation.
- Keep the artefact folders small and deterministic (avoid personal data in
  filenames, scrub tokens). Update `roadmap.md` and the weekly digest with the
  bundle path once captured.
