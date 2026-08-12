# CI Helpers

This directory hosts the developer-facing shell helpers that gate CI jobs
(`ci/check_*.sh`). Most scripts assume the default Cargo artifact layout under
`target/`, so keep that layout unchanged unless the entire toolchain is updated
as part of a coordinated migration.

## Affected Rust lanes

`ci/rust_lanes.toml` assigns every Cargo workspace package to exactly one
validation lane. `scripts/rust_ci.py` reads locked Cargo metadata, identifies
the package that owns each changed path, and expands that seed through the
workspace reverse-dependency graph. The PR workflow runs locked Clippy, build,
test, and documentation commands for the resulting package sets, then exposes
one required aggregate result.

The router fails closed to all lanes for unknown paths, deleted packages,
ambiguous mappings, and shared build inputs. Adding or removing a workspace
member therefore requires an explicit lane-manifest update. The release
workflow continues to run the complete workspace matrix on `main`, tags, and
manual release gates. Apply the `ci/full` label to a pull request to force all
seven routed Rust lanes before merge.

Use the same routing locally:

```sh
scripts/dev_workflow.sh
scripts/dev_workflow.sh --base origin/main
scripts/dev_workflow.sh --full
python3 scripts/rust_ci.py validate
```

## Compile-unit ratchet

The affected execution lane and the full-workspace test gate both run
`scripts/check_compile_unit_budget.py` before their ordinary tests. The guard
compiles the `iroha_data_model` library test graph with the locked dependency
set, writes JSON evidence under `target/ci/`, and enforces the checked-in
`ci/compile_unit_baselines.json` entry with a 2% or three-unit growth allowance,
whichever is larger.

The enforced count deliberately includes only Cargo workspace-member artifacts.
Registry dependencies can legitimately differ across host targets, so using the
all-artifact count as one cross-platform baseline would be unstable. The
workspace-only, library-target scope measures the internal crate graph that the
structural simplification is intended to control. Run the exact CI command when
intentionally refreshing the baseline, review the JSON evidence, and never
raise the baseline merely to make an unexplained regression pass.

Schema-v2 evidence identifies a compile unit by package, target kind and crate
types, source path, sorted features, and the complete Cargo profile, and fails
closed when any identity field is missing. A baseline is usable only when its
scope, manifest, packages, target selection, workspace/locked mode, budget
policy, and exact Rust release match the current command. The PR affected lane
therefore runs the guard with Rust 1.93.1.

## Focused dependency-graph ratchet

`python3 scripts/check_dependency_budget.py` enforces the exact no-growth
limits in `ci/dependency_budget.json`. The checked-in scopes cover source
graphs rooted at the shipping crates `iroha_data_model`, `irohad`, and
`iroha_cli`, plus a whole-workspace/all-targets scope whose roots include
development dependencies. CI runs this source-only check before classifying
affected Rust lanes, so it does not fetch crates, invoke Cargo, depend on the
host target, or rewrite `Cargo.lock`.

The ratchet resolves workspace inheritance, local path dependencies, and root
path patches directly from the Cargo manifests. “Required” metrics count
non-optional dependency declarations; “declared” metrics include optional
declarations too. Both include normal, build, and target-specific declarations
as a cross-platform upper bound. Development dependencies are included only
for configured roots, matching Cargo's rule that dependency crates do not
contribute their own dev graph. The limits cover local/workspace/path package
counts, unique external package names, and manifest dependency edges. They are
deliberately described as a reproducible source graph: use the compile-unit
guard or an actual Cargo profile when compiler-unit or fully resolved
registry-package evidence is needed.

After an intentional dependency reduction, refresh the exact limits with:

```sh
python3 scripts/check_dependency_budget.py \
  --config ci/dependency_budget.json \
  --write-baseline
```

Review the resulting diff and the content-derived manifest fingerprint. Never
raise a limit merely to accept unexplained growth; reductions pass the existing
ceiling and should ratchet it downward in the same change. Required UI/media
stacks listed in `denied_required_packages` cannot be blessed by a refresh.
Any manifest-fingerprint drift fails closed until that dependency change and
the refreshed exact limits are reviewed together.

