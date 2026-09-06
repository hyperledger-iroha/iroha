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

On macOS, select Xcode's actual Git binary on PATH (locate it with
`xcrun --find git`). The `/usr/bin/git` system launcher may be terminated by
macOS when copied for private execution. The selected Git path and bytes must
match between baseline and candidate.

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

Source snapshots preserve dangling relative links whose complete resolution
stays within the repository. This records missing optional native artifacts
without inventing them; any build needing one still fails. Cache and toolchain
copies require available link targets. Link resolution checks each component
before processing parent traversal, and rejects escaping chains even when the
final target is absent.

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

The authoritative report schema is version 4. Each compiler invocation runs
through a private `RUSTC` and PATH launcher, which calls `wait4` for that exact
child. `RUSTC_WRAPPER` is unset: build scripts invoking `RUSTC` directly must also
be measured. The launcher embeds the pinned real compiler and preserves its arguments. Its peak
RSS is recorded in bytes (Darwin reports bytes; Linux reports KiB, which the
wrapper multiplies by 1024). The profiler does not use the parent process's
cumulative child high-water mark. The separate Cargo-process peak includes its
child accounting and must not be interpreted as simultaneous process-tree
memory or as an individual compiler unit's memory.

`result.compiler_measurements.compiled` contains one measurement for each
compiled Cargo artifact. It includes the package, target name/kinds/crate
types, package-relative source path, sorted features, complete Cargo profile,
compiler arguments, elapsed time, CPU time, peak RSS, and SHA-256/size identities
for the compiler and Cargo output files. Rustc's artifact diagnostics are
forwarded unchanged. Matching checks the source/manifest, crate name, features,
codegen profile, and emitted artifact bytes, including Cargo's copies of
executables. A missing or inconsistent Cargo completion message, an empty artifact inventory,
or missing, duplicate, unmatched, or changed evidence invalidates the
report and exits with status 4. The profiler entry point, measurement helper,
private launcher, and Python and shell interpreter bytes are checked for drift.
Their identities remain explicit when `--root` selects a frozen source checkout
separate from the profiler's own checkout.

The wrapper forwards Cargo's inherited jobserver descriptors to `rustc` so
measurement preserves Cargo's concurrency control. It rejects malformed,
conflicting, or unavailable descriptors before invoking the compiler. Raw
compiler records and their digest remain in the report when artifact matching
fails, for diagnosis; only reconciled `compiler_measurements` qualify as
per-unit memory evidence. A failed match does not produce a valid baseline.
Cargo-selected features remain distinct from compiler `cfg` features injected
by a build script. Injected features must match Cargo's `build-script-executed`
record for the exact package and compiler `OUT_DIR`; unaccounted, conflicting,
or differently owned cfg evidence invalidates the measurement.

Compiler discovery and generated-source probes have separate records from Cargo
compilation units. Source probes preserve rustc arguments, stdin and diagnostics;
record schema 3 binds the generated input by byte count and SHA-256, including
stdin, package-source files and generated `OUT_DIR` files. A real negative compiler probe
remains a negative result. An instrumentation failure always leaves a fatal
record, even if a build script ignores its exit status; it cannot qualify a
profile through a fallback configuration. Real compiler tests exercise std
probing under build, check, release and test target selection. Failed Cargo
compiler invocations also retain their own records. Cached
artifacts appear under `fresh` without an invented RSS value. A warm/no-op run
can therefore be a valid observation, but cannot establish a cold compile-unit
memory baseline. Use a fresh target for each baseline/candidate measurement;
require every governed unit to have a successful compiled measurement before
applying its memory budget. The compiler-measurement digest binds the complete
measurement inventory. Other PATH helpers and Python's standard library retain
the existing host-runtime trust boundary; the wrapper is instrumentation, not
an operating-system sandbox.

Keep the report together with its adjacent `.jsonl` Cargo message stream,
`.stderr.log`, and the Cargo timing HTML below the target directory. Compare
reports only when their input manifests differ by the intended change and the
host characteristics are suitable for the metric being compared.

`scripts/profile_cargo_build.py` is the only build-profiling entry point. Its
source and toolchain fingerprints and post-run drift check are authoritative.

The target directory and report bundle are deliberate mutable outputs. Reads
of caller inputs can still update filesystem access times on hosts that track
them. This is a path-isolation boundary, not an operating-system sandbox:
transitive build helpers found through the recorded PATH and hostile processes
with access to unrelated absolute host paths remain outside its guarantee.


## Cold architecture qualification

Freeze the intended baseline and candidate input trees before measuring. A
concurrent source, lockfile, toolchain, or caller-cache write invalidates the
run. Use Rust 1.93.1 and the same profiler/helper, interpreter, host architecture,
Cargo arguments, features, job count, and cache/toolchain inputs for both sides.
The refreshed candidate lockfile belongs to the candidate source identity;
it does not need to equal the historical signed lock anchor.

The following surfaces use their ordinary portable production features; Core's
explicit test feature includes the full Core test targets, and the native
privacy bridge uses its production feature. They compile tests without running
them. Set the four external paths below, then run one surface at a time. Give
the baseline and candidate different output roots, and keep every target and
report path absent until its own cold run.

