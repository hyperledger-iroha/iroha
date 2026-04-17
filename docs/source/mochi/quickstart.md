# MOCHI Quickstart

**MOCHI** is a local Hyperledger Iroha devnet app. This guide walks through installing the
prerequisites, building the application, launching the egui frontend, and using the devnet
quickstart, maintenance controls, and live activity surfaces for day-to-day development.

## Prerequisites

- Rust toolchain: `rustup default stable` (workspace targets edition 2024 / Rust 1.82+).
- Platform toolchain:
  - macOS: Xcode Command Line Tools (`xcode-select --install`).
  - Linux: GCC, pkg-config, OpenSSL headers (`sudo apt install build-essential pkg-config libssl-dev`).
- Iroha workspace dependencies:
  - `cargo xtask mochi-bundle` requires built `irohad`, `kagami`, and `iroha_cli`. Build them once via
    `cargo build -p irohad -p kagami -p iroha_cli`.
- Optional: `direnv` or `cargo binstall` for managing local cargo binaries.

MOCHI shells out to the CLI binaries. Ensure they are discoverable via the environment variables
below or available on the PATH:

| Binary   | Environment override | Notes                                   |
|----------|----------------------|-----------------------------------------|
| `irohad` | `MOCHI_IROHAD`       | Supervises peers                        |
| `kagami` | `MOCHI_KAGAMI`       | Generates genesis manifests/snapshots   |
| `iroha_cli` | `MOCHI_IROHA_CLI` | Optional for upcoming helper features   |

## Building MOCHI

From the repository root:

```bash
cargo build -p mochi-ui
```

This command builds both `mochi-core` and the egui frontend. To produce a distributable bundle, run:

```bash
cargo xtask mochi-bundle
```

The bundle task assembles the binaries, manifest, and config stubs under `target/mochi-bundle`.

## Launching the devnet app

Run the UI directly from cargo:

```bash
cargo run -p mochi-ui
```

By default MOCHI opens on the **Network** page with a single-peer preset rooted at the current
workspace:

- Workspace root: the current working directory.
- Sandbox root: `<workspace>/.mochi/sandbox/single-peer`.
- Torii base port: `8080`.
- P2P base port: `1337`.

Use CLI flags to override the defaults when launching:

