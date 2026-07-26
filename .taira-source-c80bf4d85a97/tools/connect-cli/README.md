# connect-cli

SwiftPM CLI utility for capturing, replaying, and inspecting Connect queue journals.

## Prerequisites
- Swift 5.9 toolchain on macOS 12+
- `NoritoBridge.xcframework` discoverable in the repo checkout (same as the SDK).

## Build & run
```bash
swift run --package-path . connect-cli <command> <sid_base64url> [bundle_dir]
```
Commands:
- `capture <sid_base64url>`: read hex ciphertext lines from stdin, append to `ConnectQueueJournal` (app→wallet direction), store under the default diagnostics root.
- `replay <sid_base64url>`: print per-direction queue contents from the default diagnostics root.
- `inspect <sid_base64url> [bundle_dir]`: export `manifest.json`, queue files, and `metrics.ndjson` to a bundle directory and print a snapshot summary.

Use this to debug evidence bundles or to exercise queue parsing/overflow handling without a UI.
