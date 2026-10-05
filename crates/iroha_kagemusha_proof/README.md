# iroha_kagemusha_proof

Native-only KAGEMUSHA relations on the PIPA-v1 engine (`iroha_plonk`,
`specs/plonk_ipa_v1.md`). This crate is the compilation boundary of KAGEMUSHA
relations built on the native stack. It links `iroha_pasta`, `iroha_plonk` and
`iroha_plonk_gadgets` only, never the vendored halo2 stack or `iroha_core_zk`.

## Prototype status

Everything here is a **prototype** of the proposed *split-lineage* design
(`specs/kagemusha_single_design_proposal.md` sections 3 and 3.2): step proofs
on the payment path, with the recursive lineage proof in the background. Owner
approval of that design is pending. The relations implement the spec section 3
core with every regulatory control off, under prototype domain labels
(`kgspcor1`, `kgspcrd1`, ...); only the public statement follows the G1
wallet statement encoding (`kgwstmt1`). They are not a protocol format, and no
protocol path uses them. Aligning the core layout, the chain entries and the
domains with the G1 encodings is open (TODO(G3) in `src/witness.rs`). A frozen relation needs versioned types, shared Swift/Kotlin
vectors and owner sign-off.

## Relations

`sigma_send` and `sigma_recv` (`SigmaCircuit`) on Pow5 sponge lanes,
running-sum range checks, checked `u128`/`u64` arithmetic and glue gates.

- **State.** A 40-field wallet state: a 30-field core plus a 10-field
  remainder.
  - The core holds every field spec section 3 assigns to it: lifecycle,
    `wallet_id` and credential digest, balance, `burned_total`, sequence, the
    send, load and redeem ordinals, both chains, the five map roots, the
    enabled-controls mask, the quota windows root, the blacklist version and
    root, the lease expiry, the policy epoch, the accepted-time floor and the
    state nonce.
  - It also holds the scheme and asset identifiers, beyond the spec's list.
    The lineage proof exposes no asset, so a step proof binds the asset of the
    incarnation only by opening it. (Spec question for the owner: add the
    asset to the core, or to the section 3.2 consumer checks.)
  - Two-level (spec): `H(core || H(remainder))`, 16 permutations per opening
    with a folded prefix. Flat: `H(core || remainder)`, 21. Each
    `(step, layout)` pair has its own relation identifier.
- **Both steps.**
  - Open the predecessor commitment and require lifecycle Active.
  - Require a nonzero `u128` amount and `sequence + 1 < 2^128`.
  - Derive `credit_id = H(kgspcrd1, Request body)` in circuit and decompose it
    into its canonical limbs. The Request's scheme, asset and own wallet (and,
    for `sigma_recv`, the receiver credential) are the opened core cells.
  - Require distinct payer and receiver wallets.
  - Commit the successor with a fresh state nonce. Roots the step updates are
    carried witnesses; the others are copied. Spec section 3.2 assigns root
    transitions to the native Advance check and to the lineage relation.
- **`sigma_send`.**
  - Takes `burned_total` and the pending-outgoing root of the predecessor's
    lineage proof as public inputs (in the statement).
  - Requires an empty enabled-controls mask.
  - Checks `amount + fee < 2^128` and `amount + fee <= balance - burned_total`.
    The successor balance is `balance - amount - fee`, and its `burned_total`
    is the lineage input.
  - Advances the send ordinal without overflow. Requires
    `request_policy_epoch <= policy_epoch` and
    `max(accepted_time_floor, request_time) <= lower <= upper` (all `u64`),
    and raises the successor's floor to `lower`.
  - Appends `send_chain` over `credit_id`, receiver, ordinal, amount and fee.
- **`sigma_recv`.**
  - Credits the amount without overflow and appends `recv_chain`.
  - Its statement contains no Payment digest (precomputed at Request
    signing).
- **Public outputs.** The Poseidon digest of the 29-field step statement
  under `kgwstmt1`, in the order of the G1
  `KagemushaWalletStatementV1::field_items` (version, the 32-byte relation
  identity as two limbs, scheme, asset, credential, successor lifecycle,
  sequence and `next_load`, mask, the lineage inputs of a Send, both
  commitments, the effect tag and an 11-field effect union; the Send effect
  carries the Request digest limbs after the fee), plus the credit identifier
  for `sigma_send`. The statement carries no
  prover-chosen other-parity commitment component.

