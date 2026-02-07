---
lang: zh-hans
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-12-29T18:16:35.099277+00:00"
translation_last_reviewed: 2026-02-07
---

# Norito SwiftUI Demo Contributor Guide

This document captures the manual setup steps required to run the SwiftUI demo against a
local Torii node and mock ledger. It complements `docs/norito_bridge_release.md` by
focusing on day-to-day development tasks. For a deeper walkthrough of integrating the
Norito bridge/Connect stack into Xcode projects, see `docs/connect_swift_integration.md`.

## Environment setup

1. Install the Rust toolchain defined in `rust-toolchain.toml`.
2. Install Swift 5.7+ and Xcode command line tools on macOS.
3. (Optional) Install [SwiftLint](https://github.com/realm/SwiftLint) for linting.
4. Run `cargo build -p irohad` to ensure the node compiles on your host.
5. Copy `examples/ios/NoritoDemoXcode/Configs/demo.env.example` to `.env` and adjust the
   values to match your environment. The app reads these variables on launch:
   - `TORII_NODE_URL` — base REST URL (WebSocket URLs are derived from it).
   - `CONNECT_SESSION_ID` — 32-byte session identifier (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — tokens returned by `/v1/connect/session`.
   - `CONNECT_CHAIN_ID` — chain identifier announced during the control handshake.
   - `CONNECT_ROLE` — default role pre-selected in the UI (`app` or `wallet`).
   - Optional helpers for manual testing: `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`,
     `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`,
     `CONNECT_APPROVE_SIGNATURE_B64`.

## Bootstrapping Torii + mock ledger

The repository ships helper scripts that start a Torii node with an in-memory ledger pre-
loaded with demo accounts:

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

The script emits:

- Torii node logs to `artifacts/torii.log`.
- Ledger metrics (Prometheus format) to `artifacts/metrics.prom`.
- Client access tokens to `artifacts/torii.jwt`.

`start.sh` keeps the demo peer running until you press `Ctrl+C`. It writes a ready-state
snapshot to `artifacts/ios_demo_state.json` (the source of truth for the other artefacts),
copies the active Torii stdout log, polls `/metrics` until a Prometheus scrape is
available, and renders the configured accounts into `torii.jwt` (including private keys
when the config provides them). The script accepts `--artifacts` to override the output
directory, `--telemetry-profile` to match custom Torii configurations, and
`--exit-after-ready` for non-interactive CI jobs.

Each entry in `SampleAccounts.json` supports the following fields:

- `name` (string, optional) — stored as account metadata `alias`.
- `public_key` (multihash string, required) — used as the account signatory.
- `private_key` (optional) — included in `torii.jwt` for client credential generation.
- `domain` (optional) — defaults to the asset domain if omitted.
- `asset_id` (string, required) — asset definition to mint for the account.
- `initial_balance` (string, required) — numeric amount minted into the account.

## Running the SwiftUI demo

1. Build the XCFramework as described in `docs/norito_bridge_release.md` and bundle it
   into the demo project (references expect `NoritoBridge.xcframework` in the project
   root).
2. Open the `NoritoDemoXcode` project in Xcode.
3. Select the `NoritoDemo` scheme and target an iOS simulator or device.
4. Ensure the `.env` file is referenced through the scheme's environment variables.
   Populate the `CONNECT_*` values exported by `/v1/connect/session` so the UI is
   pre-filled when the app launches.
5. Verify hardware acceleration defaults: `App.swift` calls
   `DemoAccelerationConfig.load().apply()` so the demo picks up either the
   `NORITO_ACCEL_CONFIG_PATH` environment override or a bundled
   `acceleration.{json,toml}`/`client.{json,toml}` file. Remove/adjust these inputs if you
   want to force a CPU fallback before running.
6. Build and launch the application. The home screen prompts for Torii URL/token if not
   already set via `.env`.
7. Initiate a "Connect" session to subscribe to account updates or approve requests.
8. Submit an IRH transfer and inspect the on-screen log output along with Torii logs.

### Hardware acceleration toggles (Metal / NEON)

`DemoAccelerationConfig` mirrors the Rust node configuration so developers can exercise
Metal/NEON paths without hard-coding thresholds. The loader searches the following
locations on launch:

1. `NORITO_ACCEL_CONFIG_PATH` (defined in `.env`/scheme arguments) — absolute path or
   `tilde`-expanded pointer to an `iroha_config` JSON/TOML file.
2. Bundled config files named `acceleration.{json,toml}` or `client.{json,toml}`.
3. If neither source is available, the default settings (`AccelerationSettings()`) remain.

Example `acceleration.toml` snippet:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

Leaving the fields `nil` inherits the workspace defaults. Negative numbers are ignored,
and missing `[accel]` sections fall back to deterministic CPU behaviour. When running on
a simulator without Metal support the bridge silently keeps the scalar path even if the
config requests Metal.

## Integration tests

- Integration tests reside in `Tests/NoritoDemoTests` (to be added once macOS CI is
  available).
- Tests spin up Torii using the scripts above and exercise WebSocket subscriptions, token
  balances, and transfer flows via the Swift package.
- Logs from test runs are stored in `artifacts/tests/<timestamp>/` alongside metrics and
  sample ledger dumps.

## CI parity checks

- Run `make swift-ci` before sending a PR that touches the demo or shared fixtures. The
  target executes fixture parity checks, validates the dashboard feeds, and renders the
  summaries locally. In CI the same workflow depends on Buildkite metadata
  (`ci/xcframework-smoke:<lane>:device_tag`) so dashboards can attribute results to the
  correct simulator or StrongBox lane—verify the metadata is present if you adjust the
  pipeline or agent tags.
- When `make swift-ci` fails, follow the steps in `docs/source/swift_parity_triage.md`
  and review the rendered `mobile_ci` output to determine which lane requires
  regeneration or incident follow-up.

## Troubleshooting

- If the demo cannot connect to Torii, verify the node URL and TLS settings.
- Ensure the JWT token (if required) is valid and not expired.
- Check `artifacts/torii.log` for server-side errors.
- For WebSocket issues, inspect the client log window or the Xcode console output.
