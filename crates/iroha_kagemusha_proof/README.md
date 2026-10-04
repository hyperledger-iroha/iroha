# iroha_kagemusha_proof

Native-only KAGEMUSHA relations on the PIPA-v1 engine (`iroha_plonk`,
`specs/plonk_ipa_v1.md`). This crate is the compilation boundary of KAGEMUSHA
relations built on the native stack. It links `iroha_pasta`, `iroha_plonk` and
`iroha_plonk_gadgets` only, never the vendored halo2 stack or `iroha_core_zk`.

## Prototype status

Everything here is a **prototype** of the proposed *split-lineage* design:
step proofs on the payment path, with the recursive lineage proof in the
background. Owner approval of that design is pending. The relations reproduce
the M7 measurement semantics (`g3_proof_scaling_measurement_tests.rs`,
`m7_step`), including its domain labels (`m7score1`, `m7stmnt1`, ...). They are
not a protocol format, and no protocol path uses them. A frozen relation needs
versioned types, shared Swift/Kotlin vectors and owner sign-off.

## Relations

`sigma_send` and `sigma_recv` (`SigmaCircuit`) on Pow5 sponge lanes,
running-sum range checks, checked `u128`/`u64` arithmetic and glue gates.

- **State.** A 40-field wallet state: a 10-field core plus a 30-field
  remainder.
  - Two-level (the M7 recommendation): `H(core || H(remainder))`.
  - Flat: `H(core || remainder)`.
- **Both steps.**
  - Open the predecessor commitment and require lifecycle Active.
  - Require a nonzero `u128` amount and `sequence + 1 < 2^128`.
  - Commit the successor with a fresh state nonce.
- **`sigma_send`.**
  - Debits `amount + fee` without overdraft.
  - Advances the send ordinal without overflow.
  - Requires `request_policy_epoch <= policy_epoch` and
    `max(accepted_time_floor, request_time) <= lower <= upper` (all `u64`).
  - Appends `send_chain` and binds the 24-field Request body.
- **`sigma_recv`.**
  - Credits the amount without overflow and appends `recv_chain`.
  - It is the precomputed form: the payment digest in its effect is zero.
- **Public outputs.** The Poseidon digest of the 32-field G1 statement
  encoding, plus the Request digest for `sigma_send`.

`StepWitness::evaluate` is the native reference. It returns every digest, the
successor and the relation `Violation`s. The circuit compares every in-circuit
digest with it while the witness is known.

## Shapes and proofs

- `select_shape` chooses the smallest `k`, then the fewest lanes, at which a
  key-generation synthesis fits. A proof byte budget is optional; the default
  is the 3.5 KB gate. Proof lengths are exact, taken from the descriptor.
- Shapes chosen:

  | Relation | With the 3.5 KB budget | Without a budget |
  | --- | --- | --- |
  | Two-level | `k = 11`, one lane, 3,232 B | `k = 10`, two lanes, 3,776 B |
  | Flat | `k = 12`, one lane, 3,296 B | — |

- The glue chip shares the columns of the least loaded lane, in the rows after
  its last permutation block.
- `SigmaProver` derives the parameters, generates keys and proves. It refuses
  a witness that breaks the relation.
- `KeyOptions` trades memory for speed and never changes a key or proof byte.
  The default has no fixed-base commitment tables. With
  `KeyOptions::WITH_TABLES`, at `k = 11`, σ_send proves in about 8% less CPU
  time but its peak RSS rises by about 6 MiB, to about 33 MiB.
- `SigmaVerifier` needs only the descriptor bytes, the verifying-key bytes and
  the pinned parameters.
- The format is the KAGEMUSHA step format: RP57 Poseidon transcript, Direct
  instances and the folded-generator suffix.

## Tests

| File | What it covers |
| --- | --- |
| `tests/relation_checks.rs` | The 24 M7 relation-check cases in the strict constraint checker: two steps, two layouts, two Poseidon prefix modes, each honest witness plus its mutations. Every rejection is a range-check limb lookup. It also breaks each relation rule alone, and (release) runs the per-cell tamper suite on every honest case. |
| `tests/real_proofs.rs` (release) | The same 24 cases as real proofs. Honest proofs verify. The library and the engine refuse each mutated witness. A forger who zeroes the failing range checks gets a proof the verifier rejects. Every flipped proof byte is rejected. Proofs on Pallas also verify. |
| `tests/digest_parity.rs` | The native reference against `fixtures/native_prover/kats_v1.json`; in-circuit digests equal native ones on both fields; pinned known answers. |
| `tests/shapes.rs` | Shape selection, inventories, and verifiers rebuilt from bytes. |
| `tests/measure.rs` (ignored) | The M12 measurement harness and footprint workloads. |

Validate:

```sh
cargo test -p iroha_kagemusha_proof
cargo test --release -p iroha_kagemusha_proof -- --include-ignored --skip m12_
cargo clippy -p iroha_kagemusha_proof --all-targets -- -D warnings
```

Measure one case per process, in release:

```sh
/usr/bin/time -l <measure binary> m12_send_two_level_budget --exact --ignored --nocapture --test-threads=1
```
