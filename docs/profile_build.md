# Profiling Rust builds

Use `scripts/profile_cargo_build.py` for reproducible build measurements. It
keeps both Cargo artifacts and its report outside the checkout, so profiling
cannot delete or contaminate the normal repository-local Cargo cache:

```sh
profile_dir="$(mktemp -d "${TMPDIR:-/tmp}/iroha-build-profile.XXXXXX")"
python3 scripts/profile_cargo_build.py \
  --target-dir "$profile_dir/target" \
  --out "$profile_dir/data-model.json" \
  -- build -p iroha_data_model --lib
```

Replace the Cargo arguments after `--` to measure another surface. Common
equivalents are `build --workspace`, `build -p irohad --bin iroha3d`, and
`build -p iroha_cli --bin iroha`. The profiler adds Cargo's `--locked`, JSON
message output, timing output, and a deterministic job count when the caller
does not provide those controls. The default is one job; pass `--jobs N` to
compare a deliberately wider build lane.

The target directory and `--out` path must be outside the repository. The
target must be absent or empty for a cold measurement; pass `--reuse-target`
explicitly for a warm or no-op measurement. The profiler never runs
`cargo clean`.

The JSON report binds the Cargo arguments, tracked and non-ignored untracked
source tree (including tracked deletions), Git revision, `Cargo.lock`, selected
build environment, and verbose Cargo/Rust compiler identities. Those inputs
are captured again after Cargo exits. If any input changes during the build,
`input_validation.stable` and top-level `valid` are false and an otherwise
successful invocation exits with status 3. Never compare a report unless
`valid` is true.

Keep the report together with its adjacent `.jsonl` Cargo message stream,
`.stderr.log`, and the Cargo timing HTML below the target directory. Compare
reports only when their input manifests differ by the intended change and the
host characteristics are suitable for the metric being compared.

`scripts/profile_build.sh` and `scripts/profile_build.py` remain as legacy
compatibility entry points for callers using the named `data-model`, `daemon`,
`cli`, and `workspace` scenarios. New automation and retained profiling
evidence should use `scripts/profile_cargo_build.py`, whose source and
toolchain fingerprints and post-run drift check are authoritative.
