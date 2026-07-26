---
lang: uz
direction: ltr
source: docs/source/sdk/swift/connect_dev_quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4dda5c37ff0ed04715761c638defdc7f4984f0cca13f3310eed5a454ebbfb19d
source_last_modified: "2025-12-29T18:16:36.066734+00:00"
translation_last_reviewed: 2026-02-07
---

<!-- Keep this guide short and task-focused for iOS/macOS SDK developers. -->

# IrohaSwift developer quickstart (Connect + offline)

This guide shows how to wire the Swift SDK into an iOS/macOS app, enable the Norito bridge, and cover offline queue/journal flows.

## Prerequisites
- Platforms: iOS 15+ / macOS 12+ (Swift 5.9 toolchain).
- Norito bridge: bundle `NoritoBridge.xcframework` alongside the app (SPM binary target or CocoaPods vendored framework). SwiftPM enables the bridge automatically when present and falls back to Swift-only mode with a warning when absent; production should ship the signed bundle.

## Install the SDK
**Swift Package Manager**
```swift
// Package.swift (application)
.package(url: "https://github.com/hyperledger/iroha-swift.git", branch: "main"),
// target dependencies:
.product(name: "IrohaSwift", package: "iroha-swift")
```
Ensure the `NoritoBridge` binary target is present under `dist/` or provided by your workspace; the package enables the bridge automatically when found and surfaces the expected bridge path in build logs when the bundle is missing.

**CocoaPods (Podspec consumer)**
```ruby
pod 'IrohaSwift', :path => '../IrohaSwift' # or your internal mirror
```
Bundle `NoritoBridge.xcframework` under the repository `dist/` directory or add it to your app’s `Frameworks` folder; the pod guard rails fall back to stubs if the bridge is missing and the same bridge-path hint will surface in runtime errors.

## Sample projects
- `examples/ios/ConnectMinimalApp/` — SwiftPM executable harness that opens a Connect session, logs events, and exports diagnostics/bundles. Use it to validate bridge bundling and queue exports locally.
- `tools/connect-cli/` — SwiftPM CLI utility with `capture`, `replay`, and `inspect` subcommands for Connect queue files. Useful for offline replay or debugging evidence bundles.

## Connect session lifecycle (happy path)
```swift
import IrohaSwift

// 1. Prepare identifiers and keys.
let sid = ConnectSid.generate(chainId: "sora", appPublicKey: appPk, nonce16: nonce)
let connectKeys = ConnectCrypto.deriveDirectionKeys(sharedSecret: sharedSecret, sid: sid)

// 2. Create the session with flow control + diagnostics.
let diagnosticsRoot = ConnectSessionDiagnostics.defaultRootDirectory()
let session = ConnectSession(
    baseURL: URL(string: "https://torii.example")!,
    sessionID: sid.rawBytes,
    flowControl: ConnectFlowControlWindow(defaultTokens: 8),
    diagnosticsRoot: diagnosticsRoot
)
session.setDirectionKeys(connectKeys)

// 3. Drive the WS loop and handle frames.
for try await event in session.eventStream() {
    switch event {
    case .ciphertext(let frame):
        // Decrypt / handle user payloads here.
        print("ciphertext seq=\(frame.sequence)")
    case .control(let control):
        print("control: \(control)")
    }
}
```
Tips:
- Call `ConnectSession.resumeSummary()` after reconnecting to emit sequence/queue depth telemetry.
- Use `ConnectSession.eventStream(filter:)` or `eventsPublisher` (Combine) for UI integration.
- Check `NoritoNativeBridge.shared.isAvailable` before trying bridge-backed features; fall back to error surfaces when unavailable.

## Offline queues and journals
- **Connect queue persistence**: `ConnectQueueJournal` writes per-direction journals under Application Support. Configure bounds to avoid unbounded files:
```swift
let journal = ConnectQueueJournal(
    sessionID: sid.rawBytes,
    configuration: .init(maxRecordsPerQueue: 64, maxBytesPerQueue: 1 << 20)
)
try journal.append(direction: .appToWallet, sequence: 1, ciphertext: payload)
let records = try journal.records(direction: .appToWallet)
```
Oversize/truncated files raise `ConnectQueueError` instead of loading into memory.

- **Offline wallet journal**: `OfflineJournal` stores pending/committed envelopes with hash chain + HMAC. Set caps with `OfflineJournalConfiguration(maxRecords:maxBytes:)` to prevent runaway files.

- **Pipeline retry queue**: `FilePendingTransactionQueue` uses newline-delimited base64 JSON. Configure bounds with `FilePendingTransactionQueueConfiguration` and handle `overflow*` errors.

- **Evidence export**: `ConnectReplayRecorder` + `ConnectSessionDiagnostics.exportJournalBundle` write `manifest.json`, queue files, and `metrics.ndjson` so operators can inspect/replay sessions.

## Troubleshooting
- `ConnectQueueError.overflow` / `ConnectQueueError.corrupted`: journal exceeded caps or contains truncated frames; clear the queue after exporting evidence.
- `FilePendingTransactionQueueError.overflow*`: pending queue too large—flush to Torii or raise limits intentionally.
- Bridge missing: ensure `NoritoBridge.xcframework` is embedded and codesigned.
