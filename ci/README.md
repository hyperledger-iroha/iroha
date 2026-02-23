# CI Helpers

This directory hosts the developer-facing shell helpers that gate CI jobs
(`ci/check_*.sh`). Most scripts assume the default Cargo artifact layout under
`target/`, so keep that layout unchanged unless the entire toolchain is updated
as part of a coordinated migration.

### Featured checks
- `check_rust_1_92_lints.sh` – runs `cargo check` with the Rust 1.92 lint set (including the new never-type fallback and macro-export checks) so stricter diagnostics surface before CI.
- `check_nexus_cross_dataspace_localnet.sh` – runs the deterministic Nexus cross-dataspace all-or-nothing localnet proof (`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`) through `scripts/run_nexus_cross_dataspace_atomic_swap.sh`.
- `check_sumeragi_formal.sh` – runs bounded Apalache checks for the Sumeragi TLA+ model (`fast` and `deep`) via `scripts/formal/sumeragi_apalache.sh`.
- `check_swift_spm_validation.sh` – exercises `IrohaSwift/Package.swift` with the bridge present, with the bridge intentionally missing (expecting a fatal error in required mode), and with `IROHASWIFT_USE_BRIDGE=0` to ensure Swift-only builds still succeed. Writes a summary + logs under `artifacts/swift_spm_validation`.
- `check_swift_pod_bridge.sh` – runs `pod lib lint` against `IrohaSwift/IrohaSwift.podspec` with the bundled `NoritoBridge.xcframework` to make sure pod consumers get the signed bridge and minimum platform/toolchain settings stay in sync with SPM.
- `check_sorafs_gateway_denylist.sh` – generates two sample denylist bundles from the canonical fixtures, runs `cargo xtask sorafs-gateway denylist diff`, and fails the build if the report is missing or lacks additions/removals. This guards the MINFO-6 workflow so releases always have working bundle-evidence tooling.
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
