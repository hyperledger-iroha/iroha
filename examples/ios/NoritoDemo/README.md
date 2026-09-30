## NoritoDemo iOS Template (XcodeGen)

This iOS sample and its XCTest target depend on the repository-local `IrohaSwift` package and its authenticated NoritoBridge XCFramework.

Requirements
- Xcode 15+
- iOS 15+ target
- XcodeGen (`brew install xcodegen`)

Generate Xcode project
- `cd examples/ios/NoritoDemo`
- `xcodegen generate`
- `open NoritoDemo.xcodeproj`

Build the maintained Apple artifact described in
[`docs/norito_bridge_release.md`](../../../docs/norito_bridge_release.md), then
set `MOBILE_SDK_APPLE_ARTIFACT_DIR` to its absolute canonical artifact directory
before invoking Xcode. `project.yml` already declares the local SDK dependency
for the app and tests.

Environment bootstrap
- Copy `.env.example` to `.env` (or configure the scheme directly) to pre-fill Connect values.
- Supported keys:
  - `TORII_NODE_URL` — base REST URL (the WebSocket endpoint is derived automatically).
  - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` / `CONNECT_TOKEN_RELAY` — role and relay tokens from the same endpoint.
  - `CONNECT_NETWORK_ID` — exact canonical checksummed `NetworkId` bound into the SID and `Open` constraints.
  - The app generates a fresh app key and non-zero 16-byte nonce, derives the SID from those inputs, and rejects substituted session responses; an arbitrary SID cannot be configured.
  - `CONNECT_ROLE` — default role in the picker (`app` or `wallet`).
  - Optional helpers: `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`,
    `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`, `CONNECT_APPROVE_SIGNATURE_B64`.

Provisioning manifest status (OA12)
- [`fixtures/sdk/pos/manifest_v1.json`](../../../fixtures/sdk/pos/manifest_v1.json) is the shared synthetic Swift/Android unit fixture. XcodeGen includes it as a resource.
- `PosManifestLoader` verifies one canonical signed payload before exposing its typed fields. The envelope contains only its base64 payload and Ed25519 signature; unsigned outer fields and duplicate or unknown payload fields are rejected.
- The manifest card shows its ID/sequence, rotation hint and configured backend roots. A fixture signature does not qualify hardware custody.
- Pair manifest card screenshots with the Android sample’s `pos_security_audit.log` entries when rehearsing rotation drills so both platforms keep aligned OA12 evidence.

Next steps
- Use the included `Sources/NoritoBridgeKit.swift` for ergonomic bridging helpers.
- Follow the [public Swift SDK tutorial](https://docs.iroha.tech/guide/tutorials/swift.html)
  to derive keys, build AAD/nonces, and send/receive Iroha Connect frames.
- When `NoritoBridge.xcframework` is linked, the demo UI exposes encrypted send/receive using the bridge.

Key derivation (demo)
- The SwiftUI demo includes X25519 + HKDF-SHA256 key derivation UI. Generate a local ephemeral key and paste the peer’s base64 public key to derive direction keys.
- Derived keys are used automatically for encryption/decryption; the `AEAD Key` field is a manual diagnostic input.
- Connect key derivation requires NoritoBridge's `BLAKE2b-256("iroha-connect|salt|" || sid)` salt. The demo fails closed if the bridge primitive is unavailable.

Control handshake + auto keying
- App role auto-sends the one-shot, sequence-1 `Open` control frame bound to the exact `NetworkId`, app public key, and launch nonce after WS connects.
- Wallet role derives keys on `Open` and can sign/send `Approve` with account_id and signature via the UI.
- App role derives keys on `Approve`. The derived keys populate the demo’s AEAD fields automatically.
- The Connect handshake is disabled when the required bridge controls are unavailable.

Approve UI (wallet)
- Fields: Account ID, wallet Ed25519 private key (base64), signature output (base64).
- "Sign Approve" must use the SDK/native approval preimage, which binds the exact `NetworkId`, constraints, SID, app and wallet keys, canonical I105 account, accepted permissions/proof, and relay token.
- "Send Approve" transmits the control with account_id and signature.

Handshake banner
- Displays state across WS connect and control exchange: Open sent/received, Approve sent/received, Keys ready.
Session setup
- Tap "Create Session" to derive `sid` from the exact `NetworkId`, app key, and fresh 16-byte nonce, then POST that full identity to `/v1/connect/session` to obtain role and relay tokens. Substituted response identity fields are rejected.
- Tap "Join WS" to connect with `sid`, `role`, and the appropriate `token` in the query string.

## CI smoke coverage

- `ci/check_swift_samples.sh` drives a smoke build/test of this template (via
  `xcodegen`) and the SwiftUI demo in `examples/ios/NoritoDemoXcode`. Override simulator
  choice with `SWIFT_SAMPLES_DESTINATION` when running locally; CI defaults to an
  `iPhone 15` simulator and falls back to any available destination automatically.
