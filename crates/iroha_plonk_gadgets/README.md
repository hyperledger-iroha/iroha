# iroha_plonk_gadgets

Chips for the Iroha-native PIPA-v1 PLONKish engine (`iroha_plonk`,
`specs/plonk_ipa_v1.md`) on the Pasta fields of `iroha_pasta`. Stage GADGETS
(task T17).

- `poseidon::pow5`: the RP57 width-3 Pow5 permutation lane, ported from the
  M8 custom gate: three state columns and one auxiliary column, 37 rows and
  148 cells per permutation (4 full rows, 28 rows of two partial rounds, one
  partial row, 4 full rows), absorption fused into the first full round, the
  sponge output squeezed by the last round's gate. Six round-constant columns,
  shareable between lanes (blocks start at multiples of 37 rows).
- `poseidon::sponge`: the KAGEMUSHA sponge, bit for bit
  `iroha_pasta::poseidon::{hash, hash_with_domain}` and the
  `kagemusha_v1_poseidon` vectors of `fixtures/native_prover/kats_v1.json`
  (domain and arity prefix, `[x, 1]` and `[1, 0]` padding, output word 1). A configured `(domain, arity)` prefix starts from
  its constant post-prefix state and saves one permutation.
- `range::running_sum`: range checks of 1 to 252 bits with `b`-bit limbs
  against a `2^b`-row table, with a shifted top-limb row (10 rows for 128 bits
  at `b = 15`, the M8 inventory).
- `range::u128`: checked add, subtract, add-constant, `<=`, `<` and the `<`
  bit on `Uint<BITS>` cells (`U128`, `U64`); an overflow or underflow has no
  satisfying assignment.
- `arith`: the glue gate `q_m a b + q_a a + q_b b + q_c c + q_d d + q_k` with
  boolean, select and is-zero gates.
- `statement`: the G1 step statement encoding of the split-lineage step
  relations: 28 elements under `kgwstmt1`, in the order of the G1
  `KagemushaWalletStatementV1::field_items` (wire record section 3.2; not yet
  wired into a protocol path): version, the scheme-level relation identity
  (two limbs, witness cells bound by the public digest), scheme, asset,
  credential, successor lifecycle, sequence and `next_load`, the
  enabled-controls mask, the lineage inputs of a Send, both state
  commitments, the effect tag and a 10-element effect union (`credit_id` is
  one element). Its tests reproduce the G1 statement vectors of
  `fixtures/kagemusha/wallet_v1_vectors.json` natively and in circuit. Also
  the canonical cross-field limb encoding of spec S6 (`lo < 2^128`,
  `hi < 2^127`, `lo + 2^128 hi < modulus`) and the decomposition of an
  own-field word into those canonical limbs.
- `cells`: typed cells (`Word`, `Bit`, `Uint`) and row cursors; `tamper`: the
  per-cell tamper harness.

Every gate has degree at most 6 (`MAX_GATE_DEGREE`). The `iroha_plonk` floor
planner starts every region at row 0, so each chip owns its columns and a row
cursor.

Each chip has typed cells, a native reference, shared vectors, a per-cell
tamper suite (each assigned advice cell, changed alone, makes the strict
constraint checker fail) and an inventory test. The tamper suite shows that
every cell is pinned by a gate, a lookup or a copy. It does not show that a
composed relation binds what it should: a free witness that is only copied
into a hash is pinned by that copy whatever value it claims, so relations
need their own consistent-forgery tests.

- `tests/pow5_tamper.rs`: parity with `iroha_pasta::poseidon::permute`, every
  `m8_custom_gate_checks` Poseidon case, 37 rows / 148 cells / degree 6, lane
  sharing, misuse errors;
- `tests/digest_parity.rs`: the `kagemusha_v1_poseidon` vectors of
  `fixtures/native_prover/kats_v1.json` and the `iroha_pasta` raw-sponge
  known answers in circuit, folded and unfolded, on both fields; M7 hash
  shapes;
- `tests/glue_tamper.rs`, `tests/u128_tamper.rs`: every operation against its
  native reference, the M8 u128 cases (a sum of exactly `2^128`, a
  subtraction below zero, a wrong sum) and the M7 u64 windows;
- `tests/statement_digest.rs`: the in-circuit statement digest equals
  `StatementV1::digest`; non-canonical foreign limbs are unsatisfiable; an
  own-field word decomposes only into its canonical limbs;
- `tests/real_proofs.rs`: PIPA-v1 proofs on Pallas and Vesta with both
  transcripts.

Validate:

```sh
cargo test -p iroha_plonk_gadgets
cargo test -p iroha_plonk_gadgets --release -- --include-ignored
cargo clippy -p iroha_plonk_gadgets --all-targets -- -D warnings
```
