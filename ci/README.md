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
- `check_swift_pod_bridge.sh` – runs `pod lib lint` against `IrohaSwift/IrohaSwift.podspec` with the bundled `NoritoBridge.xcframework` to make sure pod consumers get the signed bridge and minimum platform/toolchain settings stay in sync with SPM.
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
