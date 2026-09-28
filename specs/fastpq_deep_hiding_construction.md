# DEEP hiding construction prerequisite

This is a source-coupled construction analysis, not a qualified production
profile. The offline compact producer uses one bounded masked DEEP construction
with explicit cryptographic entropy, exact quotient division and independently
committed composition masking. `deep_engine`, `deep_proof` and the masking
owners are private normal-library modules. The canonical frame includes the
authenticated composition-mask opening. Production callers must independently
authenticate source state, permissions, finality and replay protection; the
cryptographic and performance evidence limits below are separate obligations.

## Primary construction and remaining adaptation

[Haböck–Al Kindi, *A note on adding zero-knowledge to STARKs*](https://eprint.iacr.org/2024/1037)
(the [author-uploaded February 2025 text](https://www.researchgate.net/publication/381742262_A_note_on_adding_zero-knowledge_to_STARKs))
uses base-field vanishing masks, an independent extension-field composition
mask committed before batching, and randomized quotient decompositions.
Section 4.1 gives witness-mask freedom `h >= 2(e n_F + n_D)` and quotient-mask
freedom `h_p >= n_F + n_D`. Its canonical split uses a widened common chunk
degree, so its complete theorem cannot simply be claimed for our fixed `X^N`
split. The finite-opening argument below and the unequal-chunk implementation
are our application; the full transcript adaptation still needs review.

## Exact finite-opening obligation for the current geometry

Here `N=65,536`, `M=8,388,608`, `e=4`, `n_F=1`, `n_D=64`, and multiplication
by the execution generator `g` rotates LDE indices by 128. For each of the 301
private columns, sample independent uniform base coefficients of
`r_j in F_p[X]_<136` and `w'_j=w_j+(X^N-1)r_j`. The 41 omitted public columns
must stay exactly verifier-reconstructed and unmasked.

The direct row/OOD view has at most `64+8=72` base-linear observations. That is
not the whole simulation obligation: quotient evaluations at each base query
depend on both `w'(x)` and `w'(gx)`. Use the closure

`S = Q_D union g Q_D union FrobeniusOrbit(z) union FrobeniusOrbit(gz)`.

It has at most `2*64+2*4=136` distinct points. It is Frobenius closed, and none
is a root of `X^N-1`. Evaluation of degree-below-136 base polynomials is onto
the compatible base-linear answer space on `S`: interpolate on `S`, then
uniqueness forces Frobenius-invariant coefficients. Multiplication by the
nonvanishing `X^N-1` is invertible on that space. This proves independence of
this finite witness view for fresh uniform masks. It does not prove independence
of the entire FRI/Merkle/Fiat–Shamir transcript.

The bound is attained by legal geometry: queries `0..63` have 128 distinct
current/next points, and `z=u` in `F_p[u]/(u^4-7)` contributes eight independent
base constraints. The independent Python matrix calculation includes the actual
vanishing multiplier, has rank 136, and drops to 135 with 135 coefficients.
For `z=u^2`, which the current sampler also permits, the two OOD orbits together
have rank four and the whole map rank 132. Thus “eight OOD constraints” is a
worst-case bound, not an invariant of every sampled challenge.

## Quotient split and degree accounting

Use fresh independent `T in F_p4[X]_<65`. Preserve the exact existing split:

`Q = Q0 + X^N Q1 = (Q0 + X^N T) + X^N (Q1 - T)`.

At the 65 distinct points `Q_D union {z}`, evaluation of `T` is bijective onto
65 extension answers by Vandermonde interpolation. The randomized high answers
are consequently uniform; the low answers are determined by the quotient
identity. Evaluating the AIR on the witness closure determines `Q` at those
points. This finite-view argument does not require the two original chunks to
have equal degree. It does require a checked zero remainder in division by
`X^N-1`; agreement at a few evaluations does not establish that remainder.

The actual 923-slot Rust degree owner now has a regression for all 301 private
columns bounded by `N+136`, with public-column bounds retained. Expected
exclusive degrees are:

| Polynomial | Exclusive degree bound |
| --- | ---: |
| Masked private trace | 65,672 |
| Full mixed AIR numerator | 196,751 |
| Quotient, conditional on zero remainder | 131,215 |
| Randomized low chunk | 65,601 |
| Randomized high chunk | 65,679 |
| Independent composition mask | 131,072 |

The quotient exceeds `2N` by 143 coefficients. A producer that assumes both
unrandomized chunks fit `N` would silently truncate data. The new private
`quotient_pair_masking` owner preserves the unequal high chunk, preflights
dimensions/degrees/work/payload before private allocation, and returns fixed
zeroizing outputs. It selects no entropy source or security policy.

## Composition, wire and verifier obligations

`DeepComposition` supplies the existing 606 terms of `H_lambda`, whose powers
start at zero. The integrated offline candidate authenticates the independent
`R in F_p4[X]_<2N` together with both quotient evaluations before `z` and the
batching challenge. Its coefficient producer and initial FRI equality use
`R + lambda H_lambda`. The extra lambda is necessary: `R` owns power zero and
the relation components own distinct powers 1 through 606. Adding `R` to the
old sum without this shift would share its batching coefficient with the first
component and invalidate the straightforward batching reduction. The existing
trace `X^2` and quotient `X` shifted terms remain.

The source now has a distinct closed context identity, a three-value leaf
descriptor, bounded codec and query linkage. The coefficient source requires
explicit trace/chunk inputs and all 2N mask coefficients, with no default mask.
It checks all input coordinates and retains zeroizing result/scratch buffers.
Whole-transcript soundness/hiding analysis remains mandatory; successful
construction and verification alone do not establish that reduction.
In particular, justify power batching with the committed `R`, the larger
malicious trace-degree envelope, all mixed-arity FRI steps, finite challenge
sampling/abort probabilities, and the concrete hash/Fiat–Shamir reduction.
This change does not establish those reductions. The
[DEEP-FRI paper](https://arxiv.org/abs/1903.12243) is a soundness reference, not a
substitute for this construction's zero-knowledge argument or concrete security
qualification.

`check_deep_hiding_candidate.py` independently charges the third individually
framed Fp4 field and all containing vector/field prefixes. Roots and frontiers
retain their counts; the September 28 maximal native codec fixture passes with
this exact encoded extent:

| Quantity | Bytes |
| --- | ---: |
| Pre-mask candidate maximum DEEP DTO | 500,783 |
| Third quotient field, 64 queries | 2,112 |
| Current offline candidate layout maximum | 502,895 |
| Candidate margin to unchanged 524,288 cap | 21,393 |
| Two candidate children before AXT carrier | 1,005,790 |
| Margin to 1,048,576 inner cap before carrier/context | 42,786 |

The separate existing geometry screen's inline-row-mask proposal yields
502,831 because it relocates R into 32 inline bytes per row. That is a different
hypothetical layout, not the implemented third-field codec. The codec size
screen is not a complete proof-generation measurement. Exact complete carriers
are checked against their independently enforced byte limits.

## Prover resources and source authority still required

Exact field payload subtotals for this proposal are 327,488 bytes for all base
witness masks, 2,080 for `T`, and 4,194,304 for coefficients of `R`. Keeping
301 base coefficient columns, masks, and one N-row stripe requires 315,948,864
bytes. Within a stripe, `x^N` is constant, so evaluating
`w+(x^N-1)r` can use the existing N-point transform with adjusted coefficients;
`deep_masked_replay` now implements that identity with 128 stripes, a move-only
entropy owner and a consumed pass budget. Canonical rejection sampling takes a
caller-supplied `TryCryptoRng`; there is no production seed or deterministic
fallback. Five attempts per base coordinate bound exhaustion below 2^-140 by
a union bound over 565,484 coordinates (this is only a sampler failure bound).
The exact replay payload plan is 499,759,968 bytes including the borrowed 342
source columns, retained 301 coefficient columns, all masks, one stripe and one
maximum 128-row selection. Caller-retained prior selections, callback buffers,
allocator metadata and hardware/runtime state require separate accounting.

`deep_masked_quotient` consumes four nested stripes on the 262,144-point numerator
domain. It reconstructs all 41 unchanged public columns, runs the existing full
923-slot AIR, interpolates the complete numerator and invokes strict
zero-remainder division before applying the replay-owned `T`. The actual degree
ledger gives numerator bound 196,751 and quotient bound 131,215; the blinded
chunk bounds are 65,601 and 65,679. Its shared plan adds public preparation,
reconstruction, numerator, division and chunk buffers to the replay charge.
The DEEP coefficient composer borrows virtual masked coefficients directly from
the replay owner; it does not duplicate 301 masked coefficient arrays. These
crate-private owners now feed the normal-library `deep_prover` through the
sealed `DeepRelation` bridge and the offline quantity facade.

Those are not a complete producer memory bound. One full retained-row LDE alone
is 20,199,768,064 bytes, one M-leaf binary digest tree is 805,306,320 bytes, and
each full extension oracle is 268,435,456 bytes. The implemented shared producer
plan charges source storage, public fixed-polynomial evaluation, all replay
passes, quotient construction/division, bounded Merkle/frontier replay, FRI
layers, scratch and output. It retains no complete LDE or digest tree. Its
2 GiB payload, 2^42 structural work and 524,288-byte child defaults are unchanged.
The predecessor eight-stripe protocol remains only in test diagnostics.

The inner AIR proves declared two-update SMT/hash arithmetic.
`CompactTransferAir::new` binds caller context as opaque bytes; the prepared
ordinary/AXT wrappers supply checked quantity semantics, identities, native
leaf hashes and collision-resolved paths. The sealed bridge binds their complete
statement and relation identity. Neither binding bytes nor a valid mathematical
proof authenticates a source root or finalized transaction set by itself. Core
must independently enforce authoritative source-state and exact-spend
authorization. This construction note does not qualify those admission paths.

## Evidence and next implementation boundary

Python tests independently check full/deficient opening rank, overlapping query
sets, quadratic OOD rank, proposed nested codec charges and resource subtotals.
Rust tests cover coefficient reconstruction, base and extension evaluation,
unequal/short chunks, masks longer than the split, explicit zero masks, padding,
overflow and exact resource limits. Additional native tests check nonzero R in
coefficient composition, the highest permitted coefficient through all five
folds and all 128 terminal values, authenticated R mutation, and rejection of
the old two-value leaf. The September 28 normal-library check passes; the fresh
feature-enabled focused suite passes 134 tests, including masked replay,
full-numerator arithmetic, typed coefficient commitments, all five coefficient
folds, streamed frontier parity, private frame storage, returned device-digest
erasure and whole-attempt resource rejection. Three separately selected actual
Metal tests pass leaf/parent parity and required-device readiness, including
1024-job batches. Eleven external API tests and one public usage doctest also
pass. The current Python
geometry, hiding and source-budget selection passes 37 tests. See the
[source-scoped validation record](../docs/history/2026-09-28/fastpq-masked-native-validation.md).

`deep_prover` joins the actual owners: whole-attempt preflight,
explicit entropy, row root, full quotient, quotient/R root, OOD identity,
composition, five FRI roots and coefficient folds, terminal, transcript-sampled
query replays/frontiers, bounded canonical serialization and an independent
verifier self-check. It retains coefficient vectors, one active stripe and
streamed tree stacks; no complete LDE or digest tree is retained. A failed attempt
does not retry with reused entropy. This normal-library module serves the offline
quantity facade. Its full-size roundtrip diagnostic is explicitly selected
separately because one attempt hashes over 69 million typed leaves/parents; it
passes in the September 28 captured binary at 482,978 proof bytes, 4,346.20s wall
time and 1,118,158,848 bytes maximum RSS on the contended M1 Ultra host. The same
test rejects a different statement context, a deficient byte cap and an altered
proof. See the receipt for exact source scope and concurrent workspace drift.
This is a raw fixed-SMT child, not an ordinary/AXT quantity artifact, deployment
latency qualification or a complete hiding result.

Next execute the actual ordinary/AXT public producers, retain artifacts for
independent controls, and qualify multiple-child semantics, production resources
and the cryptographic reduction. Small arithmetic tests independently compare
128 stripes to materialized FFT/Horner evaluation, current/next rotations,
virtual/dense composition, and numerator/quotient coefficient convolution;
nonzero remainders, entropy failures and exhausted work budgets reject. Separately
qualify authenticated source-state admission and the usable SDK path. Passing
these kernel tests does not satisfy those completion criteria.
