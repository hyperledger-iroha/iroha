## NoritoDemo iOS Template (XcodeGen)

This is a minimal iOS app template that compiles out-of-the-box and can link the NoritoBridge XCFramework.

Requirements
- Xcode 15+
- iOS 13+ target
- XcodeGen (`brew install xcodegen`)

Generate Xcode project
- `cd examples/ios/NoritoDemo`
- `xcodegen generate`
- `open NoritoDemo.xcodeproj`

By default it shows whether NoritoBridge is linked (`canImport(NoritoBridge)`).

After opening the project, ensure the renamed `IrohaSwift` Swift Package dependency is
added via **File → Add Package Dependencies…** → `https://github.com/hyperledger/iroha-swift`.
Older clones may still reference pre-rename identifiers; remove them before resolving.

Environment bootstrap
- Copy `.env.example` to `.env` (or configure the scheme directly) to pre-fill Connect values.
- Supported keys:
  - `TORII_NODE_URL` — base REST URL (the WebSocket endpoint is derived automatically).
  - `CONNECT_SESSION_ID` — base64/base64url session identifier returned by `/v1/connect/session`.
  - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — role tokens from the same endpoint.
  - `CONNECT_CHAIN_ID` — chain identifier announced during control frames (defaults to `testnet`).
  - `CONNECT_ROLE` — default role in the picker (`app` or `wallet`).
  - Optional helpers: `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`,
    `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`, `CONNECT_APPROVE_SIGNATURE_B64`.

Link NoritoBridge (optional)
1. Copy `NoritoBridge.xcframework` into `examples/ios/NoritoDemo/Frameworks/` (create the folder if needed).
2. Edit `project.yml` and uncomment the dependency under the `NoritoDemo` target:
   - `- framework: Frameworks/NoritoBridge.xcframework`
3. Re-run `xcodegen generate` and re-open the project.

Provisioning manifest status (OA12)
- `Sources/Resources/pos_manifest.json` mirrors the OA12 provisioning fixture from the Android sample.
- `PosManifestLoader` decodes the manifest, and the SwiftUI view renders a manifest card that calls out the manifest ID/sequence, rotation hint, dual-signature state, and backend roots.
- Pair manifest card screenshots with the Android sample’s `pos_security_audit.log` entries when rehearsing rotation drills so both platforms keep aligned OA12 evidence.

Next steps
- Add `docs/NoritoBridgeKit.swift` (or use the included `Sources/NoritoBridgeKit.swift`) to your app target for ergonomic bridging helpers.
- Follow `docs/connect_swift_integration.md` to derive keys, build AAD/nonces, and send/receive Iroha Connect frames.
- When `NoritoBridge.xcframework` is linked, the demo UI exposes encrypted send/receive using the bridge.

Key derivation (demo)
- The SwiftUI demo includes X25519 + HKDF-SHA256 key derivation UI. Generate a local ephemeral key and paste the peer’s base64 public key to derive direction keys.
- Derived keys are used automatically for encryption/decryption; the `AEAD Key` field is a fallback.
- When NoritoBridge is linked and exports `connect_norito_blake2b_256`, the app uses `BLAKE2b-256("iroha-connect|salt|" || sid)` for salt; otherwise it falls back to SHA256.

Control handshake + auto keying
- App role auto-sends an `Open` control frame with its X25519 public key after WS connects (when the bridge provides control helpers).
- Wallet role derives keys on `Open` and can sign/send `Approve` with account_id and signature via the UI.
- App role derives keys on `Approve`. The derived keys populate the demo’s AEAD fields automatically.
- If control helpers are not available in the bridge, you can still derive keys via manual public-key paste.

Approve UI (wallet)
- Fields: Account ID, wallet Ed25519 private key (base64), signature output (base64).
- "Sign Approve" signs `"iroha-connect|approve|" || sid || app_pk || wallet_pk || account_id` using Ed25519.
- "Send Approve" transmits the control with account_id and signature.

Handshake banner
- Displays state across WS connect and control exchange: Open sent/received, Approve sent/received, Keys ready.
Session setup
- Tap "Create Session" to compute `sid` client-side and POST `/v1/connect/session` to obtain per-role tokens.
- Tap "Join WS" to connect with `sid`, `role`, and the appropriate `token` in the query string.

## CI smoke coverage

- `ci/check_swift_samples.sh` drives a smoke build/test of this template (via
  `xcodegen`) and the SwiftUI demo in `examples/ios/NoritoDemoXcode`. Override simulator
  choice with `SWIFT_SAMPLES_DESTINATION` when running locally; CI defaults to an
  `iPhone 15` simulator and falls back to any available destination automatically.
