---
lang: mn
direction: ltr
source: docs/source/sdk/swift/connect_samples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 60e9a1290d21237d955ae1cb583aeba3acaac50b820d3d2241d12ceabe90aece
source_last_modified: "2025-12-29T18:16:36.068036+00:00"
translation_last_reviewed: 2026-02-07
---

<!-- Sample project pointers for Swift developers -->

# Sample projects (Connect + offline)

Use these starter paths to validate your environment and mirror production wiring.

## Minimal SwiftUI sample (Connect)
- Target platforms: iOS 15+ / macOS 12+.
- Create a new SwiftUI app and add the `IrohaSwift` package dependency.
- Embed `NoritoBridge.xcframework` under the app’s `Frameworks` folder (or `dist/` when checking out the SDK).
- In `App` init, generate a `ConnectSid`, derive direction keys, and start `ConnectSession.eventStream()`; log control/ciphertext events and render queue depth from `ConnectSessionDiagnostics.snapshot()`.
- Configure `ConnectQueueJournal.Configuration` bounds and export evidence via a “Share Bundle” button calling `ConnectSessionDiagnostics.exportJournalBundle(to:)`.

## CLI harness (replay & diagnostics)
- Add a SwiftPM executable target in your workspace that depends on `IrohaSwift`.
- Use `ConnectReplayRecorder` to capture frames from fixture files or live WS traffic, then call `exportJournalBundle` to produce `manifest.json`, queue files, and `metrics.ndjson`.
- Include subcommands:
  - `capture --sid <hex> --input <ws_dump>`: append frames to `ConnectQueueJournal`.
  - `replay --sid <hex> --root <diagnostics_dir>`: iterate `records(direction:)` and print sequences/timestamps.
  - `inspect --bundle <dir>`: pretty-print the exported manifest and metrics.
 - A starter CLI is available in `tools/connect-cli` (SPM executable target). It expects `NoritoBridge.xcframework` to be discoverable in the repo checkout and writes bundles under the default diagnostics root.

## Existing examples
- `examples/ios/NoritoDemoXcode/` — Xcode demo showcasing Norito bridge wiring; add Connect session/queue hooks per this guide to validate UI integration.
- `docs/source/sdk/swift/connect_replay_fixtures.md` — describes fixture layout and replay steps; reuse the loader pattern in your sample tests.

## Testing your setup
- Run `swift test --filter ConnectQueueJournalTests` after bundling the bridge and ensuring `CLANG_MODULE_CACHE_PATH` is writable.
- Add sample-specific tests that open a `ConnectSession` with a mock Torii endpoint, append journal entries, and assert bundle export succeeds.

## What to ship
- Keep sample projects small (single scene, minimal dependencies).
- Document bridge placement, required configs, and known failure modes (overflow/corruption) in a local `README.md` so teammates can reproduce.