```sh
: "${IROHA_PROFILE_SOURCE_ROOT:?set the frozen source checkout}"
: "${IROHA_PROFILE_OUTPUT_ROOT:?set an external output directory}"
: "${IROHA_PROFILE_CARGO_HOME:?set a private populated offline Cargo home}"
: "${IROHA_PROFILE_RUSTUP_HOME:?set a private populated Rustup home}"

profile_surface() {
  iroha_profile_surface="$1"
  shift
  python3 scripts/profile_cargo_build.py \
    --root "$IROHA_PROFILE_SOURCE_ROOT" \
    --target-dir "$IROHA_PROFILE_OUTPUT_ROOT/$iroha_profile_surface-target" \
    --out "$IROHA_PROFILE_OUTPUT_ROOT/$iroha_profile_surface.json" \
    --cargo-home "$IROHA_PROFILE_CARGO_HOME" \
    --rustup-home "$IROHA_PROFILE_RUSTUP_HOME" \
    --jobs 1 --label "$iroha_profile_surface" -- "$@"
}

profile_surface sdk build -p iroha --lib
profile_surface model build -p iroha_data_model --lib
profile_surface core-tests test -p iroha_core --features iroha-core-tests
profile_surface torii-tests test -p iroha_torii
profile_surface daemon-release build --release -p irohad --bin iroha3d
profile_surface native-bridge-release build --release -p connect_norito_bridge \
  --lib --features privacy-production-enabled
profile_surface javascript-native-release build --release -p iroha_js_host --lib
```

Use a Unix host with `wait4` and enough physical memory for the source being
measured plus Cargo/linker/OS overhead. An initial baseline may exceed the
13 GiB release ceiling; that ceiling is an acceptance limit, not a prediction
of the current build's peak. Reserve sufficient disk for an independent source
snapshot, the selected Cargo cache, the complete Rustup home and each retained
target. The copy limits and mandatory 1 GiB free-space
reserve above are preflight bounds, not a prediction of build-output size;
measure the source/cache/toolchain sizes and reserve additional output space
before starting. Populate all target-specific dependencies and toolchains in
the private homes before the offline run. Cross-target/native SDK release
qualification also requires its actual platform toolchain and target-specific
feature selection; the host bridge commands above do not certify other slices.

Per-unit RSS recording does not itself qualify the redesign or change the
13 GiB release ceiling. Real source-bound baseline/candidate captures, the
comparison against the approved per-unit budgets, and the existing sealed
release artifact/identity gates are still required. Disposable compiler tests
and source inspection cannot substitute for those measurements.

## Measured memory acceptance

`scripts/check_compile_memory_budget.py` compares reports from the authoritative
profiler without running Cargo. `ci/compile_memory_budgets.json` pins complete
baseline report, input and measurement digests. Four surfaces have measured
baselines; Core/Torii tests and daemon release remain explicitly pending because
their frozen builds fail. A pending surface cannot pass complete qualification.

For each retained package/target/source/features/profile identity, the exact
baseline compiler peak is its candidate byte limit. Cargo may compile that
identity through more than one dependency graph; the checker preserves the
invocation count and uses the largest measured peak. Every invocation remains
bound by the profiler's measurement digest. The comparison emits the concrete
byte limit and observed peak for every identity. It rejects cached/missing
measurements, failed builds, unstable seals and unreviewed new units. New
identities need explicit byte limits and compiler controls in `introduced_units`;
these cannot override existing limits. Removed dependencies are reported.

All four model packages must also stay below 75% of the surface's heavy-model
baseline, including newly introduced model crates. The ordinary model surface
therefore requires at most **9,956,524,032 bytes** per model compilation, across
all target kinds. Every release compiler invocation, including successful and
legitimate negative capability probes, retains the **13,958,643,712-byte
(13-GiB)** ceiling. Probes keep their own source identity and are not assigned
invented Cargo units. Model target optimization must remain unchanged; build
scripts retain their separate compiler profile. The comparator also checks
codegen options, cfgs and sysroot settings absent from Cargo's profile fields,
including rustc's long and shorthand option spellings.
Changing optimization requires separate memory/runtime qualification and a
reviewed measurement contract; new-unit limits cannot silently authorize it.

Run both profiles on the policy's pinned runner with the same toolchain,
profiler/helper, interpreter, cache inputs, Cargo selection, environment and job
count. Private toolchain directory locations may differ; their executable and
launcher bytes and version identities must match. The report records platform
identity, but does not attest physical runner identity: the scheduling workflow
must enforce that and supply the runner name. Candidate report hashes must come
from the trusted artifact manifest. Digest validation is not a replacement for
signed release provenance.

For a focused comparison:

```sh
python3 scripts/check_compile_memory_budget.py --surface model \
  --baseline /profiles/baseline/model.json \
  --candidate /profiles/candidate/model.json \
  --candidate-sha256 "$IROHA_CANDIDATE_MODEL_REPORT_SHA256" \
  --runner architecture-macos-arm64-128g-20260906 > /profiles/model-comparison.json
```

Complete qualification uses `--suite /profiles/candidate-suite.json`. Its sole
schema contains `schema_version: 1`, `runner`, `candidate` and `reports`.
`candidate` declares the expected `source_sha256`, `execution_source_sha256`,
`cargo_lock_sha256` and `git_revision`. `reports` maps every surface name in the
policy to `baseline`, `candidate` and `candidate_sha256`; paths are relative to
the suite manifest or absolute. All candidate reports must bind the same source,
execution snapshot, lockfile and revision. Missing/pending surfaces and invalid
reports return exit 2; any measured budget violation returns exit 1. Only a
complete passing comparison returns exit 0. Focused comparisons qualify only
their named surface.

The 43 focused tests use synthetic reports for acceptance logic. Separately,
all four real measured baselines have been passed through the checker as
negative candidates and correctly fail the model/release limits. No measured
candidate improvement or full architecture qualification is claimed. Pinned-runner
CI execution, failed baseline repair and comparable candidate profiles remain
required.