`StepWitness::evaluate` is the native reference. It returns every digest, the
successor and the relation `Violation`s. The circuit compares every in-circuit
digest with it while the witness is known.

`consumer::check_send` and `consumer::check_receive` are the native spec
section 3.2 checks a package consumer runs before verifying the proof: the
relation identity, the predecessor, credential, scheme, mask, `burned_total`
and pending-outgoing root against the lineage proof (`LineageView`), and the
scheme, asset, wallets, amounts and credit identifier against the Request
body.

## Shapes and proofs

- `select_shape` chooses the smallest `k`, then the fewest lanes, at which a
  key-generation synthesis fits. A proof byte budget is optional; the default
  is the 3.5 KB gate. Proof lengths are exact, taken from the descriptor.
- Shapes chosen (folded prefixes):

  | Relation | Permutations | With the 3.5 KB budget | Without a budget |
  | --- | --- | --- | --- |
  | Two-level send / recv | 65 / 64 | `k = 12`, one lane, 3,296 B | `k = 10`, four lanes, 5,120 B |
  | Flat send / recv | 75 / 74 | `k = 12`, one lane, 3,296 B | `k = 10`, four lanes, 5,120 B |

  At `k = 11` the two-level relation needs two lanes, and that proof exceeds
  the budget.
- The glue chip shares the columns of the least loaded lane, in the rows after
  its last permutation block.
- `SigmaProver` derives the parameters, generates keys and proves. It refuses
  a witness that breaks the relation.
- `KeyOptions` trades memory for speed and never changes a key or proof byte.
  The default has no fixed-base commitment tables. With
  `KeyOptions::WITH_TABLES`, at the budget shape (`k = 12`), proving takes
  about 4-6% less one-thread CPU, and the single-prover peak RSS rises by
  about 11 MiB (M12 stage FIX: three processes per variant).
- `SigmaVerifier` needs only the descriptor bytes, the verifying-key bytes and
  the pinned parameters.
- The format is the KAGEMUSHA step format: RP57 Poseidon transcript, Direct
  instances and the folded-generator suffix.

## Tests

| File | What it covers |
| --- | --- |
| `tests/relation_checks.rs` | The 28 range-check cases in the strict constraint checker: two steps, two layouts, two Poseidon prefix modes, each honest witness plus its mutations (including a debit that ignores the lineage `burned_total`). Every rejection is a range-check limb lookup. Every other rule, broken alone, and the integer boundaries, on all eight shapes. In release, the per-cell tamper sweep on every honest case (every cell is pinned; this does not show what the cells are bound to). |
| `tests/forgeries.rs` | Consistent-forgery tests, one per soundness rule: a forged identity, credit identifier or `burned_total` assigned consistently (every downstream digest recomputed by the circuit) is rejected by the consumer checks or unprovable for the claimed statement; self-payment has no witness; accepted time never goes back; the statement's commitments are the opened ones; the layouts have distinct relation identities. |
| `tests/real_proofs.rs` (release) | The 28 cases as real proofs. Honest proofs verify. The library and the engine refuse each mutated witness. A forger who zeroes the failing range checks gets a proof the verifier rejects. Every flipped proof byte is rejected. Proofs on Pallas also verify. A forged identity proves only its own head. Keys and proofs are identical on 1, 2, 4 and 7 threads. |
| `tests/digest_parity.rs` | The native reference against `fixtures/native_prover/kats_v1.json`; in-circuit digests equal native ones on both fields; pinned known answers. |
| `tests/shapes.rs` | Shape selection, the pinned shapes, inventories, and verifiers rebuilt from bytes. |
| `tests/measure.rs` (ignored, release only) | The M12 measurement harness and footprint workloads. |

Validate:

```sh
cargo test -p iroha_kagemusha_proof
cargo test --release -p iroha_kagemusha_proof -- --include-ignored --skip m12_
cargo clippy -p iroha_kagemusha_proof --all-targets -- -D warnings
```

Measure one case per process, in release. The harness reads process CPU time
from `clock_gettime(CLOCK_PROCESS_CPUTIME_ID)`, runs 20 proofs per series and
prints min/median/p95/max with the load average of every run; a series with
`load1 >= 4` is marked `gate_grade=false`:

```sh
/usr/bin/time -l <measure binary> m12_send_two_level_budget --exact --ignored --nocapture --test-threads=1
```
