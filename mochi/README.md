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
cargo run -p mochi-ui --features gui --bin mochi -- --profile four-peer-bft --build-binaries
```

The default four-validator topology is the smallest exact Sumeragi committee.
Custom profiles may use four or seven validators and use the one-second
localnet cadence so crash-safe consensus persistence can keep up when all
validators share one development machine. Explicit Kagami genesis profiles
retain their profile-defined cadence. The historical `single-peer` profile
name remains readable for saved-config compatibility but launches four
validators.

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
`four-peer-bft` preset. Set `MOCHI_WORKSPACE_ROOT=/path/to/app` when the current shell is not
already in the target app workspace.
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

Mochi also provisions signer-backed local account onboarding for the universal dataspace. The
owner-only signer and token remain under the sandbox `runtime/` directory; `session.json` exposes
only the `local-dev` credential identifier and the `onboarding_signer_file` and
`onboarding_token_file` paths so local applications can use the bundle without copying its raw
secrets or digest into generated metadata.

To qualify the transactional wipe path against real binaries, use the bounded one-shot rehearsal
with a fresh data root:

```sh
rehearsal_root="$(mktemp -d)"
cargo run -p mochi-ui --features gui --bin mochi -- \
  sandbox rehearse-wipe-and-regenerate \
  --data-root "$rehearsal_root" \
  --profile four-peer-bft \
  --build-binaries \
  --enable-smoke
```

The command starts four real peers, proves committed genesis, readiness, and the local MCP surface,
calls `Supervisor::wipe_and_regenerate` while that exact peer set is running, and repeats every
proof against the new generation. It fails unless the selected generation changes and all four
aliases return. On success it stops the peers and prints one bounded Norito JSON evidence record;
the disposable data root remains available for audit.

Generated local validator configs pin the runtime-critical local defaults Mochi depends on:
`nexus.enabled = false` unless explicitly enabled and `confidential.enabled = true`. Consensus mode
is carried by the signed genesis/height context, so Mochi does not emit the retired mutable
`sumeragi.consensus_mode` setting. If you enable Nexus, Mochi fails fast unless the selected profile
generates an NPoS signed genesis.

Mochi publishes configs and genesis as immutable generations under `generations/<generation-id>`.
The closed `generation.json` inventory binds every artifact and its BLAKE3 digest; the
`current-generation` record is replaced atomically while `.generation.lock` serializes writers.
Failed candidates never replace the selected record, and previously published generations remain
available for audit. Generation V1 seals at most 8,192 files, 16,384 total tree entries, 32
directory levels, 4 MiB of relative-path text, and an 8 MiB canonical compact inventory. Mochi
walks and hashes the tree incrementally and rejects the first over-limit entry before retaining it,
so corrupt local state cannot turn publication or recovery into a directory-sized allocation.

Each peer keeps mutable runtime data under
`peers/<alias>/storage-generations/<generation-id>`, with independent `kura`, `snapshot`, and
`torii` children. A config-only generation keeps using the current storage generation, while wipe
and re-genesis prepares a fresh empty storage generation before committing its config/genesis
generation. This avoids a partially wiped peer set after the atomic selection point. Kura receives
the dedicated `storage-generations/<generation-id>/kura` root, and Mochi initializes the matching
`snapshot/generations` directory whenever it creates a fresh runtime generation.
Snapshot metadata pins this as `storage_layout = "kura-subdirectory-v1"`; restore rejects older
unmarked aggregate-layout snapshots and snapshots from another immutable generation. Config and
genesis copies in a snapshot are audit evidence; restore verifies them and rewrites only mutable
storage and logs. Snapshot digests stream file contents and enumerate each directory through a
canonical 4,096-entry lexical window (maximum depth 64), so the digest supports long Kura histories
without retaining a path or byte vector proportional to the whole snapshot.

## Repo-Shared Skill

The repo now ships a standalone skill at `skills/mochi-local-sandbox/`. Install or symlink that
directory into `$CODEX_HOME/skills/mochi-local-sandbox` when you want Codex to:

- bring the local Mochi sandbox up through `scripts/mochi_local_sandbox.sh up`;
- print or verify the exact `codex mcp add mochi-local --url ...` command;
- prefer curated local `iroha.*` MCP tools; and
- wire local apps from `.env.local` and `.mochi/generated/*` instead of ad-hoc env snippets.
