# KAGEMUSHA recursive verifier and accumulation argument

This is the implementation-coupled M4 engineering argument for
[`iroha_plonk_recursion`](../crates/iroha_plonk_recursion/src/lib.rs). It covers
the explicit PIPA-R profile, native PIPA-AS, the incoming-claim branch rule and
the obligations that the operation relations must bind. It is not an external
audit, a security reduction for arbitrary lineage depth, or acceptance of an
unimplemented operation circuit. The current evidence remains in the
[gate checklist](kagemusha_evidence_gate.md#8-recorded-results).

## Assumptions and theorem boundary

The construction assumes discrete-log hardness in the two pinned prime-order
Pasta groups; collision resistance of the pinned Poseidon hashes; and the
random-oracle model for the domain-separated RP57 transcripts. A concrete
Poseidon implementation is a heuristic instantiation of that model.

[BCMS20](https://eprint.iacr.org/2020/499.pdf) gives accumulation definitions,
a PCD construction and a discrete-log polynomial-commitment accumulation
construction (§§4–7 and Appendix A). [BCLMS21](https://eprint.iacr.org/2020/1618)
studies split accumulation and PCD under discrete-log assumptions in the ROM.
[BGH19](https://eprint.iacr.org/2019/1021) is the underlying Halo construction.
These results motivate the architecture; citing them does not prove that the
particular layouts, encodings, mixed-curve wiring or containment rule here
satisfy all their hypotheses. The arguments below are local derivations for
this implementation. The canonical construction's caveat about unbounded
lineage depth remains in force.

## Injective framing and challenge distributions

PIPA-R has an explicit V2 descriptor and decoder. Its key commits to the
descriptor, the pinned parameter digest, the transcript profile, Direct
instances and the FoldedGenerator suffix. No decoder retries a retired
profile. The transcript starts with the `pipa-rb1` domain and absorbs the key
representation, `pipainst`, column count, every column length, every type code
and the column-major instance values. This makes the statement's shape and
type part of the challenge input. Identity points and noncanonical integers
are rejected, rather than reduced or given an alternate encoding.

Proof scalars use a single canonical base-field element when the scalar
modulus is smaller, and the S6 `(lo128, hi127)` encoding otherwise. Range and
modulus checks make S6 injective. Points use finite canonical `(x,y)` pairs;
the compressed byte decoder binds both x and the sign bit to those pairs.
The circuit also checks the curve equation. An outer byte-link relation must
bind the actual supplied length and every byte; a valid dummy used by a soft
decoder is never a replacement for that binding.

Let p be the Pallas base modulus, q the Vesta base modulus and δ = q−p.
The checked constants give
`δ = 86663725065984043395317760 = 0x47afc1f319ba3400000000`.
For a uniform Fp transcript word, identity embedding into Fq has statistical
distance δ/q from uniform Fq. For a uniform Fq word, reduction by one p gives
Fp probabilities 2/q on `[0,δ)` and 1/q elsewhere; its statistical distance
from uniform Fp is `δ(p−δ)/(pq)`, at most δ/q. Thus each mapped challenge
adds at most `2^-167.836` to a uniform-challenge hybrid bound. This is a
distribution calculation conditional on the ROM assumption, not a bound on
the security of Poseidon itself. Zero and other forbidden challenges still
produce rejection bits; total soft arithmetic selects a safe denominator
before inversion and retains the rejection bit.

## What an accumulation fold proves

For an input `(G_j,k_j,u_j)`, normalized challenges have exactly `16−k_j`
leading zeros and k_j nonzero canonical suffix elements. They determine a
length-65536 coefficient vector s_j whose nonzero prefix agrees with the
source generator prefix. The deferred claim is `G_j = Commit(s_j)`.

PIPA-AS absorbs its `pipa-as1` domain, canonical external salt, input count,
and, in fixed slot order, each G, selected source k and all 16 normalized
challenges. It then squeezes α, z and ζ. Define
`h = sum_j α^j s_j`, `C = sum_j α^j G_j`, and
`v = h(z)`. The prover opens the shifted coefficient vector `h−v e_0` at z
with claimed evaluation zero. Its commitment is `C−v g_0`. The non-hiding
IPA has 16 L/R pairs, one final scalar and the unabsorbed folded-generator
suffix. Its body is 1,088 bytes; salt makes the local witness 1,120 bytes.
There is no independent hiding-generator term that could absorb a false
commitment claim. Final acceptance includes the deferred generator decide.

For fixed input errors `E_j = G_j−Commit(s_j)`, a nonzero polynomial
`sum_j α^j E_j` has at most r−1 roots over the prime-order scalar field.
This is the cancellation term. The evaluation test supplies a separate
degree-at-most-65535 term. Under the commitment/IPA extraction assumptions,
the ROM reduction must also account for adaptive hash queries and the
challenge-map distance; a suggestive `(r+65536)/|F|` term is not a complete
standalone security bound or a literal equivalence at every challenge.
The salt is fixed before α; each grinding attempt is another hash query.

An undecided suffix can satisfy the succinct equation even for a false
claim. The API therefore returns an explicitly undecided accumulator.
Neither successful parsing nor `verify_fold` alone authorizes a payment.
Native acceptance must decide all three claims with pinned parameters: the
Pallas batch contains Omega's own opening and `acc_P`, and the Vesta batch
contains `acc_V`. Checking only the two exposed accumulators is insufficient.

Short-only non-hiding folds have a structural liveness problem: the upper
coefficient half is zero and the first-round L commitment is identity for
every salt. The caller therefore supplies an explicit pinned k16 trivial
slot when all actual sources are short. Its position is fixed, counted and
absorbed. It never replaces another obligation or appears implicitly.

## Branch uniqueness and containment

`ModeCells` constrains exactly one of Accept, Trivial and Corrected in each
incoming slot. `constrain_incoming_modes` constrains the sum of Corrected
bits to a boolean, computes `valid = AND(soft_bits) AND no_correction`, and
equates every Accept bit to valid. These equations imply the exact §2.7
branch rule: acceptance uses every original incoming claim; rejection uses
only trivial replacements except for at most one correction; if all soft
checks pass, rejection requires that correction.

Selection must preserve the original challenges and source k in Accept and
Corrected modes. Corrected also requires finite `G* != G`. Trivial selects
the pinned `ACC_TRIV` with source k16 and sixteen ones, including when the
original sigma source was k12 or k14. The selected k is constrained and
absorbed; relabeling a short padded input as k16 is invalid. A valid dummy
from soft decoding is not an authorized Trivial replacement.

Conditioned on accumulation soundness, a selected correction that eventually
decides demonstrates that the original claim failed. If all succinct and
consumer checks and all original deferred claims are true, neither a false
soft bit nor a distinct deciding correction exists. The receiver therefore
cannot choose burn for valid evidence. Conversely, an invalid deferred claim
has a deterministically computable algebraic correction. A finite correction
can be transported and folded without poisoning otherwise valid wallet
lineage. If that correction is the identity, the finite-point encoding
rejects it; the API reports an encoding failure, never an accepted burn.
Thus this is not unconditional liveness for every possible challenge vector.
The exceptional identity event needs the same ROM/computational scope as the
IPA's other exceptional messages; complete operation qualification must not
claim that an encoding failure completes a payment. Hard own/predecessor/Q/A obligations
never enter this selection API. Receive burn and ArchiveSent no-op effects
still require the complete operation, map and integer constraints; the
mode rule alone does not establish monetary conservation.

## Composition, keys and disclosure

The fixed ledger identifies each opening and each transported accumulator
separately, even if their bytes happen to be equal. Current construction
metadata covers all 14 unsplit variants. The single-sigma forwarding path
preserves its original source k; the two-sigma path includes its explicit
full-length trivial slot. Every consumer must constrain all forwarding
equalities and the mode instances across Q, A and Omega. TODO: bind the
conditional A-split context schedule when a measured variant needs it.

The `kgwvkey1` digest binds curve, k, descriptor digest, representation and
every fixed/permutation commitment in canonical order. One-hot allowlists
select sigma and A keys. The final native verifier pins `vkOmega_digest`;
each A proves that its witnessed predecessor key hashes to that carried
field and copies it forward. Bootstrap has no predecessor, and its free
digest is ultimately pinned by that same final native check. This induction
requires the actual key-hash and equality constraints in every operation,
not just a native digest helper.

Omega is hiding. Q/A and local fold proofs remain private. An accumulator
reveals G and its transcript challenge vector; for a deciding accumulator,
G is a deterministic function of that vector and the pinned generators.
This observation alone is not a zero-knowledge proof: a simulator must also
match the joint distribution and correlations of both exposed accumulators,
public step proofs, D_A and all adversarial oracle queries. TODO: complete
and independently review that simulator argument for the final composed
relation and artifact set. No additional privacy guarantee is inferred from
the fact that the local fold protocol is non-hiding.

## Executable checks and remaining review

The native PIPA-R tests pin both-curve transcript KATs, type boundaries,
profile rejection and proof-byte invariance across worker counts. Native
fold tests check genuine proof claims, nondeciding claims, omitted/reordered
inputs, source padding, trivial constants and mutation cases. Codec tests
exercise malformed points/scalars, modulus aliases, challenge-map endpoints
and every assigned cell. Obligation tests enumerate all four-slot mode
combinations on both fields and reject dropped, duplicated, gated or
reshaped fixed obligations.

The full operation relations must add the foreign-key, dropped-accumulator,
bad history-path, invalid-burn and middle-proof mutations, and the depth-16
cycle test. The carry argument is separately recorded in
[the reviewed M3b memo](kagemusha_ff_carry_v1.md). The final soundness review
must bind the actual descriptor/VK hashes and generated proof behavior.
