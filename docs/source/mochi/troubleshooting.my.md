---
lang: my
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb67b304bae01fa4a50d25dc9f086811dabfbcb24239b3ec9679338248e18be6
source_last_modified: "2025-12-29T18:16:35.985892+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# MOCHI Troubleshooting Guide

Use this runbook when local MOCHI clusters refuse to start, get wedged in a
restart loop, or stop streaming block/event/status updates. It extends the
roadmap item “Documentation & rollout” by turning the supervisor behaviours in
`mochi-core` into concrete recovery steps.

## 1. First responder checklist

1. Capture the data root that MOCHI is using. The default follows
   `$TMPDIR/mochi/<profile-slug>`; custom paths appear in the UI title bar and
   via `cargo run -p mochi-ui-egui -- --data-root ...`.
2. Run `./ci/check_mochi.sh` from the workspace root. This validates the core,
   UI, and integration crates before you begin modifying configs.
3. Note the preset (`single-peer` or `four-peer-bft`). The generated topology
   determines how many peer folders/logs you should expect under the data root.

## 2. Collect logs & telemetry evidence

`NetworkPaths::ensure` (see `mochi/mochi-core/src/config.rs`) creates a stable
layout:

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

Follow these steps before making changes:

- Use the **Logs** tab or open `logs/<alias>.log` directly to capture the last
  200 lines for each peer. The supervisor tails stdout/stderr/system channels
  via `PeerLogStream`, so these files match the UI output.
- Export a snapshot via **Maintenance → Export snapshot** (or call
  `Supervisor::export_snapshot`). The snapshot bundles storage, configs, and
  logs into `snapshots/<timestamp>-<label>/`.
- If the issue involves stream widgets, copy the `ManagedBlockStream`,
  `ManagedEventStream`, and `ManagedStatusStream` health indicators from the
  Dashboard. The UI surfaces the last reconnect attempt and error reason; grab
  a screenshot for the incident record.

## 3. Resolving peer startup issues

Most peer launch failures fall into three buckets:

### Missing binaries or bad overrides

`SupervisorBuilder` shells out to `irohad`, `kagami`, and (future) `iroha_cli`.
If the UI reports “failed to spawn process” or “permission denied”, point MOCHI
at known-good binaries:

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

You can set `MOCHI_IROHAD`, `MOCHI_KAGAMI`, and `MOCHI_IROHA_CLI` to avoid
typing the flags repeatedly. When debugging bundle builds, compare the
`BundleConfig` in `mochi/mochi-ui-egui/src/config/` against the paths in
`target/mochi-bundle`.

### Port collisions

`PortAllocator` probes the loopback interface before writing configs. If you see
`failed to allocate Torii port` or `failed to allocate P2P port`, another
process is already listening on the default range (8080/1337). Relaunch MOCHI
with explicit bases:

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

The builder will fan out sequential ports from those bases, so reserve a range
sized for your preset (`peer_count` peers → `peer_count` ports per transport).

### Genesis and storage corruption

If Kagami exits before emitting a manifest, peers will crash immediately. Check
`genesis/*.json`/`.toml` inside the data root. Re-run with
`--kagami /path/to/kagami` or point the **Settings** dialog at the right binary.
For storage corruption, use the Maintenance section’s **Wipe & re-genesis**
button (covered below) instead of deleting folders by hand; it recreates the
peer directories and snapshot roots before restarting processes.

### Tuning automatic restarts

`[supervisor.restart]` in `config/local.toml` (or the CLI flags
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) control how often the
supervisor retries failed peers. Set `mode = "never"` when you need the UI to
surface the first failure immediately, or shorten `max_restarts`/`backoff_ms`
to tighten the retry window for CI jobs that must fail fast.

## 4. Resetting peers safely

1. Stop the affected peers from the Dashboard or quit the UI. The supervisor
   refuses to wipe storage while a peer is running (`PeerHandle::wipe_storage`
   returns `PeerStillRunning`).
2. Navigate to **Maintenance → Wipe & re-genesis**. MOCHI will:
   - delete `peers/<alias>/storage`;
   - rerun Kagami to rebuild configs/genesis under `genesis/`; and
   - restart peers with the preserved CLI/environment overrides.
3. If you must do this manually:
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # Note the actual root printed above, then:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   Afterwards, restart MOCHI so `NetworkPaths::ensure` recreates the tree.

Always archive the `snapshots/<timestamp>` folder before wiping, even in local
development—those bundles capture the precise `irohad` logs and configs needed
to reproduce bugs.

### 4.1 Restoring from snapshots

When an experiment corrupts storage or you need to replay a known-good state, use the Maintenance
dialog’s **Restore snapshot** button (or call `Supervisor::restore_snapshot`) instead of copying
directories manually. Provide either an absolute path to the bundle or the sanitised folder name
under `snapshots/`. The supervisor will:

1. stop any running peers;
2. verify that the snapshot’s `metadata.json` matches the current `chain_id` and peer count;
3. copy `peers/<alias>/{storage,snapshot,config.toml,latest.log}` back into the active profile; and
4. restore `genesis/genesis.json` before restarting peers if they were running beforehand.

If the snapshot was created for a different preset or chain identifier the restore call returns a
`SupervisorError::Config` so you can grab a matching bundle instead of silently mixing artefacts.
Keep at least one fresh snapshot per preset to accelerate recovery drills.

## 5. Repairing block/event/status streams

- **Stream stalled but peers healthy.** Check the **Events**/**Blocks** panels
  for red status bars. Click “Stop” then “Start” to force the managed stream to
  resubscribe; the supervisor logs every reconnect attempt (with peer alias and
  error) so you can confirm backoff stages.
- **Status overlay out of date.** `ManagedStatusStream` polls `/status` every
  two seconds and marks data stale after `STATUS_POLL_INTERVAL *
  STATUS_STALE_MULTIPLIER` (default six seconds). If the badge stays red, verify
  `torii_status_url` in the peer config and ensure the gateway or VPN is not
  blocking loopback connections.
- **Event decoding failures.** The UI prints the decode stage (raw bytes,
  `BlockSummary`, or Norito decode) and the offending transaction hash. Export
  the event via the clipboard button so you can reproduce the decode in tests
  (`mochi-core` exposes helper constructors under
  `mochi/mochi-core/src/torii.rs`).

When streams repeatedly crash, update the issue with the exact peer alias and
error string (`ToriiErrorKind`) so the roadmap telemetry milestones stay tied
to concrete evidence.
