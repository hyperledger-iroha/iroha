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
attestations. The helper ensures both the `mochi` desktop shell and the
workspace `kagami` binary are present so genesis generation works out of the
box.

### Flags

| Flag                | Description                                                                 |
|---------------------|-----------------------------------------------------------------------------|
| `--out <dir>`       | Override the output directory (defaults to `target/mochi-bundle`).         |
| `--profile <name>`  | Build with a specific Cargo profile (e.g., `debug` for tests).              |
| `--no-archive`      | Skip the `.tar.gz` archive, leaving only the prepared folder.               |
| `--kagami <path>`   | Use an explicit `kagami` binary instead of building `iroha_kagami`.         |
| `--matrix <path>`   | Append bundle metadata to a JSON matrix for CI provenance tracking.         |
| `--smoke`           | Run `mochi --help` from the packaged bundle as a basic execution gate.      |
| `--stage <dir>`     | Copy the finished bundle (and archive, when present) into a staging folder. |

`--stage` is intended for CI pipelines where each build agent uploads its
artefacts to a shared location. The helper recreates the bundle directory and
copies the generated archive into the staging directory so publish jobs can
collect platform-specific outputs without shell scripting.

The layout inside the bundle is intentionally simple:

```
bin/mochi              # egui desktop executable
bin/kagami             # kagami helper for genesis generation
config/sample.toml     # starter supervisor configuration
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

### Runtime overrides

The packaged `mochi` executable accepts command-line overrides for the most
common supervisor settings. Use these flags instead of editing
`config/local.toml` when experimenting:

```
./bin/mochi --data-root ./data --profile four-peer-bft \
    --torii-start 12000 --p2p-start 14000 \
    --irohad /path/to/irohad --kagami /path/to/kagami
```

Any CLI value takes precedence over `config/local.toml` entries and environment
variables.

## Snapshot automation

`manifest.json` records the generation timestamp, target triple, Cargo profile,
and the complete file inventory. Pipelines can diff the manifest to detect when
new artefacts appear, upload the JSON alongside release assets, or audit the
hashes before promoting a bundle to operators.

The helper is idempotent: re-running the command updates the manifest and
overwrites the previous archive, keeping `target/mochi-bundle/` as the single
source of truth for the latest bundle on the current machine.
