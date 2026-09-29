# RAM-LFE encryption replacement plan

Date: 2026-09-29. Status: source-coupled design work, no selected or qualified
replacement. Both BFV production backends remain unavailable. This plan refines
the encryption prerequisite of [ZK03](zk_first_release_goals.md) and the
[execution relation](ram_lfe_execution_proof.md); it does not enable a profile,
change admission limits or establish a security level.

## What the existing arithmetic establishes

The exact profile is known insecure. The rounded diagnostic path instead uses
small centered errors and scaled plaintexts, but has no independent security
qualification. The following are implementation facts, not evidence that either
profile protects encrypted inputs or hidden functions.

| Source owner | Current contract and replacement consequence |
| --- | --- |
| [`BfvParameters::validate`](../crates/iroha_crypto/src/fhe_bfv.rs) | One `u64` ciphertext modulus, divisible by plaintext modulus, with `n*q*q*t <= i128::MAX`. The same owner constrains rounded arithmetic. A larger genuine RNS modulus needs a new internal representation; relaxing this overflow guard is not an implementation. |
| `ram_lfe_bfv_parameters_v1` | Ring degree 64, `t = 257`, `q = 257*2^48`, decomposition base `2^12`. Registration recognizes this exact diagnostic shape; it supplies no security qualification. |
| `keygen_bounded_noise_with_relinearization_from_seed` and `encrypt_bounded_noise_from_seed` | Diagnostic ternary secrets/masks, centered errors bounded by one, public-key relation `b = -a*s-e`, plaintext scale `q/t`, and matching noisy relinearization material. These private constructors are available through explicit test fixtures only. |
| `multiply_ciphertexts_bounded_noise` and `multiply_ciphertexts_bounded_noise_rns_exact` | Centered raw products, deterministic coefficient scale-and-round, then relinearization. RNS bridges accelerate exact reconstruction around the scalar modulus; their reconstruction-prime product is not the ciphertext modulus `Q`. |
| `BfvIdentifierCiphertext::slots` | 64 separate scalar ciphertexts, including the input-length slot. This is not 64 SIMD positions inside one ciphertext. |
| [`eq_zero_indicator`, `pow_ciphertext`, `select_eq_zero`](../crates/iroha_crypto/src/ram_lfe.rs) | `1-c^256` over the field of 257 elements, evaluated with eight squarings, a final multiply by encrypted one, then a multiply by the branch difference. A planner must charge the entire expansion. |

The current rounded bound already prevents dispatching that last operation to
the rounded evaluator. With `N=64`, `t=257`, `q=257*2^48`, base `B=4096`, five
decomposition digits and coefficient error bound one, the public functions give

```text
fresh E = 2*N + 1 = 129
capacity = floor((q/t - 1)/2) = 140,737,488,355,327
mul(Ea,Eb) = N*(t-1)*(Ea+Eb) + ceil(N*Ea*Eb*t/q)
             + N*N + N + 1 + N*5*(B-1)
```

| Consecutive squaring | Output bound | Within current capacity |
| --- | ---: | --- |
| 1 | 5,541,634 | yes |
| 2 | 181,589,577,480 | yes |
| 3 | 5,950,334,773,774,910 | no |

These values reproduce `bfv_fresh_bounded_noise_ciphertext_bound`,
`bfv_multiply_bounded_noise_output_bound`,
`bfv_key_switch_extra_residual_multiple_bound` and
`rounded_plaintext_decoding_capacity`. They show rejection by the current
conservative bound, not a measured decryption failure or a security experiment.
The source hashes, excerpts and integer-only reproduction are retained under
`dist/zk-remediation/2026-09-29/ram-lfe-encryption-replacement/`.

## Architectural decision before implementation

