# Apple confidential-wallet qualification

This record covers the frozen Apple source candidate and its normal authenticated
local-integration artifact. It establishes host macOS behavior and successful
construction of five Apple target slices. It does not establish physical-device
execution, a signed release, ledger admission or current full-Core/X509 behavior.

The original five-slice build completed after an earlier storage-exhaustion
failure. Its published framework, manifest and source receipt remain preserved.
After that process exited, exactly four reviewed Swift files were refreshed:
the wallet owner, two wallet test files and the public redemption example. These
add retained-change conversion, cleanup when an injected driver throws before
native job consumption, a real change-to-redemption control and partial-entropy
failure cleanup. The latter does not guarantee erasure of Swift-managed copies.

The normal warm builder then passed in 665.558 seconds with no native or Swift
source drift. It built arm64 macOS, arm64 iOS, arm64 iOS simulator, x86_64 iOS
simulator and x86_64 macOS, linked each native slice into its C consumer, passed
the strict artifact/source/export checks and atomically published the framework.
All three packaged archive hashes are identical to the preserved original:

| Slice | SHA-256 |
| --- | --- |
| `ios-arm64` | `d47ed179be8d8ccf05656e2665bcb7328207272e1db82f1460a6b354f63173f7` |
| `ios-arm64_x86_64-simulator` | `98a367af5dcd3f22118e70dda5f2765e54c80582967a6220e4c0188c493cf2ce` |
| `macos-arm64_x86_64` | `e17722bb1d11a3b12b928580e23876c26461628575cd43222cca37a510ecb9c5` |

The refreshed manifest SHA-256 is
`9c0c44028953c85a0422ce0cec35f7c759220b9fd9fcedeb4103b79d64e9bd68`;
its authenticated source seal is
`e3f844c177a6f96f8ec30dc76f50354b2e1d66efd6ec47d9b8bb7739357ed5f7`.

Using the normal `MOBILE_SDK_APPLE_ARTIFACT_DIR` selection, with no loader or
provenance bypass:

- `swift test --filter 'ConfidentialProver(Native)?Tests'` passed all eight
  controls with no failures or skips: six injected-driver lifecycle/boundary
  tests and two native tests producing three real proofs. XCTest execution took
  31.279 seconds; the complete initial package build/test command took 204.046
  seconds. The separate mobile-transports bundle selected zero tests; it is not
  counted as coverage.
- The native tests cover full redemption, conservation rejection, explicit
  closure, and a nondefault input owner followed by partial redemption and
  full redemption of the retained change using its native default owner.
- `swift run confidential-redemption-example` passed in 20.843 seconds and
  reported a locally verified 13,741-byte full-redemption proof. The loader
  reported a valid authenticated bridge in the linked main executable.

The host was arm64 macOS 27.0 with Apple Swift 6.4. The native builder used Rust
1.93.1. Current proof/consensus admission limits were not changed for these runs.
Applications must independently authenticate network, asset, root and change
index; the example submits no transaction.

Source hashes, commands, logs, per-step results, toolchain information, retained
artifacts and the completed receipt are under the ignored evidence directory
`dist/zk-remediation/2026-09-28/apple-wallet-refresh/`. The final receipt is
`swift-native/complete-receipt.json`; the four-file patch and pre/postimages are
retained alongside it. The already preserved archives back the refreshed
manifest by the identical hashes above.
