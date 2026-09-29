# RAM-LFE scalar packing over F257

Date: 2026-09-29. Status: public algebra candidate, no encryption profile selected
or enabled. This refines the packing question in the
[replacement plan](ram_lfe_encryption_replacement.md). It changes no production
code, security claim, admission limit or proof relation.

## A fixed scalar subalgebra

For the illustrative ring `R = F257[x]/(x^4096+1)`,
`257^k = 1+256k mod8192`, so the multiplicative order of 257 modulo 8192 is 32.
The finite-field cyclotomic factorization theorem therefore gives 128 distinct
irreducible factors of degree 32. The theorem and factor count are stated in
[Mathlib's cyclotomic factorization source](https://leanprover-community.github.io/mathlib4_docs/Mathlib/RingTheory/Polynomial/Cyclotomic/Factorization.html).
This agrees with the general extension-field slot model documented by
[HElib](https://ibm.github.io/fhe-toolkit-linux/html/helib/classhelib_1_1_ptxt.html);
it is not a claim that SEAL's BFV `BatchEncoder` supports these parameters.

The following explicit encoding is a derivation for this ring. Fix `zeta=3` in
F257, whose order is 256, and `beta_j = zeta^(2j+1)` for `0 <= j < 128`. Then

```text
x^4096 + 1 = product_j (x^32 - beta_j)
R = product_j F257[x]/(x^32-beta_j) = product_j F_(257^32)
```

Each root of `x^32-beta_j` has order 8192, hence minimal polynomial degree 32;
the degree-32 factor is irreducible. The factorization follows by substituting
`y=x^32` in the split polynomial `y^128+1`. Embed a scalar `v_j` as the constant
element of each factor field. All 128 scalars can then be encoded as

```text
encode(v) = g(x^32),       degree(g) < 128
g(beta_j) = v_j
g_k = (1/128) * sum_j v_j * beta_j^(-k) mod257
decode(g)_j = sum_k g_k * beta_j^k mod257
```

Only 128 coefficient positions, the multiples of 32, can be nonzero. This is an
injective ring embedding of `F257[y]/(y^128+1)` into `R`, so componentwise addition,
subtraction and multiplication preserve scalar F257 semantics. There is no need
to increase the plaintext modulus merely to obtain scalar packing.

The scalar subalgebra is exactly the Frobenius-fixed part: `m^257=m`. Every
multiple-of-32 monomial is fixed; each other monomial's Frobenius orbit contains
its negative, forcing its coefficient to zero in odd characteristic. This also
gives a direct encoding/well-formedness condition, independently of a library
slot numbering convention.

## Selection, movement and output disclosure

For admitted scalar slots, `1-c^256` is one exactly when `c=0` and zero otherwise.
The existing eight squarings, final multiply by one and branch multiplication
therefore implement `SelectEqZero` componentwise without changing its F257
semantics. This statement does **not** hold for arbitrary extension-field slots.
For example, the nonzero element `c=x` in `F257[x]/(x^32-beta_0)` has
`c^256=beta_0^8 != 1`; its purported zero indicator equals 122. A decoder accepting
arbitrary ciphertexts cannot assume that their unknown plaintext is scalar.

Admission must establish the encoded plaintext's subalgebra membership, byte and
length ranges, and zero unused slots. A client can possess plaintext/encryption
coins for an input well-formedness proof; the evaluator generally cannot supply
that witness. Select and review this separate input-admission mechanism and its
binding to the execution statement, rather than silently requiring the evaluator
to know the client's plaintext. Similarly, trusted policy must authenticate
appropriate key material; shape checks alone do not establish its security.

The current tape accesses independent scalar inputs/registers, not a uniform SIMD
program. One possible adapter keeps scalar intermediate values broadcast in all
128 slots. Loading input slot `j` first multiplies by the public algebra's fixed
idempotent `e_j=encode(one_hot_j)`, then sums its orbit. On the scalar subalgebra,
automorphisms `x -> x^a` act as the permutation
`j -> ((a*(2j+1)-1)/2) mod128`. The exponents
`5,25,625,5601,4033,3969,8191` support seven doubling additions that broadcast a
single selected scalar. Actual encrypted rotations need seven authenticated
Galois/key-switch keys and incur their full noise, work and proof costs. The
hidden input index and choice of mask must be constrained inside the private
execution relation; this algebra does not provide an uncosted random-access gate.

Outputs can be assigned to fixed slots by idempotent masks and summed into one
packed plaintext. Force every undeclared output slot to zero and prove the masks
and order. Masking plaintext slots does not sanitize ciphertext noise or establish
hidden-function privacy. The opening authority must authenticate only the exact
declared output sequence and context, with no alternate decode of extension-field
coordinates. Function identity and rekey stability still follow the semantic
commitment design.

## Noise, representation and proof costs

Centered coefficients of a generic encoded scalar vector are bounded by 128 and
have at most 128 nonzero positions, giving an `l1` bound of 16,384. A single-slot
idempotent has all 128 centered absolute coefficient magnitudes `1..128`, so its
exact `l1` norm is 8,256. Under the elementary convolution inequality,
`||e_j * noise||_infinity <= 8,256 * ||noise||_infinity` before any additional
modulus/rounding contribution. Plaintext masking is therefore not a unit-cost
scalar multiply with negligible noise. Slot broadcast adds seven key switches
and additions. The planner must derive bounds for the complete selected BFV-RNS
algorithm; these public polynomial norms are only inputs to that derivation.

Ciphertext components and errors still live in the full degree-4096 ring. Do not
compress ciphertext coefficient arrays to the 128 sparse plaintext positions or
substitute 128 for the ring dimension in ciphertext multiplication/noise terms.
The plaintext embedding also does not authorize sampling keys or encryption
randomness from that smaller subalgebra; their reviewed distributions stay
separate from message encoding.
For illustration only, two components with two 64-bit RNS limbs consume 131,072
bytes per ciphertext before framing, versus 8,388,608 bytes for 64 such separate
ciphertexts. Two limbs are not a recommended or qualified modulus. Evaluation
keys, masked/broadcast intermediates, input admission, output packing and the
complete proof may still exceed current budgets.

Pin root choice, slot order, plaintext coefficient representatives, sparse
embedding, ciphertext residue order and exact bounded Norito framing as one
profile contract if this design is adopted. Each residue still requires canonical
range checks; no alternate sparse ciphertext decoder or extra public sidecar is
introduced. Input admission and execution must share the same encoding identity.
The full relation must constrain the transform, slot masks, automorphisms, key
switches, output-zero policy and all ordinary BFV arithmetic. Recompute complete
rows/cells/lookup, proof/envelope and memory costs; the prior degree-64 estimate
does not qualify this representation.

## Evidence and next decision

`dist/zk-remediation/2026-09-29/ram-lfe-encryption-replacement/packing-algebra.json`
records public integer/F257 checks: order/factor facts, all 128 basis roundtrips,
four mixed product/selection vectors, all 257 scalar zero indicators, all 128
fixed broadcasts, an extension-field counterexample and output masking. Its
companion script generates no key or ciphertext and performs no encryption,
decryption or recovery experiment. The test matrix fingerprint is not a wire
profile commitment or an independently reviewed cryptographic artifact.

The unused Rust [plaintext candidate](../crates/iroha_crypto/src/fhe_bfv/plaintext_packing_candidate.rs)
now implements clearing fixed-size scalar/coefficient owners, transforms, masks,
plaintext ring arithmetic and automorphisms under `cfg(test)`. Its exact-source
isolated Cargo harness passes seven native controls and strict Clippy, including
the independent vector, canonical bounds and observed live-cell erasure on
success, error and unwind. Evidence is retained in
`dist/zk-remediation/2026-09-29/ram-lfe-plaintext-candidate/20260929T110759Z/`.
The first attempt's native passes and one formatting-idiom Clippy failure remain
separate. Normal whole-crypto integration is pending; this module defines no
encryption, wire frame or supported profile.

Prefer evaluating this fixed F257 scalar packing before changing plaintext
semantics. It resolves an algebraic packing obstacle, not the replacement's
parameter, noise-depth, circuit-privacy or proof-capacity gates. Next compare the
complete masked/broadcast tape plan against a reviewed BFV-RNS modulus and key
schedule, including maximum `SelectEqZero` programs and every output. Production
remains unavailable until all replacement-plan milestones pass.
