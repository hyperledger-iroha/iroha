# PLONK/IPA differential oracle

`iroha_plonk_oracle` is temporary test infrastructure. It is the only crate
that depends on both the vendored halo2 stack (`vendor/halo2-axiom`,
`vendor/halo2curves-axiom`, `vendor/halo2-base` and the pinned `snark-verifier`
revision) and the Iroha-native PLONK/IPA crates (`iroha_pasta` and
`iroha_plonk`). It checks that the native implementation reproduces the vendored
one byte for byte while consumers migrate.

Rules:

- `publish = false`. Use the crate only as a dev-dependency. No shipping crate
  may depend on it, and it is not a default workspace member.
- `cargo test -p iroha_plonk_oracle --no-default-features` skips
  `vendored_goldens` (it requires the `circuit-params` feature) instead of
  failing to compile.
- It never edits `vendor/`. The vendored goldens
  (`vendor/halo2-axiom/tests/golden_proof_bytes.rs`) are compiled here, which
  gives them a workspace runner: `build.rs` copies the file into `OUT_DIR` with
  its `//!` lines turned into `//` (the only change, since `include!` rejects
  inner doc comments), and `tests/vendored_goldens.rs` includes the copy.
- Oracle mode of `iroha_plonk` (`create_proof_oracle`, `verify_full_oracle`,
  fixed prover seeds) exists only with `--cfg iroha_plonk_oracle` in
  `RUSTFLAGS`, never as a Cargo feature (spec section 6.4). Tests that need it
  are compiled only under that cfg (`build.rs` declares it for check-cfg); run
  them in a separate target directory, because changing `RUSTFLAGS` rebuilds
  everything. The path-filtered `native_prover_parity.yml` job runs all five
  release correctness harnesses on native x86_64 and aarch64 runners through
  `ci/native_prover_oracle.py`. It includes ignored large cases, excludes only
  timing tests, and rejects absent oracle-mode tests or partial results.
  Compiler messages, source hashes, executable hashes and natural test outcomes
  are retained. This is correctness evidence, not a timing or phone gate. The
  affected-lane runner (`ci/rust_lanes.toml`, `scripts/rust_ci.py`) also runs
  ordinary tests. Hosted execution of the new parity job remains unobserved.
- The vendored crates resolve through the workspace `[patch]` tables with the
  pinned revisions and features captured by the oracle. Shipping consumers
  have migrated to the native crates and must not reach this dependency graph.
- The crate, the vendored halo2 stack and the git dependencies are deleted
  together once every consumer has migrated to the native crates.

Milestone M0, contract capture (in `tests/`):

- `vendored_goldens.rs`: the include of
  `vendor/halo2-axiom/tests/golden_proof_bytes.rs` (see above). It runs the
  sigma (k = 6, 9, and 11 ignored) and wide (k = 8, 10) cases on both cycles.
  Each proof is repeated in Rayon pools of 1, 2, 4 and 7 threads. The same
  target hosts the native parity suites (milestones M1b and M1c, below).