For diagnostic comparison with a Cargo-resolved graph, opt in explicitly. The
command remains locked unless `--allow-lock-update` is provided:

```sh
python3 scripts/check_dependency_budget.py \
  --resolved -p iroha_data_model \
  --max-total-packages <reviewed-limit>
```

## Repository structure ratchets

Six fast, read-only checks keep structural and provisioning debt from returning:

- `python3 scripts/check_source_file_budget.py` caps production and test source
  files across the complete non-ignored candidate tree, including files not
  yet staged, and applies an exact no-growth ratchet to legacy files that are
  still above the limit. Intentional splits should lower
  `ci/source_file_budget.json`; unexplained growth must not refresh it. The
  checked-in `aggregate_rust` section pins the reviewed first-party Rust
  baseline, a ceiling requiring at least a 10% reduction, and a lower working
  target. Until that objective is reached, `ratchet_ceiling` is the exact
  no-growth cap enforced by CI; JSON reports say whether the objective is met
  and expose the remaining gap. The ratchet may only move downward during the
  transition, and must converge to the hard ceiling. `--write-baseline`
  preserves all reviewed aggregate targets rather than redefining them from
  the current tree.

The aggregate baseline is the task-start tree at
`cd05eebfc07c9742734b9d684394c4fe89cdb7c5`: 5,067,263 logical Rust lines.
The checker counts tracked and non-ignored untracked regular `*.rs` files with
UTF-8 `splitlines()`, excluding only the checked-in `excluded_prefixes`. The
hard ceiling is 4,560,536 lines (`floor(0.90 * 5,067,263)`), and the 4,500,000
working target leaves review headroom below it. These values are provenance,
not a baseline that may be regenerated from a later candidate.
- `python3 scripts/check_compile_time_table_assets.py` verifies the exact size
  and SHA-256 of the versioned binary tables decoded into Rust constants,
  reconstructs every removed declaration from its pinned Git preimage, rejects
  stray binary files, and requires exactly one fixed-size `include_bytes!`
  consumer per asset.
- `python3 scripts/check_cargo_feature_hygiene.py` rejects workspace-wide
  feature injection and implicit default-feature ownership across every
  workspace member. Capability bundles belong to the crate or binary that
  consumes them.
- `python3 scripts/check_workspace_target_inventory.py` keeps ordinary
  workspace builds limited to the first-release shipping executables. Fixture
  generators, probes, benchmarks, and evidence tools require explicit opt-in.
- `python3 scripts/check_generated_artifacts.py` validates
  `generated-files.toml`, requires reproducible ownership for checked-in
  generated source, and rejects tracked build, cache, package, and `dist`
  outputs.
- `python3 scripts/check_nexus_provisioning_templates.py` rejects runtime
  signing keys in production/default Nexus and Taira templates, requires
  dedicated `/run/secrets/iroha` file handles, and checks paired client/server
  exact-network identities.

Their focused regression tests live under `scripts/tests/` and
`pytests/scripts/`; the PR classifier runs them before selecting Rust lanes.

## Reproducible Cargo profiling

Use `scripts/profile_cargo_build.py` to compare compiler work without changing
the repository-local `target/` layout used by CI. The profiler requires both
its target directory and report path to be outside the checkout, adds
`--locked`, Cargo JSON messages, timing output, and a deterministic job count,
then records the source, lockfile, toolchain, environment, and compiled-unit
fingerprints alongside wall-clock and process-resource measurements.

After Cargo exits, the profiler re-captures the source/HEAD, `Cargo.lock`,
toolchain, and environment fingerprints. Only top-level `valid: true` reports
are comparable; any drift invalidates the report and makes an otherwise
successful profile exit with status 3.

For a cold profile, start with an absent or empty target directory:

```sh
python3 scripts/profile_cargo_build.py \
  --target-dir /tmp/iroha-profile-target \
  --out /tmp/iroha-profile/cold.json \
  -- build --workspace
```

Warm profiles require an explicit `--reuse-target` so cached work cannot be
mistaken for a cold measurement. Keep the emitted JSON report, JSONL Cargo
message stream, stderr log, and Cargo timing HTML together when comparing two
revisions. A comparison is meaningful only when the report input fingerprints
and Cargo arguments identify the intended source/toolchain change.