```bash
cargo run -p mochi-ui -- \
  --workspace-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

Environment variables mirror the same overrides when CLI flags are omitted: set
`MOCHI_WORKSPACE_ROOT`, `MOCHI_DATA_ROOT`, `MOCHI_PROFILE`, `MOCHI_CHAIN_ID`,
`MOCHI_TORII_START`, `MOCHI_P2P_START`, `MOCHI_RESTART_MODE`, `MOCHI_RESTART_MAX`, or
`MOCHI_RESTART_BACKOFF_MS` to preseed the supervisor builder; binary paths continue to respect
`MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI`, and `MOCHI_CONFIG` points at an explicit
`config/local.toml`.

When only a workspace root is provided, Mochi writes app bootstrap files there and keeps runtime
state under `<workspace>/.mochi/sandbox/<profile>`. Use `--data-root` only when you intentionally
want runtime logs/storage somewhere else while still treating the workspace as the home for
`.env.local` and `.mochi/generated/*`.

## Headless local sandbox

For Codex or shell automation, use the helper script instead of clicking through the GUI:

```bash
scripts/mochi_local_sandbox.sh up
scripts/mochi_local_sandbox.sh status
scripts/mochi_local_sandbox.sh env
scripts/mochi_local_sandbox.sh mcp-add-command
```

`up` launches `cargo run -p mochi-ui -- sandbox serve` in a detached process group, waits for
`/status` readiness, runs a local smoke transaction, validates local MCP, writes
`<workspace>/.mochi/sandbox/<profile>/session.json`, and refreshes `.env.local` plus
`.mochi/generated/*`. The helper records the actual long-lived Mochi PID in `serve.pid`, so
`scripts/mochi_local_sandbox.sh status` should remain `ready` after `up` returns and
`scripts/mochi_local_sandbox.sh down` can stop the sandbox cleanly with SIGTERM. By default the
helper uses `<workspace>/.mochi/build-target` as its `CARGO_TARGET_DIR`, which keeps Mochi startup
isolated from other builds happening in the repo; set `MOCHI_CARGO_TARGET_DIR` if you want a
different cache location. Set `MOCHI_PROFILE=four-peer-bft` for the four-validator rehearsal or
`MOCHI_WORKSPACE_ROOT=/path/to/app` when you are starting the sandbox for another workspace. The
repo-shared Codex guidance for this flow lives at
`skills/mochi-local-sandbox/`; install or symlink it into
`$CODEX_HOME/skills/mochi-local-sandbox` when you want Codex to use it.

Local validator configs now pin `nexus.enabled = false`, `confidential.enabled = true`, and the
same `sumeragi.consensus_mode` that Mochi asked Kagami to use for genesis. Enabling Nexus requires
an NPoS profile; Mochi rejects `nexus.enabled = true` on permissioned presets before peers launch.
The rendered local Torii config also enables `[torii.mcp]` with the curated writer profile and
`[torii.transport.norito_rpc]` with `enabled = true`, `require_mtls = false`, and `stage = "ga"`.

After launch, use the **Devnet quickstart** card on the Network page for the normal local-dev flow:

- Pick `Single Peer` or `Four Peer BFT`.
- Adjust the workspace, chain ID, and base ports.
- Use **Start devnet**, **Restart devnet with this setup**, **Apply without starting**, or
  **Stop devnet**.
- Use the **Connect your app** card to copy Torii/API endpoints and the bundled development
  identities.
- Open **Advanced settings** only when the quickstart fields are not enough.

## Settings & persistence

Open **Advanced settings** from the Network page or the top control bar when you need to adjust the
full supervisor configuration:

- **Workspace root** — home for `.env.local`, `.mochi/generated/*`, and the default
  `.mochi/sandbox/<profile>` runtime directory.
- **Data root override** — optional direct runtime path for peer configs, storage, logs, and
  snapshots when you do not want Mochi to use the default workspace-relative sandbox root.
- **Torii / P2P base ports** — starting ports for deterministic allocation.
- **Profile override / inline TOML** — advanced topology and config tuning.
- **Nexus / DA** — lane, dataspace, and DA-specific configuration.
- **Tooling / readiness** — auto-build and readiness-smoke behaviour.
- **Logs / exports** — output and export directory controls.

Advanced knobs such as the supervisor restart policy live in
`config/local.toml`. Set `[supervisor.restart] mode = "never"` to disable
automatic restarts during incident debugging, or adjust
`max_restarts`/`backoff_ms` (via either the config file or the CLI flags
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) to control retry
behaviour.

Applying changes rebuilds the supervisor, restarts any running peers, and writes the overrides to
`config/local.toml`. The configuration merge preserves unrelated keys so advanced users can keep
manual tweaks alongside MOCHI-managed values.

## Snapshots & wipe/re-genesis

The **Maintenance** controls expose the reset flows you use when iterating on a local network:

- **Export snapshot** — copies peer storage/config/logs and the current genesis manifest into
  `snapshots/<label>` under the active data root. Labels are sanitized automatically.
- **Restore snapshot** — rehydrates peer storage, snapshot roots, configs, logs, and the genesis
  manifest from an existing bundle. `Supervisor::restore_snapshot` accepts either an absolute path or
  the sanitised `snapshots/<label>` folder name; the UI mirrors this flow so Maintenance → Restore
  can replay evidence bundles without touching files manually.
- **Reset lane** — clears a configured Nexus lane when lane management is enabled for the current
  profile.
- **Wipe & re-genesis** — stops running peers, removes storage directories, regenerates genesis via
  Kagami, and restarts peers when the wipe completes.

Both flows are covered by regression tests (`export_snapshot_captures_storage_and_metadata`,
`wipe_and_regenerate_resets_storage_and_genesis`) to guarantee deterministic outputs.

## Activity, state, and transactions

The **Activity** view exposes the live debugging surfaces:

- **Logs** — tails `irohad` stdout/stderr/system lifecycle messages for the selected peer.
- **Events** — shows managed event streams with filter and export controls.
- **Blocks** — shows managed block streams with decoded summaries.
- The activity toggles can auto-attach these surfaces to a running peer so you can start a devnet
  and inspect it immediately instead of wiring streams by hand.

The **Network** view continues to show the peer dashboard and readiness/health telemetry:

- **Status** — polls `/status` and renders sparklines for queue depth, throughput, and latency.
- **Startup readiness** — after pressing **Start devnet**, MOCHI probes `/status` with bounded
  backoff; the banner reports when each peer goes ready (with the observed queue depth) or surfaces
  the Torii error if readiness times out.

The **State** and **Transactions** views keep the common dev workflow in one app:

- **State** provides quick access to accounts, assets, peers, domains, and asset definitions without
  leaving the UI. The Peers query mirrors `FindPeers`, which helps confirm the current validator set
  before running integration tests.
- **Transactions** prioritizes mint/burn/transfer and registration flows, while still exposing raw
  Norito editing, multisig proposals, manifests, admission policy updates, and role changes.
- Successful transaction submissions now jump into **Activity → Events** with the transaction hash
  prefilled as a filter; failed submissions jump into **Activity → Logs** for the selected peer.

Use the composer toolbar's **Manage signing vault** button to import or edit signing authorities. The
dialog writes entries to the active sandbox root (`<sandbox_root>/signers.json`), and saved vault
keys are immediately available for transaction previews and submissions. When the vault is empty the
composer falls back to the bundled development keys so local workflows continue to work.
Forms now cover mint/burn/transfer (including implicit receive), domain/account/asset-definition
registration, account admission policies, multisig proposals, Space Directory manifests (AXT/AMX),
SoraFS pin manifests, and governance actions such as granting or revoking roles so common
roadmap-authoring tasks can be rehearsed without hand-writing Norito payloads.

## Cleanup & troubleshooting

- Stop the application to terminate supervised peers.
- Remove the sandbox root (`rm -rf <workspace>/.mochi/sandbox/<profile>`) to reset runtime state, or
  use `scripts/mochi_local_sandbox.sh reset`.
- If Kagami or irohad locations change, update the environment variables or re-run MOCHI with the
  appropriate CLI flags; the Settings dialog will persist new paths on the next apply.

For additional automation check `mochi/mochi-core/tests` (supervisor lifecycle tests) and
`mochi/mochi-integration` for mocked Torii scenarios. To ship bundles or wire the
desktop into CI pipelines, refer to the {doc}`mochi/packaging` guide.

## Local test gate

Run `ci/check_mochi.sh` before sending patches so the shared CI gate exercises all three MOCHI
crates:

```bash
./ci/check_mochi.sh
```

The helper executes `cargo check`/`cargo test` for `mochi-core`, `mochi-ui`, and
`mochi-integration`, which catches fixture drift (canonical block/event captures) and egui harness
regressions in one shot. If the script reports stale fixtures, rerun the ignored regeneration tests,
for example:

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

Re-running the gate after regenerating ensures the updated bytes stay consistent before you push.