- `native_prover_kats.rs`: generates and checks
  `fixtures/native_prover/kats_v1.json` (see `fixtures/native_prover/README.md`).
  By default it compares params for k = 6..=14; k = 15 and 16 are an ignored
  release test. `IROHA_UPDATE_NATIVE_PROVER_KATS=1` rewrites the file; run that
  in release because it includes k = 15 and 16. The fixture is the pinned
  authority for the KAGEMUSHA golden table (`golden_proofs.iroha_core_zk_kagemusha`):
  nothing regenerates it, so the test carries those eight digests over
  unchanged (also when rewriting) and checks only their case names and digest
  format. It reads no other crate's sources. The KAGEMUSHA domain-hash
  vectors are asserted by their native consumers (`iroha_pasta`,
  `iroha_plonk_gadgets`, `iroha_kagemusha_proof`); the confidential ones are
  anchored to `iroha_core_zk` KATs.
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
- `export` (task T15): the export of a vendored circuit into the `iroha_plonk`
  IR. `export_constraint_system` replays the configure-time vendored
  `ConstraintSystem` natively (columns, simple and complex selectors, gates
  node for node, lookups, equality columns, constants, minimum degree) with
  the vendored query tables in their exact interning order. `export_circuit`
  runs the vendored floor planner twice into a capturing implementation of
  the vendored `Assignment` trait: on `circuit.without_witnesses()` for the
  fixed values, selector activations and ordered copy constraints (as the
  vendored keygen `Assembly` sees them), and on the circuit for the advice
  values (as the vendored `WitnessCollection` sees them). `ExportedCircuit`
  replays the copies into a native `PermutationAssembly` and generates the
  native keys (`keygen_from_tables`; native selector compression);
  `compare_constraint_systems` checks the result node for node against the
  vendored compressed `VerifyingKey::cs`; `vendored_transcript_repr` and
  `vendored_keygen_config(vk, transcript, suffix)` carry the vendored
  `transcript_repr` and selector choice for a proving path (the Blake2b
  golden path, or the KAGEMUSHA path with the Poseidon transcript and the
  `FoldedGenerator` suffix). The vendored `Assignment::assign_advice` returns
  a reference that outlives the backend (the vendored backends use
  `unsafe`); this crate forbids `unsafe`, so the witness pass stores values
  in one leaked arena of `n` write-once slots per assigned advice column
  (plus a box for a reassigned cell), and the golden tests cache each setup
  so every golden is exported once per process. Unit tests export a circuit
  with selectors, both lookup kinds, constants and instance reads: its keys
  equal the vendored ones with and without selector compression and, in
  oracle builds, its native proofs equal the vendored proofs in Committed and
  Direct instance modes.

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
`tests/plonk_cs_parity.rs`:

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

Milestones M1b and M1c (engine and verifier parity, task T15), in
`tests/vendored_goldens/` over the vendored golden circuits, seeds and
constants:

- `export_parity` (every build): each golden `(family, curve, k)` is exported.
  The replayed system equals the vendored one; the native selector handling
  equals the vendored `VerifyingKey::cs`; the exported advice rows, the native
  configure-time fixed columns and the native permutation mapping equal the
  vendored `MockProver`'s, and the selector columns are checked on their own
  (`MockProver` always compresses: a compress-on native key must equal its
  whole fixed table, and uncompressed columns must hold the exported
  activations; `selector_tables_match_vendored` exercises this on a
  selector-gated circuit, since the goldens have no selectors); the native
  verifying-key bytes equal the vendored `0x02` bytes (and the strict native
  reader accepts them); native keygen (VK bytes, copy digest, a SHA-256 over
  every fixed and `sigma` polynomial and coset) and parameter derivation are
  equal in pools of 1, 2, 4 and 7 threads; the native parameter bytes equal
  the vendored `ParamsIPA` bytes and the pinned digest.
- `deviation_registry` (every build): every `DEV-xx` row of spec section 14
  names a native test that exists, is a `#[test]` and mentions the row, and
  every verdict-corpus deviation names a `both`-mode row.
- `kagemusha_vendored` (every build): the vendored KAGEMUSHA path as
  `iroha_core_zk` runs it (snark-verifier `PoseidonTranscript<C, NativeLoader,
  _, 3, 2, 8, 57>`, the vendored `G'_0` appended, the augmented verdict), over
  the golden circuits.
- `proof_parity` (oracle builds): the native prover with the injected
  vendored `transcript_repr` re-proves all 20 golden cases (k = 11 ignored in
  debug) in pools of 1, 2, 4 and 7 threads; the bytes must not depend on the
  pool and their SHA-256 must equal the vendored constant. On a mismatch the
  first differing 32-byte message is reported. Both verifiers accept every
  golden.
