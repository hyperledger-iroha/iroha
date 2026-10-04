# PLONK/IPA differential oracle

`iroha_plonk_oracle` is temporary test infrastructure. It is the only crate
that depends on both the vendored halo2 stack (`vendor/halo2-axiom`,
`vendor/halo2curves-axiom`, `vendor/halo2-base` and the pinned `snark-verifier`
revision) and the Iroha-native PLONK/IPA crates (`iroha_pasta`, and later
`iroha_plonk`). It checks that the native implementation reproduces the vendored
one byte for byte while consumers migrate.

Rules:

- `publish = false`. Use the crate only as a dev-dependency. No shipping crate
  may depend on it, and it is not a default workspace member.
- `cargo test -p iroha_plonk_oracle --no-default-features` skips
  `vendored_goldens` (it requires the `circuit-params` feature) instead of
  failing to compile.
- It never edits `vendor/`. The vendored goldens
  (`vendor/halo2-axiom/tests/golden_proof_bytes.rs`) are compiled here through
  a `#[path]` include, which gives them a workspace runner.
- The vendored crates resolve through the workspace `[patch]` tables with the
  same revisions and features as `iroha_core_zk`.
- The crate, the vendored halo2 stack and the git dependencies are deleted
  together once every consumer has migrated to the native crates.

Milestone M0, contract capture (in `tests/`):

- `vendored_goldens.rs`: a `#[path]` include (with `#[rustfmt::skip]`) of
  `vendor/halo2-axiom/tests/golden_proof_bytes.rs`. It runs the sigma (k = 6,
  9, and 11 ignored) and wide (k = 8, 10) cases on both cycles. Each proof is
  repeated in Rayon pools of 1, 2, 4 and 7 threads.
- `native_prover_kats.rs`: generates and checks
  `fixtures/native_prover/kats_v1.json` (see `fixtures/native_prover/README.md`).
  By default it compares params for k = 6..=14; k = 15 and 16 are an ignored
  release test. `IROHA_UPDATE_NATIVE_PROVER_KATS=1` rewrites the file; run that
  in release because it includes k = 15 and 16. It reads the KAGEMUSHA golden
  table from `crates/iroha_core_zk/src/prover_golden_tests.rs` at run time;
  that table should move into the fixture (TODO for the `iroha_core_zk`
  owner). The KAGEMUSHA hash vectors have no production anchor in
  `iroha_core_zk` yet (also a TODO there); the confidential ones do.
- Oracle baseline: repository HEAD `1de7210a74d6`; last `vendor/halo2-axiom`
  commit `8f41274044c9`. Full hashes are in the fixture's `oracle_baseline`
  section and in `specs/native_prover_migration_inventory.md`.

Library modules (`src/`):

- `convert`: canonical conversions between the vendored Pasta types and the
  `iroha_pasta` types, per half of the cycle (`Vesta`, `Pallas`). Every
  conversion panics if the other side rejects the encoding.
- `pools`: shared Rayon pools of 1, 2, 4 and 7 threads; `same_on_each_pool`
  runs a computation in each and requires identical results.
- `vendored`: `generator_collapse`, a statement-for-statement reproduction of
  the crate-private vendored IPA fold, and `Recording`, a transcript wrapper
  that records every absorbed element, proof element and challenge.

Milestone M1.a (engine parity), `iroha_pasta` part (`tests/pasta_parity/`).
Milestone labels are defined in `specs/native_prover_migration_inventory.md`.
Every comparison
runs the vendored and the native code in each pool, requires the native result
to be pool independent, and requires byte equality with the vendored result:

- `encoding`: field constants, decoding verdicts on boundary encodings,
  arithmetic, square roots, bits, wide reduction and seeded sampling; point
  encodings, decoding verdicts (boundary and 2,000 random strings per pool),
  coordinates, `from_xy`, the group law, scalar multiplication, the
  endomorphism, batch normalisation and hash-to-curve; the halo2-axiom
  `Processed` and `RawBytes` serde formats. Reference: the
  `halo2curves-axiom` Pasta types.
- `params`: `ParamsIPA` bytes for k = 6..=14 on both curves (k = 15 and 16
  ignored), codec round trips, the stricter native decoding (identity points
  and trailing bytes), the `kats_v1.json` digests, `downsize` and commitments.
- `msm`: `msm_public`, `msm_secret` and `FixedBaseTable` against
  `best_multiexp` on sizes 0..=2^14 (2^15 and 2^16 ignored), 13 scalar kinds
  and 7 adversarial base shapes, and tight memory budgets.
- `fold`: per-round folds against the vendored collapse on special
  challenges and exceptional lanes, and full folds over real vendored IPA
  proofs for k = 1..=10 (11..=16 ignored), whose native `G'_0` must equal
  `GuardIPA::compute_g` and satisfy the vendored verifier.
- `fft`: FFT and IFFT against `EvaluationDomain` and all four vendored FFT
  backends for k = 1..=14 (15 and 16 ignored), and coset transforms
  (`coeff_to_extended`, `coeff_to_extended_part`, `extended_to_coeff`).
- `poseidon`: RP57 tables against halo2-base, the sponge against
  `snark-verifier`, and the fixture's native hash and transcript vectors.

`tests/kernel_benchmarks.rs` holds ignored release microbenchmarks (MSM
2^10..=2^18, the full IPA fold for k = 11..=16, FFT/IFFT/coset for
k = 11..=16, at 1 and 4 threads) that time the native and vendored kernels side
by side and print Markdown rows with wall time, process CPU time and load.

Milestone M1.a, `iroha_plonk` constraint-system part (tasks T8/T9),
`tests/plonk_cs_parity.rs` (`iroha_plonk` is a dev-dependency):

- `cs_parity_*`: 600 seeded random constraint systems per curve plus a dense
  selector case are configured through both APIs with the same calls. Degrees,
  blinding factors, query tables (interning order), permutation columns and
  every gate and lookup expression, node for node, must be equal. After
  `compress_selectors` and `directly_convert_selectors_to_fixed` on the same
  activations, so must the selector columns and the substituted expressions.
- `layout_parity_*`: one circuit with tables, constants, copies,
  `copy_advice`, instance copies, rational fixed values and selectors. Its
  vendored `MockProver` fixed columns (with the selector columns) and copy
  permutation must equal the native `SimpleFloorPlanner` assembly.

Planned contents (the rest of M1.a, engine parity, task T15):

- `export`: vendored `ConstraintSystem` export (compressed selectors, fixed,
  advice and instance tables, copy constraints, `transcript_repr`) into the
  native IR.
- Native re-proving of every vendored golden case at 1, 2, 4 and 7 threads.
- Verifying-key byte parity, and verifier verdict parity on the sigma-shaped
  and wide tamper corpora.

Validate:

```sh
cargo test -p iroha_plonk_oracle
# k = 11 goldens, k = 15 and 16 params and the large parity cases:
cargo test --release -p iroha_plonk_oracle --lib --test vendored_goldens \
    --test native_prover_kats --test pasta_parity -- --include-ignored
python3 fixtures/native_prover/verify_kats_v1.py
# Benchmarks (ignored tests; an idle host gives meaningful numbers):
cargo test --release -p iroha_plonk_oracle --test kernel_benchmarks -- \
    --ignored --nocapture --test-threads=1
```
