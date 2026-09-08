# FASTPQ compact digest qualification boundary

Updated: 2026-09-06. The current six-lane construction is **unqualified**.
No full six-lane collision or admissible proof forgery was demonstrated in this
audit. No applicable theorem was established for its required commitment and
quantum random-oracle guarantees either. Implementation parity and output width
do not settle those questions.

## Exact source and limited checks

The canonical implementation is
[`poseidon_digest384.rs`](../crates/fastpq_isi/src/poseidon_digest384.rs), SHA-256
`d97552e693a324b96cc4149945aca538656dd14a6ba8500481ee3efff5fc6899`.
Its source remains frozen. Each of six parallel lanes uses a three-element
Goldilocks state, two rate elements, one capacity element and one output element.
Each applies eight full and 57 partial rounds with exponent seven and the same
three-by-three matrix; lane-specific constants and initial states come from
separately framed SHAKE256 parameter-generation inputs.

For `p = 2^64 - 2^32 + 1`, the S-box exponent is coprime to `p-1`. All 19
nonempty square matrix minors are nonzero. The published
[GRS21 linear-layer algorithms](https://doi.org/10.46586/tosc.v2021.i2.314-352)
pass through period `4*3 = 12`, including the active invariant screen on every
matrix power in that range. The exact width-three specialization computes the
largest invariant subspace of the inactive plane by descending intersection;
it is zero. For each power `A=M^r`, `span(e0,A*e0,A^2*e0)` has dimension three,
so no active iterative candidate survives the initial screen. The older
reference script's distinct Algorithm 1 and conservative Algorithm 3 also pass;
they are reported separately from the published algorithms.

[`check_compact_mds.py`](../scripts/fastpq/check_compact_mds.py) reproduces these
decisions using exact arithmetic and frozen source hashes. Independent finite
subspace enumeration agrees on 224 matrices and 2,688 power comparisons, with
explicit scalar, cycle, inactive-eigenvector and irreducible-plane controls.
Normal execution passes; optimized Python is rejected. These bounded screens
do not establish all-period, round-selection, related-instance or combiner
security. Generated evidence is
`target/fastpq-production-validation/compact-mds-linear-checks-certificate.json`;
its successful generation is not a release qualification.

The [Poseidon paper, sections 2.1 and 2.3](https://www.usenix.org/system/files/sec21summer_grassi.pdf)
relates ordinary sponge bounds to capacity and requires linear-layer checks
beyond MDS. Applying a one-lane theorem six times yields a sum of errors at the
one-element capacity scale; it does not yield a product at the six-element scale.
The lanes' block-diagonal permutation is not a random permutation on an
18-element state. The published width-three, eight/57-round instance over a
roughly 255-bit field with exponent five is also a different parameter set.

## Message restrictions and collision correspondence

The source frames every byte field by its tag, exact length, seven-byte chunks
and terminal remainder marker. Full byte chunks are below `2^56`. Domain
metadata, field counts and lengths delimit the complete language before final
sponge padding. Domain separation prevents encoding aliases; it does not
amplify capacity or prove independence of lane outputs.

The compact verifier uses several distinct message families:

| Role | Payload and binding |
| --- | --- |
| AIR row | 342 canonical field words, 2,736 bytes; natural row index bound |
| Binary Merkle parent | Two separately framed 48-byte canonical digests; role, level, position and FRI counter bound |
| FRI pair / terminal | 64 / 128 bytes of canonical Fp4 coordinates |
| Public statement | Bounded statement plus canonical enclosing frame; substantially longer than a parent |
| Public digest API / preprocessing column | Up to `u32::MAX` bytes per field; column callers can hash complete traces |

A 64-bit collision in a lane's returned first coordinate does not merge its
three-element state. For a fixed incoming state, one rate-block absorption
followed by a permutation is injective. A capacity collision can become a full
state merge only with compensating rate elements; those compensations must fit
the actual byte alphabet and framing. Consequently, neither an unrestricted
iterated-hash multicollision slogan nor the restricted alphabet alone settles
this construction's security. The longer public API and statement messages
must be included in any general hash claim. Count internal permutations and
message lengths when translating external hash-query work.

## Required release argument

The canonical output alphabet is `Fp^6`, not all 384-bit strings. Under a uniform
product-field oracle assumption, taking four coordinates gives a uniform Fp4
element. The implementation does not prove that oracle assumption. A
canonicality test distinguishes the two ideal output alphabets; this is a model
distinction, not a demonstrated attack against a correctly specified field oracle.

Qualification must supply the following evidence for the final implementation:

1. Exact prime/exponent/width/round estimates and applicable linear-layer checks,
   including the six related parameter instances.
2. A declared full-digest binding and quantum-query game for the actual parallel
   sponge combiner, with an applicable bound or explicit independently reviewed
   assumption. Separate lane-capacity bounds are insufficient.
3. A fixed caller/work envelope covering long inputs, complete framing and internal
   permutation calls, plus all external commitments and marker-bit restrictions.
4. A compiler-compatible mapping of the actual transcript to its ideal oracle
   model. The [round-by-round state argument](fastpq_compact_round_by_round.md)
   covers grouped ideal messages; the implemented multi-call expansion is an
   unresolved separate obligation.
5. Final-artifact multi-target accounting and independent cryptographic review.
   Only a justified theorem permits substituting the intended target/query
   budgets. None of the checks above authorizes production admission.

If this construction cannot obtain the needed argument, a replacement requires
a distinct reviewed profile, regenerated proof fixtures, resource measurements
and hardware parity. Changing output size or composing more unqualified lanes
would not by itself resolve the missing argument.
