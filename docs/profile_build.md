# Profiling Rust builds

Use `scripts/profile_cargo_build.py` for reproducible build measurements. It
keeps Cargo artifacts and its report outside the checkout and never gives the
measured Cargo process the caller checkout as its working tree:

```sh
profile_dir="$(mktemp -d "${TMPDIR:-/tmp}/iroha-build-profile.XXXXXX")"
: "${IROHA_PROFILE_CARGO_HOME:?set a caller-private, offline-populated Cargo home}"
: "${IROHA_PROFILE_RUSTUP_HOME:?set a caller-private Rustup home}"
python3 scripts/profile_cargo_build.py \
  --target-dir "$profile_dir/target" \
  --out "$profile_dir/data-model.json" \
  --cargo-home "$IROHA_PROFILE_CARGO_HOME" \
  --rustup-home "$IROHA_PROFILE_RUSTUP_HOME" \
  -- build -p iroha_data_model --lib
```

Replace the Cargo arguments after `--` to measure another surface. Common
equivalents are `build --workspace`, `build -p irohad --bin iroha3d`, and
`build -p iroha_cli --bin iroha`. Only `build`, `check`, and compile-only
`test --no-run` profiles are accepted. The profiler adds Cargo's `--locked`,
`--offline`, JSON message output, timing output, and a deterministic job count
when the caller does not provide those controls. The default is one job; pass
`--jobs N` to compare a deliberately wider build lane.

The target directory, `--out` path, and explicit canonical `--cargo-home` and
`--rustup-home` roots must be outside and disjoint from the repository and one
another. The two homes must be caller-private, owned by the caller with mode
`0700`, and already contain everything needed by the offline build. PATH
entries must not be inside the checkout or either caller home.

Before measuring, the profiler creates an inode-independent source snapshot
from Git's tracked and non-ignored untracked path list. This preserves dirty
file bytes and tracked deletions. It also copies the caller Cargo home's
`registry/` and `git/` trees and the complete Rustup home into invocation-private
state. Source/cache/Rustup inventories and copies traverse below held directory
descriptors, so replacement symlinks in ancestor components cannot redirect a
read or write. Cargo runs with that source snapshot as its working directory, a
read-only source tree, writable private cache, private Rustup tree, private
HOME/tmp, and private Cargo/rustc/Git selections. Git discovery is performed
with optional locks, fsmonitor, untracked-cache, hooks, and system/global
configuration disabled. The caller source, Cargo cache, and Rustup tree are
inventoried again after the build; a change invalidates the report.

Each copied tree is bounded to 250,000 records, 4 GiB per file, 64 GiB total,
128 path components, and 4096 path bytes, and the copy must leave at least 1
GiB free. Special files, hard-linked regular files in writable caller-derived
inputs, absolute symlinks, and symlinks that escape their input root are
rejected. The target must be absent or empty for a cold measurement; pass
`--reuse-target` explicitly for a warm or no-op measurement. A reused target
may not contain hard-linked files. The initial warm-target inventory is
included in the report. The profiler never runs `cargo clean`.

The report, adjacent `.jsonl` and `.stderr.log` transcripts, and adjacent
`.state` path must all be absent. Report files are reserved without replacement
before Cargo starts; failed-output and invocation-private-state cleanup uses
atomic quarantine exchange and rechecks the exact inode owned by this
invocation. A foreign replacement is restored when possible; otherwise cleanup
fails with its retained quarantine name instead of deleting it. Cargo
`--target-dir`, `--config`, path-like `--target` values, external
`--manifest-path`, caller-input/cache/tool path-bearing arguments, and other
output-redirection controls are rejected. An allowed relative or absolute
manifest path is canonicalized against the caller checkout and remapped into
the private source snapshot before Cargo starts.

The JSON report binds the Cargo arguments, tracked and non-ignored untracked
source tree (including tracked deletions), Git revision, `Cargo.lock`, initial
Cargo/Rustup/warm-target inventories, selected build environment and PATH, and
the resolved Cargo, Rust compiler, and Git executable paths and byte digests.
Rustup shims, including hard-linked proxies, are resolved to the actual
selected toolchain binaries. The measured Cargo/rustc/Git paths and nested
lookups for those three names select the private copies; other PATH helpers
are neither copied nor byte-authenticated. Those inputs are captured again
after Cargo exits. If any input changes during the build,
`input_validation.stable` and top-level `valid` are false and an otherwise
successful invocation exits with status 3. Never compare a report unless
`valid` is true.

The authoritative report schema is version 3. The legacy named-scenario
wrapper emits schema version 2 after removal of process-table RSS sampling.

Keep the report together with its adjacent `.jsonl` Cargo message stream,
`.stderr.log`, and the Cargo timing HTML below the target directory. Compare
reports only when their input manifests differ by the intended change and the
host characteristics are suitable for the metric being compared.

`scripts/profile_build.sh` and `scripts/profile_build.py` remain as legacy
compatibility entry points for callers using the named `data-model`, `daemon`,
`cli`, and `workspace` scenarios. They use the same source/cache/Rustup/tool
snapshot boundary, accept `--root` for a non-default repository, require
`--cargo-home` and `--rustup-home` plus an absent external `--output`, run
locked and offline, and report elapsed time, completed child CPU usage, and
target size without observing the host process table. New
automation and retained profiling evidence should use
`scripts/profile_cargo_build.py`, whose source and toolchain fingerprints and
post-run drift check are authoritative.

The target directory and report bundle are deliberate mutable outputs. Reads
of caller inputs can still update filesystem access times on hosts that track
them. This is a path-isolation boundary, not an operating-system sandbox:
transitive build helpers found through the recorded PATH and hostile processes
with access to unrelated absolute host paths remain outside its guarantee.
