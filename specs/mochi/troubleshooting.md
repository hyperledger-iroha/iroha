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
   `cargo run -p mochi-ui --features gui --bin mochi -- sandbox serve --workspace-root ...`.
   When you use `scripts/mochi_local_sandbox.sh`, the helper also uses
   `<workspace>/.mochi/build-target` as its default Cargo target dir unless
   `MOCHI_CARGO_TARGET_DIR` overrides it.
2. Run `./ci/check_mochi.sh` from the workspace root. This validates the core,
   UI, and integration crates before you begin modifying configs.
3. Note the preset (`four-peer-bft`, or a custom exact 3f+1 committee). The
   generated topology determines how many peer folders/logs you should expect
   under the sandbox root. Historical configs named `single-peer` also launch
   four validators.
4. If you are using the shell helper, capture:
   - `scripts/mochi_local_sandbox.sh status`
   - `<workspace>/.mochi/sandbox/<profile>/serve.log`
   - `<workspace>/.mochi/sandbox/<profile>/session.json`
   - `<workspace>/.mochi/sandbox/<profile>/serve.pid`
   A `stale-session` status means `session.json` exists but the recorded Mochi
   process is gone; inspect `serve.log`, then rerun `up` or `reset`. A
   `mismatched-pid` status means `serve.pid` points at a live process whose
   command line is not the expected `sandbox serve` command for this workspace;
   the helper will refuse to stop or reuse that PID, so inspect the pidfile and
   process manually before removing stale state.

## 2. Collect logs & telemetry evidence

`NetworkPaths::ensure` (see `mochi/mochi-core/src/config.rs`) creates the
network root. A successful generation transaction then publishes one immutable
configuration generation plus generation-bound mutable storage:

```
<sandbox_root>/
  .supervisor.lock
  current-generation
  generations/<config-generation-id>/
    genesis/...
    peers/<alias>/config.toml
  peers/<alias>/storage-generations/<storage-generation-id>/
    snapshot/generations/...
    kura/...
    torii/...
  logs/<alias>.log
  snapshots/
  session.json
  serve.log
```

Do not infer a mutable path such as `peers/<alias>/storage`. Read
`current-generation`, validate that generation's `generation.json`, then parse
its immutable `peers/<alias>/config.toml`. The canonical parent of
`snapshot.store_dir` is the selected mutable storage root. Config-only overlays
can select a newer config generation while intentionally retaining an older
storage generation. The V1 inventory is compact canonical Norito JSON and is
fail-closed at 8 MiB, 8,192 files, 16,384 tree entries, 32 directory levels,
4 KiB per relative path, and 4 MiB of aggregate relative-path text. Hashing and
tree sync stream bounded files instead of materializing the whole tree.
`resolve_selected_peer_storage_paths` performs these checks
for detached tooling and returns a shared generation-selection lease. Keep the
returned `SelectedPeerStoragePaths` value alive for the entire operation; a
copied `PathBuf` does not retain that protection. An active `PeerHandle`
exposes the already-selected `storage_dir()` and `snapshot_dir()` paths. Before
start, export, restore, overlay, or wipe operations, the supervisor revalidates
every cached peer path against the selected config while retaining one shared
or exclusive generation lock; intermediate symlinks fail closed.

Only one live `Supervisor` may own a sandbox root. The owner-only
`.supervisor.lock` is acquired before Mochi creates logs, peer containers,
snapshots, onboarding material, or generation candidates. Managed peers reserve
stdin for an inherited duplicate of that ownership descriptor, so a peer
orphaned by a controller crash keeps the root fenced until it exits. Use the
consuming supervisor-replacement flow for in-process settings rebuilds; do not
construct a second handle for the same root.

Follow these steps before making changes:

- Use the **Logs** tab or open `logs/<alias>.log` directly to capture the last
  200 lines for each peer. The supervisor tails stdout/stderr/system channels
  via `PeerLogStream`, so these files match the UI output.
- For headless `sandbox serve` runs, capture `serve.log` as well. It holds the
  parent Mochi process output, including startup-stage failures before a peer
  log exists.
- Export a snapshot via **Maintenance → Export snapshot** (or call
  `Supervisor::export_snapshot`). The snapshot bundles storage, configs, and
  logs into `snapshots/<timestamp>-<label>/`. Digesting walks canonical UTF-8
  paths through one 4,096-entry lexical window at a time, streams every file,
  and rejects directory nesting beyond 64 levels; it does not materialize the
  full file inventory or any file-sized comparison buffer.
- If the issue involves stream widgets, copy the `ManagedBlockStream`,
  `ManagedEventStream`, and `ManagedStatusStream` health indicators from the
  Dashboard. The UI surfaces the last reconnect attempt and error reason; grab
  a screenshot for the incident record.