### Featured checks
- `check_rust_1_92_lints.sh` – runs `cargo check` with the Rust 1.92 lint set (including the new never-type fallback and macro-export checks) so stricter diagnostics surface before CI.
- `check_nexus_cross_dataspace_localnet.sh` – runs the Nexus 12-peer cross-dataspace proof on ten fresh deterministic seeds (`nexus-cross-dataspace-v1-seed-00` through `-09`). Each seed is a separate network/test process with no retry, and the launcher rejects missing or zero-test transcripts before publishing exact 10/10 completion accounting. Production release also invokes the launcher's ignored `--cross-dataspace-fault-soak` path, whose validated duration is exactly 7,200 seconds.
- `check_sumeragi_v2_multilane_release_inventory.sh` – statically pins the exact autoscale A/B/A and rotating-validator Native AMX four-peer test names, requires ordinary test attributes without `#[ignore]`, and verifies that the production release runner invokes their mandatory zero-skip launcher.
- `check_sumeragi_formal.sh` – runs the fail-closed serialized Sumeragi v2
  release gate. It validates the proof ledger, runs every deductive module
  with the pinned TLAPM backends and fingerprints disabled, and then validates
  fresh source- and log-bound proof evidence before any bounded checks.
  The remaining stages run pinned TLA2Tools counterexample searches, normalize
  and replay the checked-in TLC witness against the production reducer, run
  the exact seven-test fast network-simulation inventory, and verify the
  source-linked production core with pinned Verus and `--no-cheating`. The
  nightly workflow additionally runs the sole ignored test, the 100,000-height
  chaos simulation, through
  `scripts/formal/run_sumeragi_v2_harness.sh`. The retired Sumeragi v1
  Apalache and expected-failure corridors are not release evidence.
- `check_swift_spm_validation.sh` – exercises `IrohaSwift/Package.swift` with the bridge present and with the bridge intentionally missing. The complete artifact must build and the missing-artifact case must fail with the mandatory-bridge diagnostic. Writes a summary + logs under `artifacts/swift_spm_validation`.
- `check_swift_pod_bridge.sh` – requires CocoaPods and runs strict Release
  `pod lib lint` against `IrohaSwift/IrohaSwift.podspec` while an authenticated
  `NoritoBridge.xcframework` is present. Missing tooling or artifacts fail the
  lane. This is a structural lint only; native CocoaPods delivery remains
  blocked until the podspec has a reviewed vendored-XCFramework archive path
  and downstream install evidence.
- `check_walletless_follow_bundle.sh` – repackages the walletless follow-game static bundle and asserts the tarball + `.sha256` sidecar exist. Use this in CI before publishing via the content lane workflow.

## Cargo `build-dir` decision

Rust 1.91 stabilised the `[build] build-dir` option, which allows relocating
`target/`. The workspace baseline is now Rust 1.92, but we audited the CI
wrappers and decided **not** to override this
setting:

- `ci/check_sorafs_fixtures.sh` exports `target/go-cache`, `target/go-mod-cache`,
  and other Go workdirs when it runs the cross-language chunker suite
  (`ci/check_sorafs_fixtures.sh:72-85`). Moving the build directory would break
  those cache paths as well as the `TMPDIR` wiring that assumes they live inside
  the repository.
- `ci/check_norito_enum_bench.sh` writes Criterion artefacts to
  `${ROOT_DIR}/target/criterion` so downstream tooling can scrape the JSON/HTML
  reports without extra configuration (`ci/check_norito_enum_bench.sh:6-27`).

Other scripts (Swift dashboards, Android docs, etc.) stream intermediate files
into `target/` for the same reason: shared caches and human-readable locations.
To keep CI deterministic, **do not** set `[build] build-dir` in
`.cargo/config.toml` and avoid committing `CARGO_TARGET_DIR` overrides. If you
need a custom build directory for local experimentation, export
`CARGO_TARGET_DIR` in your shell session but reset it before running any
`ci/check_*` script.
