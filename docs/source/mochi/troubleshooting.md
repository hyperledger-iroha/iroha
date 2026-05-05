<!--
  SPDX-License-Identifier: Apache-2.0
-->

# MOCHI Troubleshooting Guide

Use this runbook when local MOCHI clusters refuse to start, get wedged in a
restart loop, or stop streaming block/event/status updates. It extends the
roadmap item “Documentation & rollout” by turning the supervisor behaviours in
`mochi-core` into concrete recovery steps.

## 1. First responder checklist

1. Capture the workspace root and sandbox root that MOCHI is using. The default
   layout is `<workspace>/.mochi/sandbox/<profile-slug>`; custom workspace and
   data-root overrides appear in the UI title bar and via
   `cargo run -p mochi-ui -- sandbox serve --workspace-root ...`.
   When you use `scripts/mochi_local_sandbox.sh`, the helper also uses
   `<workspace>/.mochi/build-target` as its default Cargo target dir unless
   `MOCHI_CARGO_TARGET_DIR` overrides it.
2. Run `./ci/check_mochi.sh` from the workspace root. This validates the core,
   UI, and integration crates before you begin modifying configs.
3. Note the preset (`single-peer` or `four-peer-bft`). The generated topology
   determines how many peer folders/logs you should expect under the sandbox
   root.
4. If you are using the shell helper, capture:
   - `scripts/mochi_local_sandbox.sh status`
   - `<workspace>/.mochi/sandbox/<profile>/serve.log`
   - `<workspace>/.mochi/sandbox/<profile>/session.json`
   - `<workspace>/.mochi/sandbox/<profile>/serve.pid`
   A `stale-session` status means `session.json` exists but the recorded Mochi
   process is gone; inspect `serve.log`, then rerun `up` or `reset`.

## 2. Collect logs & telemetry evidence

`NetworkPaths::ensure` (see `mochi/mochi-core/src/config.rs`) creates a stable
layout:

```
<sandbox_root>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
  session.json
  serve.log
```

Follow these steps before making changes:

- Use the **Logs** tab or open `logs/<alias>.log` directly to capture the last
  200 lines for each peer. The supervisor tails stdout/stderr/system channels
  via `PeerLogStream`, so these files match the UI output.
- For headless `sandbox serve` runs, capture `serve.log` as well. It holds the
  parent Mochi process output, including startup-stage failures before a peer
  log exists.
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
cargo run -p mochi-ui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

You can set `MOCHI_IROHAD`, `MOCHI_KAGAMI`, and `MOCHI_IROHA_CLI` to avoid
typing the flags repeatedly. When debugging bundle builds, compare the
`BundleConfig` in `mochi/mochi-ui-egui/src/config.rs` against the paths in
`target/mochi-bundle`.

For helper-script runs, check whether another process is holding the shared repo
`target/` lock. Current Mochi helpers default to an isolated
`<workspace>/.mochi/build-target`, so lock contention usually means
`MOCHI_CARGO_TARGET_DIR` was pointed back at a busy directory.

### Port collisions

`PortAllocator` probes the loopback interface before writing configs. If you see
`failed to allocate Torii port` or `failed to allocate P2P port`, another
process is already listening on the default range (8080/1337). Relaunch MOCHI
with explicit bases:

```bash
cargo run -p mochi-ui -- --torii-start 12000 --p2p-start 19000
```

The builder will fan out sequential ports from those bases, so reserve a range
sized for your preset (`peer_count` peers → `peer_count` ports per transport).

### Local MCP failed after peers started

`sandbox serve` now validates the local Torii MCP surface after `/status`
readiness. If startup reaches `serve.log` output such as `failed while
validating local MCP`, confirm:

- the active peer responds on `GET <torii>/v1/mcp`;
- `tools/list` exposes curated `iroha.*` tools such as
  `iroha.status`, `iroha.sumeragi.status`,
  `iroha.transactions.submit`, and
  `iroha.transactions.submit_and_wait`; and
- raw `torii.*` tools are not leaking through the local curated surface.

The helper script will not mark the sandbox ready until both `ready` and
`mcp_ready` are `true` in `session.json`.

### Readiness smoke failed after `/status` was ready

The local smoke path signs with the bundled primary development signer and
updates metadata on the existing `wonderland.universal` domain. It does not
create a new SNS-gated domain. Submission uses `Content-Type:
application/x-norito` and waits for commit through block/event streams plus
HTTP status fallback (`/v1/pipeline/transactions/status?hash=...`, then
explorer transaction lookup). A closed WebSocket stream should not fail
readiness by itself if the HTTP status endpoint reports the transaction as
committed.

If the smoke transaction rejects, treat the rejection text in `serve.log` as
authoritative. Common causes are stale storage from an older genesis, a
mismatched generated config, or a non-default profile whose genesis does not
include the sample domain/assets.

### Genesis and storage corruption

If Kagami exits before emitting a manifest, peers will crash immediately. Check
`genesis/*.json`/`.toml` inside the data root. Re-run with
`--kagami /path/to/kagami` or point the **Settings** dialog at the right binary.
For storage corruption, use the Maintenance section’s **Wipe & re-genesis**
button (covered below) instead of deleting folders by hand; it recreates the
peer directories and snapshot roots before restarting processes.

If startup fails during config integrity checks, compare the rendered peer
config with the validator-safe defaults Mochi now pins automatically:

- `nexus.enabled = false` for local permissioned profiles unless you explicitly
  enable Nexus.
- `confidential.enabled = true` for validator peers.
- `sumeragi.consensus_mode` must match the genesis block consensus mode Mochi
  asked Kagami to generate.
- `[torii.mcp]` should be enabled with `profile = "writer"` on local sandboxes.
- `[torii.transport.norito_rpc]` should have `enabled = true`,
  `require_mtls = false`, and `stage = "ga"` for the local SDK/RPC path.

When you explicitly enable Nexus, Mochi now rejects permissioned profiles before
launch and requires `sumeragi.consensus_mode = "npos"`.

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
   MOCHI_WORKSPACE_ROOT=/tmp/mochi-app MOCHI_PROFILE=four-peer-bft scripts/mochi_local_sandbox.sh reset
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
