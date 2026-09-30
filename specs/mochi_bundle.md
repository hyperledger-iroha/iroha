# MOCHI Bundle Tooling

MOCHI ships with a lightweight packaging workflow so developers can produce a
portable desktop bundle without wiring bespoke CI scripts. The `xtask`
subcommand handles compilation, layout, hashing, and (optionally) archive
creation in one shot.

## Generating a bundle

```bash
cargo xtask mochi-bundle
```

By default the command builds release binaries, assembles the bundle under
`target/mochi-bundle/`, and emits a `mochi-<os>-<arch>-release.tar.gz` archive
alongside a deterministic `manifest.json`. The manifest lists every file with
its size and SHA-256 hash so CI pipelines can re-run verification or publish
attestations. The helper builds `mochi`, `kagami`, and `iroha3d` together in one
locked Cargo invocation and includes all three native executables. A prior
binary's existence is not a freshness check. Arbitrary Kagami overrides are
not accepted by the bundler, so it cannot mix a supplied helper with a newly
built daemon.

### Flags

| Flag                | Description                                                                 |
|---------------------|-----------------------------------------------------------------------------|
| `--out <dir>`       | Override the output directory (defaults to `target/mochi-bundle`).         |
| `--profile <name>`  | Build with a specific Cargo profile (e.g., `debug` for tests).              |
| `--no-archive`      | Skip the `.tar.gz` archive, leaving only the prepared folder.               |
| `--matrix <path>`   | Append bundle metadata to a JSON matrix for CI provenance tracking.         |
| `--smoke`           | Check packaged help, config-free source deployment, repeated deployment, and four-validator restart with retained identity/state. |
| `--stage <dir>`     | Copy the finished bundle (and archive, when present) into a staging folder. |

`--stage` is intended for CI pipelines where each build agent uploads its
artefacts to a shared location. The helper recreates the bundle directory and
copies the generated archive into the staging directory so publish jobs can
collect platform-specific outputs without shell scripting.

The layout inside the bundle is intentionally simple:

```
bin/mochi              # egui desktop executable
bin/kagami             # developer CLI and managed runtime worker
bin/iroha3d            # matching native validator executable
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

`--smoke` runs the packaged Kagami from an empty workspace with an empty `PATH`.
It supplies no TOML, starts the localnet through `contract deploy hello.ko`,
checks repeated starts and deployments, then stops and restarts all four validators.
The exact deployment receipt and journal must survive. Cleanup uses authenticated
localnet control; failures retain the private runtime directory and diagnostics.
This single-run smoke does not establish the twenty-run latency target or the
remote private-dataspace acceptance gates.

### Workspace selection

The packaged desktop selects the same workspace context as Kagami:

```
./bin/mochi --workspace /path/to/project
```

Omitting `--workspace` selects the current directory. Generated configuration,
credentials and retained ledger state live in private OS application storage.
The desktop uses the matching sibling Kagami and daemon; it does not require
sample TOML, invoke Cargo, search PATH for a daemon or own a second supervisor.

## Snapshot automation

`manifest.json` records the generation timestamp, target triple, Cargo profile,
and the complete file inventory. Pipelines can diff the manifest to detect when
new artefacts appear, upload the JSON alongside release assets, or audit the
hashes before promoting a bundle to operators.

The helper is idempotent: re-running the command updates the manifest and
overwrites the previous archive, keeping `target/mochi-bundle/` as the single
source of truth for the latest bundle on the current machine.
