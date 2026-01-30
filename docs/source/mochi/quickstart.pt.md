---
lang: pt
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44faf6c98d141959cf8cf40b1df7d3d82c3448e6f2b1bc4fa54cdeceb97994b0
source_last_modified: "2026-01-03T18:07:56.999063+00:00"
translation_last_reviewed: 2026-01-30
---

# MOCHI Quickstart

**MOCHI** is the desktop supervisor for local Hyperledger Iroha networks. This guide walks through
installing the prerequisites, building the application, launching the egui shell, and using the
runtime tools (settings, snapshots, wipes) for day‑to‑day development.

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
cargo build -p mochi-ui-egui
```

This command builds both `mochi-core` and the egui frontend. To produce a distributable bundle, run:

```bash
cargo xtask mochi-bundle
```

The bundle task assembles the binaries, manifest, and config stubs under `target/mochi-bundle`.

## Launching the egui shell

Run the UI directly from cargo:

```bash
cargo run -p mochi-ui-egui
```

By default MOCHI creates a single-peer preset in a temporary data directory:

- Data root: `$TMPDIR/mochi`.
- Torii base port: `8080`.
- P2P base port: `1337`.

Use CLI flags to override the defaults when launching:

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

Environment variables mirror the same overrides when CLI flags are omitted: set `MOCHI_DATA_ROOT`,
`MOCHI_PROFILE`, `MOCHI_CHAIN_ID`, `MOCHI_TORII_START`, `MOCHI_P2P_START`, `MOCHI_RESTART_MODE`,
`MOCHI_RESTART_MAX`, or `MOCHI_RESTART_BACKOFF_MS` to preseed the supervisor builder; binary paths
continue to respect `MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI`, and `MOCHI_CONFIG` points at an
explicit `config/local.toml`.

## Settings & persistence

Open the **Settings** dialog from the dashboard toolbar to adjust the supervisor configuration:

- **Data root** — base directory for peer configs, storage, logs, and snapshots.
- **Torii / P2P base ports** — starting ports for deterministic allocation.
- **Log visibility** — toggle stdout/stderr/system channels in the log viewer.

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

The **Maintenance** dialog exposes two safety operations:

- **Export snapshot** — copies peer storage/config/logs and the current genesis manifest into
  `snapshots/<label>` under the active data root. Labels are sanitized automatically.
- **Restore snapshot** — rehydrates peer storage, snapshot roots, configs, logs, and the genesis
  manifest from an existing bundle. `Supervisor::restore_snapshot` accepts either an absolute path or
  the sanitised `snapshots/<label>` folder name; the UI mirrors this flow so Maintenance → Restore
  can replay evidence bundles without touching files manually.
- **Wipe & re-genesis** — stops running peers, removes storage directories, regenerates genesis via
  Kagami, and restarts peers when the wipe completes.

Both flows are covered by regression tests (`export_snapshot_captures_storage_and_metadata`,
`wipe_and_regenerate_resets_storage_and_genesis`) to guarantee deterministic outputs.

## Logs & streams

The dashboard exposes data/metrics at a glance:

- **Logs** — follows `irohad` stdout/stderr/system lifecycle messages. Toggle channels in Settings.
- **Blocks / Events** — managed streams auto-reconnect with exponential backoff and annotate frames
  with Norito-decoded summaries.
- **Status** — polls `/status` and renders sparklines for queue depth, throughput, and latency.
- **Startup readiness** — after pressing **Start** (single peer or all peers), MOCHI probes
  `/status` with bounded backoff; the banner reports when each peer goes ready (with the observed
  queue depth) or surfaces the Torii error if readiness times out.

Tabs for state explorer and composer provide quick access to accounts, assets, peers, and common
instructions without leaving the UI. The Peers view mirrors the `FindPeers` query so you can confirm
which public keys are currently registered in the validator set before running integration tests.

Use the composer toolbar's **Manage signing vault** button to import or edit signing authorities. The
dialog writes entries to the active network root (`<data_root>/<profile>/signers.json`), and saved
vault keys are immediately available for transaction previews and submissions. When the vault is
empty the composer falls back to the bundled development keys so local workflows continue to work.
Forms now cover mint/burn/transfer (including implicit receive), domain/account/asset-definition
registration, account admission policies, multisig proposals, Space Directory manifests (AXT/AMX),
SoraFS pin manifests, and governance actions such as granting or revoking roles so common
roadmap-authoring tasks can be rehearsed without hand-writing Norito payloads.

## Cleanup & troubleshooting

- Stop the application to terminate supervised peers.
- Remove the data root (`rm -rf <data_root>`) to reset all state.
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

The helper executes `cargo check`/`cargo test` for `mochi-core`, `mochi-ui-egui`, and
`mochi-integration`, which catches fixture drift (canonical block/event captures) and egui harness
regressions in one shot. If the script reports stale fixtures, rerun the ignored regeneration tests,
for example:

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

Re-running the gate after regenerating ensures the updated bytes stay consistent before you push.