## 3. Resolving peer startup issues

Most peer launch failures fall into three buckets:

### Missing binaries or bad overrides

`SupervisorBuilder` shells out to `iroha3d`, `kagami`, and `iroha`.
If the UI reports “failed to spawn process” or “permission denied”, point MOCHI
at known-good binaries:

```bash
cargo run -p mochi-ui --features gui --bin mochi -- \
  --irohad /path/to/iroha3d \
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
cargo run -p mochi-ui --features gui --bin mochi -- --torii-start 12000 --p2p-start 19000
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
one authoritative HTTP reconciliation route:
`/v1/pipeline/transactions/status?hash=<marked-64-lowercase-hex>&scope=global`,
where the transaction hash matches `[0-9a-f]{63}[13579bdf]` and therefore
retains Iroha's canonical `HashOf` marker.
Mochi never falls back to an Explorer transaction lookup. The response must
match that exact hash and contain exactly `hash`, `status`, `scope`, and
`resolved_from`; `status` contains `kind` and only the optional
`block_height`. Missing, unknown, or aliased fields fail closed. Only a global,
state-resolved `Applied` status with a positive integer `block_height` completes
the smoke check. Queue/cache observations remain progress hints. A closed
WebSocket stream should not fail readiness by itself if this status route proves
exact Applied execution finality.

If the smoke transaction rejects, treat the rejection text in `serve.log` as
authoritative. Common causes are stale storage from an older genesis, a
mismatched generated config, or a non-default profile whose genesis does not
include the sample domain/assets.

### Genesis and storage corruption

If Kagami exits before emitting a manifest, peers will crash immediately. Check
the validated selected generation's `genesis/` directory. Re-run with
`--kagami /path/to/kagami` or point the **Settings** dialog at the right binary.
For storage corruption, use the Maintenance section’s **Wipe & re-genesis**
button (covered below) instead of deleting folders by hand; it recreates the
peer directories and snapshot roots before restarting processes.

If startup fails during config integrity checks, compare the rendered peer
config with the validator-safe defaults Mochi now pins automatically:

- Nexus has no availability switch; the first release has one transaction-aware
  routing runtime for both canonical one-lane and custom multi-lane deployments.
- `confidential.enabled = true` for validator peers.
- Consensus mode is carried by the signed genesis/height context. Mochi does
  not emit the retired mutable `sumeragi.consensus_mode` config field; select
  the matching profile instead of adding that field by hand.
- `[torii.mcp]` should be enabled with `profile = "writer"` on local sandboxes.
- `[torii.transport.norito_rpc]` should have `enabled = true`,
  `require_mtls = false`, and `stage = "ga"` for the local SDK/RPC path.

The canonical one-lane topology is valid with permissioned consensus. Custom
multi-lane topology requires a profile that generates an NPoS signed genesis.

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
   - prepare and validate a new immutable config/genesis generation;
   - allocate a fresh `peers/<alias>/storage-generations/<new-id>` tree without
     deleting the retired generation's state;
   - atomically select the new config generation; and
   - restore exactly the peers that were running before the operation. A
     partial-running topology remains partial.
3. If you must do this manually:
   ```bash
   MOCHI_WORKSPACE_ROOT=/tmp/mochi-app MOCHI_PROFILE=four-peer-bft scripts/mochi_local_sandbox.sh reset
   ```
   Afterwards, restart MOCHI so `NetworkPaths::ensure` recreates the tree.

Never delete a path chosen only by directory naming. Use the validated selected
config and its managed storage paths. Retired immutable and storage generations
remain available for diagnosis; archive a `snapshots/<timestamp>` bundle before
intentional cleanup because it captures the precise `irohad` logs and configs
needed to reproduce bugs.

### 4.1 Restoring from snapshots

When an experiment corrupts storage or you need to replay a known-good state, use the Maintenance
dialog’s **Restore snapshot** button (or call `Supervisor::restore_snapshot`) instead of copying
directories manually. Provide either an absolute path to the bundle or the sanitised folder name
under `snapshots/`. The supervisor will:

1. retain a shared generation-selection lease and strictly revalidate every selected peer storage
   hierarchy before stopping or copying anything;
2. verify that the snapshot’s `metadata.json` matches the current `chain_id` and peer count;
3. verify byte-for-byte that its manifest, signed genesis, and every peer config match the
   validated selected immutable generation;
4. verify every copied storage tree against its recorded integrity hash; and
5. capture and stop exactly the currently running peer aliases, replace only selected mutable
   storage and logs, then restore that exact set on success or failure. Immutable config/genesis
   artifacts are never overwritten.

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
