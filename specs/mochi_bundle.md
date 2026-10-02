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
alongside `manifest.json` with a sorted file inventory. The manifest lists every
file with its size and SHA-256 hash so CI pipelines can re-run verification or publish
attestations. The helper builds `mochi`, `kagami`, and `iroha3d` together in one
locked Cargo invocation and includes all three native executables. A prior
binary's existence is not a freshness check. Arbitrary Kagami overrides are
not accepted by the bundler, so it cannot mix a supplied helper with a newly
built daemon.

### Flags

| Flag                | Description                                                                 |
|---------------------|-----------------------------------------------------------------------------|
| `--out <dir>`       | Override the output directory (defaults to `target/mochi-bundle`).         |
| `--profile <name>`  | Build with a specific Cargo profile (e.g., `debug` for previews); `local-release` cannot be packaged. |
| `--no-archive`      | Skip the `.tar.gz` archive, leaving only the prepared folder.               |
| `--matrix <path>`   | Append bundle metadata to a JSON matrix for CI provenance tracking.         |
| `--smoke`           | Check packaged help, config-free source/bytecode/package deployment, live execution, repeated deployment, and four-validator restart with retained identity/state. |
| `--network-profiles <path>` | Validate and package a canonical artifact of independently installed network authorities. |
| `--stage <dir>`     | Copy the finished bundle (and archive, when present) into a staging folder. |

`--stage` is intended for CI pipelines where each build agent uploads its
artefacts to a shared location. The helper recreates the bundle directory and
copies the generated archive into the staging directory so publish jobs can
collect platform-specific outputs without shell scripting.

The canonical macOS package has one native application:

```text
Mochi.app/Contents/Info.plist
Mochi.app/Contents/MacOS/mochi
Mochi.app/Contents/MacOS/kagami
Mochi.app/Contents/MacOS/iroha3d
Mochi.app/Contents/Resources/network-profiles.nrt # optional installed authorities
docs/README.md
LICENSE
manifest.json
```

`iroha_deploy::managed::NativeBundleLayout` owns package paths for assembly,
installed smoke and latency controllers. Linux/Windows use `bin` for programs
and profiles, with `.exe` on Windows. macOS ships no outer copies or launcher
scripts. The generated plist uses `org.hyperledger.iroha.mochi`, executable
`mochi`, and the explicit numeric version from the **mochi-ui package manifest**.
Assembly is not code signing or notarization; those remain separate release
qualification work.

Installed runtime discovery accepts a direct `.app/Contents/MacOS` directory
and loads profiles only from that application's `Contents/Resources`. Moving or
renaming the application before first use preserves this relationship. Existing
managed generations retain their original pinned executable paths; package
relocation does not silently rebind them. Direct loose developer executables
(e.g. `target/debug`) are also supported explicitly and keep profiles beside
their programs. This development mode is not an alternate macOS package layout.

The manifest inventories all packaged files except itself, including the plist
and optional profiles. Inventory and staging propagate traversal failures and
reject symlinks or other nonregular entries instead of silently omitting them.
The inventory is sorted by relative path; its hashes need authenticated release
provenance before they establish download trust.

`--smoke` runs the packaged Kagami from an empty workspace with an empty `PATH`.
It supplies no TOML, starts the localnet through `contract deploy hello.ko`,
also deploys `.to` and a local Musubi package, and verifies artifact readback and
a live contract result on each of the four peers. It checks repeated starts and
deployments, then stops and restarts all four validators. The exact deployment
receipts and journals must survive. Cleanup uses authenticated
localnet control; failures retain the private runtime directory and diagnostics.
This single-run smoke does not establish the twenty-run latency target or the
remote private-dataspace acceptance gates.

The smoke's source, bytecode and local-package contracts have distinct behavior
and must produce pairwise distinct artifact hashes. The test controller prepares
the `.to` fixture offline with the canonical compiler; installed commands still
run with an empty `PATH` and no external compiler.

### Diagnostic latency samples

The contributor-only collector uses an existing bundle, separate fresh managed
state for each of twenty attempts, and the same bounded installed CLI controller:

```sh
cargo xtask mochi-latency --bundle target/mochi-bundle/mochi-<os>-<arch>-release --out target/devex-latency
cargo xtask mochi-latency-report --samples target/devex-latency --out target/devex-latency-report.json
cargo xtask mochi-latency-remote --bundle target/mochi-bundle/mochi-<os>-<arch>-release --driver <matching-release-iroha_deploy-test-executable> --out target/devex-remote-latency
```

Startup and ready-environment `.ko`, distinct `.to`, and local Musubi package
deployment are separate cases. The timer includes foreground CLI startup and
receipt processing; exact artifact readback and views on all four peers are
required postconditions outside that duration. “Fresh” means empty managed state,
not a flushed OS page cache. Fixture compilation and hashing are outside the timer.
No case measures a cold remote-provider package fetch.

Samples contain fixed outcome codes, integer nanoseconds, execution OS/architecture
and observed bundle hashes before/after; private logs and journals stay in
temporary custody. Aggregation preserves the recorded execution host and rejects
mixed hosts. Failures remain in the attempted count, and missing cases remain
incomplete. No retry replaces a failed ordinal. P95 is the nineteenth ordered
observation only when all twenty attempts for that case and all twenty overall
runs succeeded. Failed or uncertain cleanup retains private diagnostics;
uncertain cleanup stops the campaign.

The remote entry point invokes the exact eight-validator installed regression
twenty times sequentially. It records attachment and the three distinct private
deployment inputs separately, all labeled as a disposable loopback parent with
TLS. Deployment timing ends at local CLI Applied; exact child scope, artifact and
all-peer views remain required postconditions. Later parent anchoring is separate
evidence. The test driver digest, whole-test exit and authenticated cleanup of
both native stores and the TLS controller are recorded independently. Any later
proof, privacy or cleanup failure prevents a successful campaign or p95 threshold
claim, even when an earlier command succeeded. Missing dependent cases remain
unattempted. A missing cleanup confirmation stops further ordinals.

The remote report is written at `<out>/report.json`; raw typed records are in
`<out>/samples/` and can be passed to `mochi-latency-report`. A matching normal
release test driver is an externally verified prerequisite, not established by
its filename or digest. Python/OpenSSL belong only to the disposable test
controller, and no product installation gains a Python requirement.

Every report is diagnostic. Collection rejects non-release bundle profiles, but a
manifest hash or `profile: release` label does not authenticate a build. Signed
exact-source release provenance and qualified reference hardware remain separate
requirements for local results. Remote qualification additionally needs a healthy
funded parent with measured RTT and independently published network trust; the
disposable loopback workload cannot qualify Taira. The collector cannot promote
caller assertions into qualification.

The collector is implemented in source; no twenty-run campaign has yet been
executed for the current candidate. This section documents its measurement
boundary, not a measured performance result.

The `cargo xtask` alias enables the required `dev-tools` feature. Native release
qualification is dispatched through `.github/workflows/devex_native.yml` using
five explicitly supplied existing runner labels. Its resource and host checks,
native custody tests and installed-runtime smoke are execution gates; defining
the workflow does not establish that those platform jobs passed. An installation
without `--network-profiles` supports localnets and claims no remote authority.
The workflow also runs the eight-validator attachment regression with its
disposable TLS controller. Python 3.10+ with SSL and OpenSSL are test prerequisites;
the controller supplies a private temporary CA only to its own child processes.
Installed Kagami and Mochi flows continue to use the native bundle alone.

### Workspace selection

The packaged desktop selects the same workspace context as Kagami:

```
./Mochi.app/Contents/MacOS/mochi --workspace /path/to/project
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
