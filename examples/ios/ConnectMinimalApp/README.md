# ConnectMinimalApp

SwiftPM executable harness for exercising Connect session plumbing outside Xcode.

## Prerequisites
- Swift 5.9 toolchain
- iOS/macOS bridge artefact: `NoritoBridge.xcframework` available under `IrohaSwift/dist/` or otherwise discoverable by the SDK.

## Run
```bash
swift run --package-path . ConnectMinimalApp
```
The sample uses placeholder keys/endpoints; replace `baseURL`, `appPublicKey`, and direction keys in `Sources/main.swift` for real targets. After exit, diagnostics/journal files are written under `~/Library/Application Support/IrohaConnect/queues` (or the temp fallback), and a bundle is exported to `connect-minimal-bundle/`.

## What it does
- Creates a `ConnectSession` with flow-control defaults.
- Logs ciphertext/control events from the event stream.
- Exports queue/journal metrics via `ConnectSessionDiagnostics.exportJournalBundle`.
