<!-- Keep this guide short and task-focused for iOS/macOS SDK developers. -->

---
title: IrohaSwift Connect Developer Quickstart
summary: Source-coupled setup notes for Connect transport and offline queue flows.
---

# IrohaSwift developer quickstart (Connect + offline)

This guide shows how to wire the Swift SDK into an iOS/macOS app, enable the Norito bridge, and cover offline queue/journal flows.

## Prerequisites
- Platforms: iOS 15+ / macOS 12+ (Swift 5.9 toolchain).
- Norito bridge: bundle `NoritoBridge.xcframework` alongside the app (SPM binary target or CocoaPods vendored framework). Connect frame and native crypto operations fail closed when the required bridge is absent.

## Install the SDK
**Swift Package Manager**
```swift
// Package.swift (application)
.package(url: "https://github.com/hyperledger/iroha-swift.git", branch: "main"),
// target dependencies:
.product(name: "IrohaSwift", package: "iroha-swift")
```
Ensure the `NoritoBridge` binary target is present under `dist/` or provided by your workspace; the package enables the bridge automatically when found and surfaces the expected bridge path in build/runtime hints when the bundle is missing.

**CocoaPods (Podspec consumer)**
```ruby
pod 'IrohaSwift', :path => '../IrohaSwift' # or your internal mirror
```
Bundle `NoritoBridge.xcframework` under the repository `dist/` directory or add it to your app’s `Frameworks` folder; a missing bridge is an installation error, not a codec fallback.

## Sample projects
- `examples/ios/ConnectMinimalApp/` — SwiftPM executable harness that opens a Connect session, logs events, and exports diagnostics/bundles. Use it to validate bridge bundling and queue exports locally.
- `tools/connect-cli/` — SwiftPM CLI utility with `capture`, `replay`, and `inspect` subcommands for Connect queue files. Useful for offline replay or debugging evidence bundles.

## Connect session lifecycle (happy path)
```swift
import IrohaSwift

// 1. Prepare one exact launch identity and let Torii echo-verify it.
let networkID = try NetworkId(literal: canonicalNetworkID)
let appKeys = try ConnectCrypto.generateKeyPair()
let nonce = try secureRandomBytes(count: 16)
let torii = ToriiClient(baseURL: URL(string: "https://torii.example")!)
let created = try await torii.createConnectSession(
    networkID: networkID,
    appPublicKey: appKeys.publicKey,
    nonce: nonce
)
let request = try ConnectClient.makeWebSocketRequest(
    baseURL: torii.baseURL,
    sid: created.sid,
    role: .app,
    token: created.tokenApp
)
let client = ConnectClient(request: request)
let session = try ConnectSession(
    networkID: networkID,
    appPublicKey: appKeys.publicKey,
    nonce: nonce,
    relayToken: created.tokenRelay,
    client: client,
    flowControl: ConnectFlowControlWindow(appToWallet: 8, walletToApp: 8)
)
client.start()
try await session.sendOpen(open: ConnectOpen(
    appPublicKey: appKeys.publicKey,
    appMetadata: nil,
    constraints: ConnectConstraints(networkID: networkID),
    permissions: ConnectPermissions(methods: ["SIGN_REQUEST_TX"])
))

// 2. Drive the WS loop and handle frames.
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
- `flowControl` is an SDK-local queue limiter only. Connect V1 has no `FlowControl`, `Resume`, or `Rotate` wire controls.
- Use `ConnectSession.eventStream(filter:)` or `eventsPublisher` (Combine) for UI integration.
- Treat bridge-unavailable errors as fatal setup errors for Connect.

## Offline queues and journals
- **Connect queue persistence**: `ConnectQueueJournal` writes per-direction journals under Application Support. Configure bounds to avoid unbounded files:
```swift
let journal = ConnectQueueJournal(
    sessionID: try ConnectCrypto.deriveSessionID(networkID: networkID, appPublicKey: appKeys.publicKey, nonce: nonce),
    configuration: .init(maxRecordsPerQueue: 64, maxBytesPerQueue: 1 << 20)
)
try journal.append(direction: .appToWallet, sequence: 1, ciphertext: payload)
let records = try journal.records(direction: .appToWallet)
```
Oversize/truncated files raise `ConnectQueueError` instead of loading into memory.

- **Offline wallet journal**: `OfflineJournal` stores pending/committed envelopes with hash chain + HMAC. Set caps with `OfflineJournalConfiguration(maxRecords:maxBytes:)` to prevent runaway files.

- **Caller-managed pipeline archive**: `FilePendingTransactionQueue` uses newline-delimited base64 JSON for explicit local storage only. `IrohaSDK` never drains or submits it; configure bounds with `FilePendingTransactionQueueConfiguration` and handle `overflow*` errors.

- **Evidence export**: `ConnectReplayRecorder` + `ConnectSessionDiagnostics.exportJournalBundle` write `manifest.json`, queue files, and `metrics.ndjson` so operators can inspect/replay sessions.

## Troubleshooting
- `ConnectQueueError.overflow` / `ConnectQueueError.corrupted`: journal exceeded caps or contains truncated frames; clear the queue after exporting evidence.
- `FilePendingTransactionQueueError.overflow*`: pending queue too large—flush to Torii or raise limits intentionally.
- Bridge missing: ensure `NoritoBridge.xcframework` is embedded and codesigned.