Investigate a bounded leveled BFV-RNS construction with exact integer plaintext
semantics, using the [Fan–Vercauteren construction](https://eprint.iacr.org/2012/144)
as a mathematical reference. This is a candidate direction, not a selected
cryptographic profile. Approximate arithmetic does not supply the exact equality
and identifier mapping required by this interpreter. A genuine noise-reducing
bootstrap would require a separate algorithm, parameter and proof qualification;
adding an encryption of zero does not supply it.

Resolve packing and plaintext semantics before choosing parameters. Ordinary
fully split prime-field BFV batching requires `t = 1 mod 2N`, as documented in
[Microsoft SEAL's encoder example](https://github.com/microsoft/SEAL/blob/main/native/examples/2_encoders.cpp).
For the existing `t=257`, this cannot hold for power-of-two `N>128`. Retaining
257 therefore needs a different reviewed representation or costly separate
ciphertexts; choosing a larger batching prime changes arithmetic and equality
semantics and must be an explicit first-release language/profile decision.
Packing coefficient positions is not interchangeable with independent scalar
slots under polynomial multiplication. Unused output positions must not expose
intermediate plaintext or hidden-function information.

The [F257 scalar-packing candidate](ram_lfe_plaintext_packing.md) supplies a
specific alternative: constants in extension-field CRT slots, encoded in a fixed
sparse subalgebra. It preserves scalar F257 operations without requiring fully
split degree-one slots. Its input-admission, masking, rotation, noise and complete
proof costs remain unqualified; no modulus/profile is selected by this algebra.

For scale only, an **unselected** representation with `N=4096`, two 64-bit RNS
limbs, two ciphertext components and 64 separate ciphertexts consumes
`64*2*4096*2*8 = 8,388,608` bytes before framing. This is not a parameter proposal
or security estimate; it demonstrates why the current scalar envelope cannot
simply be widened under the existing 1 MiB total-envelope default. The complete
input, output, keys, proof and private witness costs need a new measured budget.

## Milestones and acceptance evidence

1. **Fix the threat model and plaintext contract.** Specify the key owner,
   evaluator, opening authority and verifier; who may choose malformed keys or
   ciphertexts; adaptive query limits; visible errors, timing, lengths and unused
   slots. Input confidentiality does not imply hidden-circuit privacy. Review a
   concrete circuit-private or sanitizing construction and its assumptions;
   [Hwang, Min and Song](https://eprint.iacr.org/2025/203) is relevant research,
   not an adopted implementation or qualification. Retain the separately trusted
   plaintext-opening attestation. ZK execution alone cannot authorize a claimed
   plaintext. Produce one normative encoding/operation specification and explicit
   leakage statement before freezing a parameter profile.

2. **Create a genuine RNS parameter and residue owner.** In the existing crypto
   boundary, introduce one immutable validated internal profile with pinned
   `Q = product(q_i)`, ring dimension, plaintext encoding, secret/error
   distributions, decomposition and any special-prime basis, level schedule and
   NTT roots. Define residue ordering, canonical ranges, basis extension,
   relinearization, modulus switching and every rounding tie exactly. Residues
   remain in the specified RNS domain; the old scalar reconstruction chain does
   not become `Q` by renaming it. Reject unsupported shapes and overflow before
   allocation. No caller-selected primes and no weakened old `i128` checks.

3. **Select and independently review complete parameters.** Pin the actual
   distribution, estimator version and assumptions, classical/quantum cost
   model, key-switch material, decryption-failure target and lifetime/query
   budget. Security, correctness and operation capacity must be evaluated jointly,
   following the [HE implementation guidelines](https://eprint.iacr.org/2024/463).
   Do not invent a bit-security claim from ring degree or use a library's default
   as a qualification certificate. Review malicious-input and circuit-privacy
   requirements separately from IND-CPA encryption. The chosen representation
   must also meet the existing envelope/proof resource contract; otherwise record
   the unsatisfied requirement without raising limits to declare success.

4. **Compile a bounded typed operation/noise plan.** Expand every instruction
   into its actual arithmetic, including `SelectEqZero`'s full exponentiation,
   constant and branch multiplications, operand levels and relinearization.
   Track initialized reads, live ciphertexts, output count, modulus level,
   conservative noise/failure bounds and checked memory/work costs. Supporting
   add/subtract/plain operations alone is not programmed-backend completion.
   If the final semantics change, regenerate the equality expansion and prove it
   correct for all admitted plaintexts. A count named "depth 16" is not a noise
   proof. Reject unplannable programs before key generation or private allocation;
   give a typed error naming the offending operation and exhausted budget.

5. **Implement the smallest internal arithmetic slice.** First build the genuine
   RNS owner, canonical bounded codec and deterministic reference arithmetic,
   then encryption/decryption, addition, plaintext multiplication, ciphertext
   multiplication and relinearization for one reviewed candidate profile.
   Keep it unused by production admission. Use independently generated vectors
   and cross-library plaintext/arithmetic parity with the same parameters and
   explicit fixture randomness. Require byte parity only where the reference
   specifies the same coins, rounding and canonical representation; equivalent
   randomized ciphertexts need not have identical library serialization. Test
   zero/boundary residues, rounding ties, wrong levels/keys, malformed lengths,
   arithmetic overflow and canonical re-encoding. Seeded helpers stay in test
   fixtures; the public facade owns secure randomness.

6. **Qualify ownership and deterministic acceleration.** Keep private keys,
   randomness, CRT/NTT intermediates, rounding scratch, temporary ciphertexts and
   execution witnesses in clearing owners with bounded allocation. Verify live
   cell erasure on success, error and unwind, including partially decoded values
   and ownership transfers. Do not promise to erase caller-owned copies. Pin
   the scalar reference semantics, then test NEON/SIMD/Metal/CUDA implementations
   against it where supported. Backend selection must not affect canonical
   outputs or consensus results; no floating-point or scheduling-dependent
   reductions. Accelerator absence must retain the same deterministic behavior.

7. **Bind the complete execution relation.** After the arithmetic/privacy design
   is reviewed, constrain every limb range, integer lift, carry, quotient,
   remainder, rounding decision, modulus level and relinearization transition,
   as well as the program, initialized state, registers, memory and outputs.
   A field equation alone cannot prove an integer equation modulo `Q`. Bind any
   adopted randomized evaluation/sanitization step and its coins to the specified
   relation. Adopt the [semantic commitment proposal](ram_lfe_semantic_commitments.md)
   only after its independent review, updating packing and cost estimates for the
   actual RNS profile. The verifier derives instances and evaluation-key material
   from authoritative typed policy; no prover-selected replacement sidecar.
   Keep the stable logical `Cfunction` independent of encryption-key rotation,
   while `Cpolicy` and the receipt bind every math/key/profile identity. An opaque
   identifier derives from the authenticated opened function output and context,
   never from a ciphertext hash or an unverified opening.

8. **Measure and integrate one final profile.** Synthesize the maximum complete
   relation and measure rows/cells/lookups, native proof bytes, prover time/peak
   memory and verifier latency. Recheck configured `max_k` before parameter work
   and total proof/envelope/key/instance sizes before decoding or allocation.
   Current defaults remain `k <= 16`, proof `<= 192 KiB`, total Norito envelope
   `<= 1 MiB` and a 20 ms soft verification budget. The old degree-64 estimate of
   2,305 Poseidon permutations/149,825 round rows is not an estimate for the new
   encryption profile. Require real proofs, arbitrary-field witness mutations,
   modular aliases, malformed frames, policy/key swaps, rekey stability and wrong
   opening-authority/output tests. Qualify the same source candidate through
   registration, activation, restoration, stateless/identifier receipt consumers
   and a four-validator network before replacing refusal. Retire old profiles,
   keys, roots and frames with no aliases or fallback; ABI remains V1.

## Developer boundary and first implementation gate

The eventual facade accepts a typed program and inputs, compiles a resource plan
and chooses a qualified fixed profile. Developers do not choose noise,
transcripts, rings or seeds. Key generation, encryption, evaluation, authenticated
opening and proof verification return distinct typed results; a higher-level
resolve operation may compose them without hiding their different authorities.
An unavailable or over-budget program fails before private/network work.

The next implementable slice is milestone 2's internal RNS profile/residue owner
and a test-only scalar reference, after milestones 1 and 3 supply reviewed
encoding/parameter inputs. Small arithmetic fixtures may validate code earlier
but cannot select a production profile. Full `SelectEqZero` capacity, hidden
function privacy and the complete proof relation remain hard gates. Do not switch
the current interpreter to rounded dispatch, promote diagnostic refresh as a
bootstrap, or increase limits as a substitute for any gate.

TODO: select and independently review the replacement construction, implement
and qualify the above milestones, then remove the production refusal. The
current arithmetic, unused Poseidon candidates and this plan do not complete
ZK03 or establish a secure encrypted product.