- `verdict_parity` (oracle builds): structure-aware tamper corpora (419 inputs
  for sigma k = 6 and 608 for wide k = 8, per curve; at least 200 required):
  per-message bit flips, zeroed and all-ones messages, swaps, truncations,
  trailing bytes, instance changes and seeded corruptions. The native verifier
  must return the vendored verdict on every input and never panic; it never
  accepts what the vendored verifier rejects. The registered stricter
  rejections (`deviation_registry::DEVIATIONS`) are the only allowed differences,
  each with its typed native reason and its spec section 14 id, and each
  must occur in every Blake2b corpus:
  - DEV-05, trailing bytes: the vendored `Blake2bRead` ignores input after
    the last message it reads, so a proof with appended bytes verifies; the
    native verifier requires the exact proof length
    (`VerifyError::ProofLength`).
  - DEV-04, instance length: appended zero instance values leave the vendored
    committed instance unchanged and within its usable-row bound, so the
    padded statement verifies; PIPA-v1 fixes every instance length
    (`VerifyError::InstanceLength`, S4).

  The same corpora run on the KAGEMUSHA path against the vendored augmented
  verifier, where only DEV-04 occurs (the augmented verifier already requires
  the exact length).
- `kagemusha_parity` (oracle builds): each golden is keyed natively with the
  Poseidon transcript and the `FoldedGenerator` suffix and re-proved in pools
  of 1, 2, 4 and 7 threads; the bytes must equal the vendored KAGEMUSHA
  proof, and both verifiers accept both proofs. TODO(T16, `iroha_core_zk`):
  the `iroha_core_zk` KAGEMUSHA goldens themselves.
- `timing` (oracle builds, ignored): native against vendored prove time on
  the k = 11 sigma golden at 1 and 4 threads, with process CPU and load.

Validate:

```sh
cargo test -p iroha_plonk_oracle
# k = 11 goldens, k = 15 and 16 params and the large parity cases:
cargo test --release -p iroha_plonk_oracle --lib --test vendored_goldens \
    --test native_prover_kats --test pasta_parity -- --include-ignored
python3 fixtures/native_prover/verify_kats_v1.py
# Engine and verifier parity (oracle builds, own target directory; manual,
# TODO: CI job):
RUSTFLAGS="--cfg iroha_plonk_oracle" CARGO_TARGET_DIR=target/plonk-oracle \
    cargo test --release -p iroha_plonk_oracle --lib --test vendored_goldens \
    -- --include-ignored
RUSTFLAGS="--cfg iroha_plonk_oracle" CARGO_TARGET_DIR=target/plonk-oracle \
    cargo test --release -p iroha_plonk --lib
# Benchmarks (ignored tests; an idle host gives meaningful numbers):
cargo test --release -p iroha_plonk_oracle --test kernel_benchmarks -- \
    --ignored --nocapture --test-threads=1
RUSTFLAGS="--cfg iroha_plonk_oracle" cargo test --release \
    -p iroha_plonk_oracle --test vendored_goldens timing -- \
    --ignored --nocapture --test-threads=1
```

### Independent succinct-verifier corpus

The oracle-only `succinct_parity` module invokes the original
`PlonkSuccinctVerifier<IpaAs<_, Bgh19>>` on both Sigma/Wide golden families and
both Pasta curves, with two seeds per source. It compares all challenges against
the original Halo2 verifier and compares the returned G/u obligation against the
native oracle verifier, then requires both complete generator decisions. Five
mutations per case preserve original parse errors, exact group-assertion panics
and DEV-05 trailing-prefix acceptance; native verification rejects normally.

`fixtures/native_prover/succinct_v1.json` retains eight complete proof/key/input
records and their exact challenge/accumulator/mutation results. Native-only replay
in `iroha_plonk::verifier::captured_succinct_tests` remains after this crate is
removed. The path-filtered parity CI requires all four named oracle cases; this
corpus does not qualify the current operation catalog or claim parity with the
deliberately different PIPA-AS-v1 accumulation transcript.
