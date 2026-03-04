# MOCHI Desktop Bundle

This directory contains a portable build of the MOCHI desktop supervisor for
local Hyperledger Iroha networks. The layout is intentionally simple so it can
be unpacked anywhere and checked into reproducible build artefacts:

```
bin/mochi              # egui desktop executable
bin/kagami             # bundled kagami helper for genesis generation
config/sample.toml     # starter configuration and comments
docs/README.md         # this guide
LICENSE                # workspace licence
manifest.json          # deterministic file manifest with SHA-256 hashes
```

## Running the desktop shell

1. Install the `irohad` and `iroha_cli` binaries somewhere in your `PATH`.
   MOCHI supervises these executables instead of embedding the node itself.
   The bundle now includes a matching `kagami` binary under `bin/` so genesis
   generation works out of the box. If you store the binaries elsewhere, point
   the supervisor at them with the `MOCHI_IROHAD`/`MOCHI_KAGAMI`
   environment variables, command-line overrides (e.g., `./bin/mochi --kagami
   /path/to/kagami --chain-id sora-devnet`), or the `binaries`/`supervisor`
   sections in `config/local.toml`. When running from the source workspace you
   can also pass `--build-binaries` (or set `MOCHI_BUILD_BINARIES=true`) to let
   MOCHI invoke `cargo build` automatically when binaries are missing. Bundled
   configs can persist the same toggle via `supervisor.build_binaries = true`.
2. (Optional) Copy `config/sample.toml` to `config/local.toml` and customise the
   data directory, base ports, generated chain ID, or restart policy before
   first launch. The `[supervisor.restart]` table lets you disable automatic
   restarts (`mode = "never"`) or adjust the retry count/backoff for flaky
   development environments.
   You can also switch topology profiles: presets use `profile = "single-peer"`
   or `profile = "four-peer-bft"`, while custom profiles use a table such as
   `profile = { peer_count = 3, consensus_mode = "permissioned" }`. For NPoS
   genesis presets, set `consensus_mode = "npos"` and include
   `genesis_profile = "iroha3-dev"` in the same table (or set
   `supervisor.genesis_profile` when using presets).
   CLI runs accept the same presets or an inline profile table via
   `--profile '{ peer_count = 3, consensus_mode = "permissioned" }'`.
   For multi-lane/Nexus profiles, populate the `[nexus]` and `[sumeragi]`
   sections in `config/local.toml` (or pass `--nexus-config`, `--enable-nexus`,
   and `--enable-da` on the CLI). MOCHI validates `nexus.enabled` against lane
   counts and forces `sumeragi.da.enabled = true` when Nexus is enabled, because
   Iroha 3 always runs with DA gating enabled. Use `[torii.da_ingest]` to pin
   DA replay/manifest spool roots if you do not want the per-peer defaults.
3. Start the supervisor via `./bin/mochi`. The egui application will create the
   per-profile data tree on demand and generate a Kagami-aligned genesis block.
   The dashboard shows a compatibility summary (binary build-line/version and
   `kagami verify` output for genesis profiles) before peers are launched.
   Readiness is gated on a small smoke transaction by default; disable it with
   `--disable-smoke`, `MOCHI_READINESS_SMOKE=false`, or
   `supervisor.readiness_smoke = false`.

The bundle keeps everything relative so the archive can be expanded anywhere
on disk or inside CI artefact stores. All generated state (peer configs,
genesis manifests, logs, Kura storage) lives under the data root configured in
the sample manifest.
Generated `config.toml` files include a short MOCHI header with the resolved
chain id and (when available) consensus fingerprint so operators can confirm
they are launching the intended genesis profile.

## Topology recipes

Use these quick snippets inside `config/local.toml` to match common layouts:

```
[supervisor]
profile = "single-peer" # 1 node
```

```
[supervisor]
profile = { peer_count = 3, consensus_mode = "permissioned" } # 3 nodes
```

```
[supervisor]
profile = "four-peer-bft" # 4 nodes
```

```
[supervisor]
profile = { peer_count = 5, consensus_mode = "permissioned" } # 5 nodes
```

For NPoS/Nexus runs, switch to `consensus_mode = "npos"` and include a genesis
profile (`iroha3-dev`, `iroha3-testus`, or `iroha3-nexus`) as shown below.

## Nexus lane runs

Define Nexus lane catalogs in `config/local.toml`:

```
[supervisor]
profile = { peer_count = 4, consensus_mode = "npos", genesis_profile = "iroha3-nexus" }

[nexus]
enabled = true
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
dataspace = "universal"
visibility = "public"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
dataspace = "universal"
visibility = "restricted"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
dataspace = "universal"
visibility = "restricted"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0

[sumeragi]
da_enabled = true
```

Alternatively, store the `[nexus]` block above in a standalone TOML file and
load it with `--nexus-config path/to/nexus.toml` plus `--enable-nexus`.

To override Torii DA ingest spool roots, add:

```
[torii]
[torii.da_ingest]
replay_cache_store_dir = "/path/to/da_replay"
manifest_store_dir = "/path/to/da_manifests"
```

## Lane maintenance and status

The Settings dialog exposes lane catalogs, DA toggles, and DA spool roots, plus
a per-lane Kura/merge-log path preview (the generated peer configs include the
same paths in their header). Use the Maintenance bar to reset a single lane:
MOCHI wipes the lane storage, re-applies the lane catalog via Torii, and restarts
peers as needed. The Lane status panel surfaces DA cursors, relay lag, RBC bytes,
and relay ingest state per peer so operators can spot lagging lanes quickly.
The Settings dialog also includes a profile override field that accepts preset
slugs or inline TOML tables for custom peer counts/consensus modes.

## Deterministic manifest

`manifest.json` lists every file in the bundle with its byte size and SHA-256
hash. CI systems can use it to implement snapshot automation:

```json
{
  "generated_unix_ms": 1711843200000,
  "target": "macos-aarch64",
  "profile": "release",
  "files": [
    { "path": "bin/mochi", "size": 123456, "sha256": "…" }
  ]
}
```

Use `sha256sum` (or your platform equivalent) to verify entries against the
manifest when promoting bundles to release channels.

## Support

MOCHI is still in early development. File issues in the main Iroha repository
if the bundle script or documentation falls behind — PRs are welcome!
