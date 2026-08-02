# Profiling Rust builds

Use the repository profiler to measure a cold or warm build without deleting
the normal Cargo cache:

```sh
profile_dir="$(mktemp -d /tmp/iroha-build-profile.XXXXXX)"
./scripts/profile_build.sh data-model \
  --target-dir "$profile_dir/target" \
  --output "$profile_dir/data-model.json"
```

The available scenarios are `data-model`, `daemon`, `cli`, and `workspace`.
Each runs Cargo with `--locked --timings` and records wall time, child CPU time,
aggregate peak resident memory for Cargo's process tree, and target-directory
bytes. Cargo's HTML report is written below the selected target directory.

The target directory is mandatory and must be empty for a cold measurement.
Pass `--reuse` explicitly for a warm or no-op measurement. The profiler never
runs `cargo clean`, so it cannot erase a developer's normal artifacts. Omit
`--jobs` to measure Cargo's native jobserver; pass `--jobs N` only when
comparing a deliberately constrained lane.

Compare results from the same toolchain, feature surface, scenario, and host.
The JSON report records those inputs and the Git revision; retain the patch or
commit identity as well when profiling a dirty worktree.
