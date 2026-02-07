---
lang: az
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ab0877a6f43402d6ec13a44c4a7c2b68e4a49e6103bb50d7469d9e71aaa953
source_last_modified: "2025-12-29T18:16:35.984945+00:00"
translation_last_reviewed: 2026-02-07
---

# MOCHI Packaging Guide

This guide explains how to build the MOCHI desktop supervisor bundle, inspect
the generated artefacts, and tune the runtime overrides that ship with the
bundle. It complements the quickstart by focusing on reproducible packaging
and CI usage.

## Prerequisites

- Rust toolchain (edition 2024 / Rust 1.82+) with workspace dependencies
  already built.
- `irohad`, `iroha_cli`, and `kagami` compiled for the desired target. The
  bundler reuses binaries from `target/<profile>/`.
- Sufficient disk space for the bundle output under `target/` or a custom
  destination.

Build the dependencies once before running the bundler:

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## Building the bundle

Invoke the dedicated `xtask` command from the repository root:

```bash
cargo xtask mochi-bundle
```

By default this produces a release bundle under `target/mochi-bundle/` with a
filename derived from the host OS and architecture (for example,
`mochi-macos-aarch64-release.tar.gz`). Use the following flags to customise
the build:

- `--profile <name>` – choose a Cargo profile (`release`, `debug`, or a
  custom profile).
- `--no-archive` – keep the expanded directory without creating a `.tar.gz`
  archive (useful for local testing).
- `--out <path>` – write bundles to a custom directory instead of
  `target/mochi-bundle/`.
- `--kagami <path>` – supply a prebuilt `kagami` executable to include in the
  archive. When omitted, the bundler reuses (or builds) the binary from the
  selected profile.
- `--matrix <path>` – append bundle metadata to a JSON matrix file (created if
  missing) so CI pipelines can record every host/profile artefact produced in a
  run. Entries include the bundle directory, manifest path and SHA-256, optional
  archive location, and the latest smoke-test result.
- `--smoke` – execute the packaged `mochi --help` as a lightweight smoke gate
  after bundling; failures surface missing dependencies before publishing an
  artefact.
- `--stage <path>` – copy the finished bundle (and archive when produced) into
  a staging directory so multi-platform builds can deposit artefacts in one
  location without extra scripting.

The command copies `mochi-ui-egui`, `kagami`, `LICENSE`, the sample
configuration, and `mochi/BUNDLE_README.md` into the bundle. A deterministic
`manifest.json` is generated alongside the binaries so CI jobs can track file
hashes and sizes.

## Bundle layout and verification

An expanded bundle follows the layout documented in `BUNDLE_README.md`:

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

The `manifest.json` file lists every artefact with its SHA-256 hash. Verify
the bundle after copying it to another system:

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

CI pipelines can cache the expanded directory, sign the archive, or publish
the manifest alongside release notes. The manifest includes the generator
profile, target triple, and creation timestamp to aid provenance tracking.

## Runtime overrides

MOCHI discovers helper binaries and runtime locations through CLI flags or
environment variables:

- `--data-root` / `MOCHI_DATA_ROOT` – override the workspace used for peer
  configs, storage, and logs.
- `--profile` – switch between topology presets (`single-peer`,
  `four-peer-bft`).
- `--torii-start`, `--p2p-start` – change the base ports used when allocating
  services.
- `--irohad` / `MOCHI_IROHAD` – point at a specific `irohad` binary.
- `--kagami` / `MOCHI_KAGAMI` – override the bundled `kagami`.
- `--iroha-cli` / `MOCHI_IROHA_CLI` – override the optional CLI helper.
- `--restart-mode <never|on-failure>` – disable automatic restarts or force the
  exponential backoff policy.
- `--restart-max <attempts>` – override the number of restart attempts when
  running in `on-failure` mode.
- `--restart-backoff-ms <millis>` – set the base backoff for automatic restarts.
- `MOCHI_CONFIG` – provide a custom `config/local.toml` path.

The CLI help (`mochi --help`) prints the full flag list. Environment overrides
take effect on launch and can be combined with the Settings dialog inside the
UI.

## CI usage hints

- Run `cargo xtask mochi-bundle --no-archive` to generate a directory that can
  be zipped with platform-specific tooling (ZIP for Windows, tarballs for
  Unix).
- Capture bundle metadata with `cargo xtask mochi-bundle --matrix dist/matrix.json`
  so release jobs can publish a single JSON index listing every host/profile
  artefact produced in the pipeline.
- Use `cargo xtask mochi-bundle --stage /mnt/staging/mochi` (or similar) on each
  build agent to upload the bundle and archive into a shared directory that the
  publishing job can consume.
- Publish both the archive and `manifest.json` so operators can verify bundle
  integrity.
- Store the generated directory as a build artefact to seed smoke tests that
  exercise the supervisor with deterministically packaged binaries.
- Record bundle hashes in release notes or in the `status.md` log for future
  provenance checks.
