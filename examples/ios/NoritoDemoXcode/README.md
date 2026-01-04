# NoritoDemoXcode

`NoritoDemoXcode` is a reference SwiftUI wallet application that demonstrates how to
connect to an Iroha 2 Torii node, derive keys, and submit token transfers using the
`NoritoBridge` Swift bindings. The repository now includes the full Xcode project and
SwiftUI sources so contributors can open the demo without regenerating scaffolding.

## Project layout

```
NoritoDemoXcode/
├── Configs/
│   ├── SampleAccounts.json        # mock ledger bootstrap data
│   └── demo.env.example          # scheme environment template
├── NoritoDemoXcode/              # SwiftUI sources
│   ├── App.swift
│   ├── ContentView.swift
│   ├── Info.plist
│   └── NoritoBridgeKit.swift
├── NoritoDemoXcode.xcodeproj/    # pre-generated Xcode project
├── README.md
└── Scripts/
    └── generate-keys.sh          # convenience wrapper around the Rust key derivation flow
```

- `Configs/SampleAccounts.json` — example user accounts seeded in the mock ledger.
- `Configs/demo.env.example` — template for the scheme environment variables (Torii URL,
  Connect SID/token pairs, acceleration toggles).
- `NoritoDemoXcode/` — SwiftUI demo sources that showcase Connect WebSocket flows, Norito
  encryption helpers, and Metal/NEON acceleration hooks.
- `Scripts/generate-keys.sh` — helper for deriving new account keypairs using the Rust
  toolchain.

## Local build instructions

1. Open the repository on a macOS host.
2. Generate the XCFramework following the steps in
   [`docs/norito_bridge_release.md`](../../../docs/norito_bridge_release.md) and copy the
   resulting `NoritoBridge.xcframework` into the demo directory (the project expects the
   framework to sit next to the `.xcodeproj`).
3. Copy `Configs/demo.env.example` to `.env` (or fill the same values directly inside the
   Xcode scheme). The app reads these variables on launch:
   - `TORII_NODE_URL` — base REST URL used for `/v1/connect` and `/v1/pipeline`.
   - `CONNECT_SESSION_ID` — 32-byte session identifier (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — tokens returned by `/v1/connect/session`.
   - `CONNECT_CHAIN_ID` — chain identifier announced in control frames (defaults to `testnet`).
   - `CONNECT_ROLE` — default role selected in the UI (`app` or `wallet`).
   - Optional helpers: `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`,
     `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`, `CONNECT_APPROVE_SIGNATURE_B64`.
4. Open `NoritoDemoXcode/NoritoDemoXcode.xcodeproj` in Xcode 15 or newer. The project
   already references the renamed `IrohaSwift` Swift Package (`https://github.com/hyperledger/iroha-swift`);
   if Xcode still lists the previous package identifier, remove it under **Package
   Dependencies** and add the new URL before resolving.
5. Add `NoritoBridge.xcframework` to the project (`File → Add Files…`), ensure it is
   embedded for the `NoritoDemoXcode` target, and keep "Copy items if needed" enabled so
   local builds use the freshly produced archive.
6. Select the `NoritoDemoXcode` scheme, assign an iOS simulator/device target, and load
   the environment variables via the scheme editor (`Edit Scheme → Run → Arguments`).
7. Build and run the project. The home screen confirms the Norito bridge status and
   exposes Connect session helpers once the framework is linked.

### Hardware acceleration

`NoritoDemoXcode/App.swift` calls `DemoAccelerationConfig.load()` during launch. The
loader checks the following sources in order:

1. `NORITO_ACCEL_CONFIG_PATH` (see `.env` template) — points at an `iroha_config`
   JSON/TOML file on disk.
2. Bundled resources named `acceleration.{json,toml}` or `client.{json,toml}`.
3. Default workspace settings (`AccelerationSettings()`).

This keeps the SwiftUI demo aligned with the node/operator configuration. Drop a
config file into the target, or point the environment variable at a manifest on disk to
toggle Metal/NEON and Merkle thresholds. Devices without Metal automatically fall back
to the scalar implementation even when acceleration is requested.

### Pipeline submission walkthrough

`TransfersViewModel` calls `IrohaSDK.shared.submitAndWait(envelope:)`, which posts the
signed payload to `/v1/pipeline/transactions` and then polls
`/v1/pipeline/transactions/status` until the transaction reaches a terminal state. The
UI reflects intermediate states (`Queued`, `Approved`, etc.) so QA runs can confirm the
pipeline path end-to-end.

## CI verification

- `scripts/ci/verify_norito_demo.sh` builds and tests the Xcode project on macOS. Override
  the simulator destination via `NORITO_DEMO_DESTINATION="platform=iOS Simulator,name=<sim>"`
  when the default `iPhone 15` device is unavailable. Set `NORITO_DEMO_SCHEME` if you need to
  target a non-default scheme and `NORITO_DEMO_DERIVED_DATA` to control where DerivedData is
  written (default: `artifacts/norito_demo_xcodebuild`). If no iOS simulators are installed,
  the script automatically falls back to the first available destination (for example
  `platform=macOS,arch=arm64,variant=Designed for [iPad,iPhone],name=My Mac`) before running
  the build; otherwise it skips gracefully when tooling or destinations are missing.
- `ci/check_swift_samples.sh` wraps this helper and also builds/tests the XcodeGen template
  in `examples/ios/NoritoDemo` so CI can gate both demos in one step.

## Torii + mock ledger integration

Use the automation in `scripts/ios_demo` (see the repository root README) to spawn a Torii
endpoint and a mock ledger that hosts the sample accounts. The SwiftUI demo connects to
this environment via the node URL defined in `demo.env.example`.

## Sample telemetry and logs

- Torii node logs are exported to `artifacts/torii.log`.
- Ledger state snapshots are emitted to `artifacts/ledger/` after each test run.
- Swift client logs can be tailed via the Xcode console or from the generated
  `DerivedData` path.
