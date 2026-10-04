# iroha_plonk

The Iroha-native PIPA-v1 PLONKish/IPA proof system (`specs/plonk_ipa_v1.md`),
built on `iroha_pasta`. The vendored halo2 stack is a test oracle only
(`crates/iroha_plonk_oracle`); this crate never depends on it.

Stage ENGINE-1 (tasks T8 and T9) provides:

- `cs`: the constraint-system IR. Expressions, gates, halo2 permuted lookups,
  the permutation argument with the vendored union-find copy assembly, the
  exact port of halo2 selector compression, and `CircuitDescriptorV1`, the
  canonical Norito frame of everything a verifier evaluates, with its
  `BLAKE2b` digest, `transcript_repr` and the `PINNED_PARAMS_V1` table.
- `frontend`: `Circuit`, `Layouter`, `Region`, `Value`, `Assigned`, the
  halo2-axiom `SimpleFloorPlanner` layout (every region starts at row 0) and
  the `Assembly` that records fixed values, selectors, copies and witnesses.
- `check`: the constraint checker. It interprets the uncompressed source
  expressions and reports cell-level diagnostics. Strict mode also flags reads
  of unassigned advice, blinding rows and wrapped rotations, and unassigned
  copied cells; `Halo2Compatible` mode matches the halo2-axiom `MockProver`.

Stage ENGINE-2 (tasks T10, T11 and the PCS half of T13) provides:

- `keys`: `VerifyingKey` with the vendored `0x02` byte layout and a strict
  reader bound to a `DescriptorBinding` (exact `k`, compress flag and
  fixed-column count, canonical non-identity points, zero bitmap padding, no
  trailing bytes, and the selector-compression registration rule);
  `transcript_repr` from the descriptor digest and the VK bytes; `ProvingKey`
  with the fixed and `sigma` columns, the masks, the exact quotient cosets
  (`QuotientDomain`: `d - 1` cosets and a small Vandermonde recombination) and
  the fixed-coset cache and the digest of its copy mapping; deterministic key
  generation from a circuit or from explicit assignment tables (public API
  with no soundness effect; imported tables may not touch rows at or beyond
  the usable rows), with the permutation cycles built exactly as halo2 builds
  them.
- `transcript`: the `BLAKE2b` `Challenge255` transcript (byte-identical to
  the vendored `Blake2bWrite`/`Blake2bRead`) and the KAGEMUSHA RP57 Poseidon
  transcript (snark-verifier `NativeLoader` semantics, injective point
  absorption in production), canonical message decoding, trailing-byte
  rejection and the instance-frame prelude.
- `pcs::ipa`: commitments, the BGH19 inner-product argument (the prover
  returns the folded generator `G'_0`), succinct accumulation into a
  `#[must_use]` `PendingAccumulator` (`PendingOpening::accumulate`, never a
  verdict), the `AccumulatorV1` codec, `decide` and the deterministic
  `BLAKE2b`-weighted `batch_decide`, `PinnedParams` (parameters derived here
  or matching the pinned digest), and `msm_complete`, the portable
  complete-formula Pippenger that every verifier-side MSM uses (S10).
- `pcs::multiopen`: the halo2 multi-point opening with static query grouping:
  queries are grouped by slot (column kind and index), never by commitment
  value, and a repeated query must repeat its evaluation bit for bit.

Stage ENGINE-3 (tasks T12 and T13) provides:

- `protocol`: the tables the prover and verifier share, derived from the
  descriptor alone: the opening queries of spec 9.1 and their static plan,
  the exact proof length, the Lagrange and Direct-instance evaluations, the
  explicit-stack expression evaluator, the S7 zero-knowledge budget
  computed from the opening plan, and (S11) the constraint-term table that
  the verifier's fold interprets and the transcript schedule that the prover
  and the verifier are tested against operation for operation.
- `prover`: `create_proof` and `prove_circuit`. Randomness comes only from an
  opaque `ProverRandomness` (OS-keyed ChaCha20, a hedged derivation over OS
  entropy, the statement digest and the witness digest, or a recovery stream:
  the prover draws 32 bytes from the caller's derivation and keys ChaCha20
  itself with them and the witness and statement context, so a derivation
  cannot opt out of the binding) and is drawn in the `BlindingScheduleV1`
  order. The quotient is evaluated on exactly `d - 1` cosets by a compiled,
  hash-consed expression DAG and recombined with a Vandermonde solve.
  `Witness::from_circuit` checks that a circuit matches its key (copies
  through the key's copy digest, in constant memory) and moves the
  synthesized advice into the zeroizing witness; `Witness::from_columns`
  imports assignment tables (public, no soundness effect).
- `verifier`: `verify_full`, `accumulate_succinct` (a `#[must_use]` pending
  accumulator; its `Ok` is satisfiable for false statements until decided,
  so it is never a verdict), `verify_full_from_bytes` and `batch_verify`
  (deterministic weights, one merged `g` MSM, suffixes as accumulator
  items). Rejections are typed `VerifyError`s.

In oracle mode (`--cfg iroha_plonk_oracle`) `create_proof_oracle` reproduces
vendored halo2-axiom proof bytes, and `verify_full_oracle` accepts vendored
proofs (checked for Committed and Direct instances on both curves, with and
without selector compression).

Rules this crate enforces:

- queries are interned in halo2's order;
- descriptors reject repeated `(column, rotation)` queries and rotations that
  collide modulo `n`;
- instance columns declare their exact lengths in `configure`, and the
  transcript prelude frames the instance shape;
- params digests are pinned per `(curve, k)`; verifiers accept only derived
  or pinned parameters;
- the zero-knowledge query budget (S7) is checked;
- opening queries are grouped statically (S1) and never overwritten (S3);
- every proof message decodes canonically, the identity is never absorbed,
  the proof length is exact, `x = 0` and `x^n = 1` are rejected, instance
  column counts and lengths are exact in both instance modes, and only
  `verify_full`, `batch_verify`, `decide` and `batch_decide` accept;
- verifier MSMs use complete formulas only; memory budgets change speed,
  never verdicts;
- provers draw randomness only from `ProverRandomness`; fixed seeds exist
  only in unit tests and oracle builds;
- key generation refuses imported tables that copy, enable a selector or set
  a fixed value at or beyond the usable rows;
- nothing panics on misuse: errors are typed.

Oracle mode (the injected vendored `transcript_repr`, `fe_to_fe` Poseidon
point absorption and caller-seeded prover randomness) is compiled only with
`--cfg iroha_plonk_oracle` (and in this crate's unit tests); it is never a
Cargo feature. `build.rs` only declares the cfg for `check-cfg`. The oracle
run is manual today (TODO: a CI job); `ORACLE_BUILD` reports the cfg, and
every shipping root that links this crate asserts `!ORACLE_BUILD` at compile
time.

Every output is a pure function of its inputs and the prover's random stream;
results do not depend on the Rayon pool size, and no behaviour comes from
environment variables.

Validate:

```sh
cargo test -p iroha_plonk
cargo test -p iroha_plonk --release --test pinned_params -- --include-ignored
cargo test -p iroha_plonk --release --lib measure_prove_and_verify -- --ignored --nocapture
RUSTFLAGS="--cfg iroha_plonk_oracle" cargo test -p iroha_plonk --lib  # own target dir
cargo test -p iroha_plonk_oracle --test plonk_cs_parity
```
