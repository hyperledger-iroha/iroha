# MOCHI Development Notes

## Regression Checks

Run the following commands from the workspace root before submitting changes to MOCHI components:

```sh
cargo check -p mochi-core -p mochi-ui -p mochi-integration
cargo test -p mochi-integration
bash -n scripts/mochi_local_sandbox.sh
```

The `mochi-integration` crate provides lightweight Torii mocks and supervisor smoke tests so we can validate local workflows without compiling the full Iroha binary set.

## Fast Local Loop

For the desktop shell itself, the quickest happy path is:

```sh
cargo run -p mochi-ui --features gui --bin mochi -- --profile single-peer --build-binaries
```

Use `--profile four-peer-bft` when you want a closer validator/quorum rehearsal.
Unprofiled `single-peer` genesis keeps a 100 ms signed block cadence, while
multi-peer profiles use the one-second localnet cadence so crash-safe consensus
persistence can keep up when all validators share one development machine.
Explicit Kagami genesis profiles retain their profile-defined cadence.

The desktop app now treats the selected workspace as the home for bootstrap files and uses
`<workspace>/.mochi/sandbox/<profile>` as the default runtime state root. The dashboard and the
headless `sandbox serve` flow both write `.env.local` plus `.mochi/generated/*` into that
workspace, while runtime logs/storage/session metadata stay under `.mochi/sandbox/<profile>`.

On a clean launch, Mochi now opens a first-run wizard instead of dropping you
into the raw ops view. The default home is the Dashboard, which surfaces:

- prefunded dev accounts and explorer balances;
- recent blocks and one-click composer actions;
- copyable local shell exports for app bootstrap;
- generated bootstrap files in `.env.local` and `.mochi/generated/*`; and
- a Chaos Lab tab for quick peer bounce / partition / slowdown drills against
  the current supervised sandbox.

The Network page still exposes the lower-level launch recipe, app env snippet,
and `/status` curl probe so the same setup can move between GUI and shell
without reconstructing the config by hand.

## Headless Local Sandbox

Use the helper when you want Ganache/localton-style startup from a shell or from Codex:

```sh
scripts/mochi_local_sandbox.sh up
scripts/mochi_local_sandbox.sh status
scripts/mochi_local_sandbox.sh env
scripts/mochi_local_sandbox.sh mcp-add-command
```

By default the helper uses the current directory as `MOCHI_WORKSPACE_ROOT` and starts the
`single-peer` preset. Set `MOCHI_PROFILE=four-peer-bft` for the four-validator rehearsal, or
set `MOCHI_WORKSPACE_ROOT=/path/to/app` when the current shell is not already in the target app
workspace.
Set `MOCHI_PYTHON=/absolute/path/to/python3` when you need to select a specific validated
interpreter; the helper uses that one interpreter for every Python step.

`up` launches `cargo run -p mochi-ui --features gui --bin mochi -- sandbox serve` in a detached process group, waits for Torii
readiness, runs a local smoke transaction, validates the local MCP surface, writes
`<workspace>/.mochi/sandbox/<profile>/session.json`, and refreshes `.env.local` plus
`.mochi/generated/*` under the workspace. The helper records the long-lived Mochi process in
`serve.pid`, so `status` stays `ready` after the shell command returns and `down` can stop the
sandbox with SIGTERM. Cargo artifacts stay isolated under `<workspace>/.mochi/build-target` by
default (override with `MOCHI_CARGO_TARGET_DIR`) so local sandbox startup does not contend with
unrelated workspace builds.

The generated local Torii config enables both the curated `/v1/mcp` endpoint and local Norito-RPC
transport (`stage = "ga"`, no mTLS) so the same sandbox works for Codex MCP clients and local SDK
smoke tests without extra hand-edited config.

Generated local validator configs pin the runtime-critical local defaults Mochi depends on:
`nexus.enabled = false` unless explicitly enabled and `confidential.enabled = true`. Consensus mode
is carried by the signed genesis/height context, so Mochi does not emit the retired mutable
`sumeragi.consensus_mode` setting. If you enable Nexus, Mochi fails fast unless the selected profile
generates an NPoS signed genesis.

Each peer keeps its runtime data under `peers/<alias>/storage`, with independent `kura`, `snapshot`,
and `torii` children. Kura receives the dedicated `storage/kura` root so it can establish and
authenticate its configured-catalog baseline without unrelated runtime files in that directory.
Mochi initializes `storage/snapshot/generations` whenever it creates the explicit snapshot root,
matching the snapshot reader's authenticated directory contract.
Snapshot metadata pins this as `storage_layout = "kura-subdirectory-v1"`; restore rejects older
unmarked aggregate-layout snapshots because their Kura data cannot be relocated safely by inference.

## Repo-Shared Skill

The repo now ships a standalone skill at `skills/mochi-local-sandbox/`. Install or symlink that
directory into `$CODEX_HOME/skills/mochi-local-sandbox` when you want Codex to:

- bring the local Mochi sandbox up through `scripts/mochi_local_sandbox.sh up`;
- print or verify the exact `codex mcp add mochi-local --url ...` command;
- prefer curated local `iroha.*` MCP tools; and
- wire local apps from `.env.local` and `.mochi/generated/*` instead of ad-hoc env snippets.
