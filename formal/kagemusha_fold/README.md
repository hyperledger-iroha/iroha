# Fold offset, committed-transcript and conditional-model controls

This package checks pre-mask offset-polynomial identities, the selected Exact
committed-fold RP57 transcript, a bounded verified-source Terminal projection,
and a generic conditional ideal-permutation game. The offset and ideal-model
checks are mathematical comparisons with explicit hypotheses. They do not supply the selected fold's
stopping distribution or a fixed-RP57 privacy theorem. The separate
[stopped-path scalar controls](../kagemusha_committed_fold/README.md) exercise
the mask-dependent finite law under their own bounded model.

Run the standard-library controls from the repository root:

```sh
python3.12 -B -S -m unittest discover -s formal/kagemusha_fold -p 'test_*.py' -v
```

## Pre-mask offset identities

The polynomial controls check the algebra in the
[conditional comparison bound](../../specs/kagemusha_recursion_soundness_v1.md#conditional-honest-fold-abort-bound).
The selected `accumulation.rs` retains the following recurrence before adding
each W mask: combine ordered source coefficient vectors with powers of alpha,
subtract H(z) from coefficient zero, form the L/R offsets, then fold a with
inverse u and b/g with u. The symbolic model generates source vectors
independently from the bit-product definition. Logs describe an additive cyclic
group, including dependent generators; no discrete-log independence is assumed.

For one through four rounds, the tests check exact leading z coefficients after
clearing Laurent denominators, their nonzero top coefficients and total degree
bounds. Finite-characteristic cases compare those expressions with a separate
scalar recurrence. Short-only sources make the first L offset identically zero;
removing the evaluation shift changes R's leading coefficient. Abstract
zero-width sources are constant-polynomial boundary examples, not admitted
native `FoldInput` objects: the native source width is 1 through 16. For depths
at least two, the short-only negative also has positive-width source examples.

The k16 degree-sum coefficient remains `3,015,170 + 32(m−1)`. The exhaustive
one-round F17 experiment retains every round-variable zero, offset identity and
final-generator identity event, including tapes after an earlier event. These
are the exact hypothetical events of the independent raw-variable comparison.
The selected fold instead binds all masks through D before its challenges,
adds W masks to L/R, maps actual round challenges with `nz`, and admits an
identity final G. Its stopping law cannot be inferred from that union bound.
In particular, the old raw-coordinate atom bound cannot be assigned unchanged
to `nz(raw)`, which merges zero and one.

`source_manifest.json` binds the three inspected native owners and the local
arithmetic controls. The symbolic cases are bounded examples, not a general
theorem prover, a native proof, or evidence of challenge independence.

## Selected committed-fold transcript

The transcript controls execute the unchanged standard-library RP57 reference.
They compare both constant tables with the retained native KAT constants,
check canonical scalar limbs and field conversion, and trace every primitive
input/output against a separately implemented rate-two block scheduler.

The Exact input digest is a separate sponge. It binds the domain and encoded
word count, parameter-digest limbs, purpose, input count, every ordered input
and all 32 masks. Rho is outside that digest and opens the resulting digest in
D. The challenge sponge absorbs its own domain, the four public context words
and D before alpha, z and zeta, then each of the 16 L/R pairs before its actual
round challenge. It takes 54 primitive calls across 19 squeezes on either
curve. The separate Exact digest takes `36+floor(35m/2)` calls for Pallas and
`20+floor(19m/2)` for Vesta, where m is the input count. These controls cover Exact binding only.
The Exact API refuses `WrapperVesta`. The separate bounded Terminal controls
below derive their projection from genuinely verified reference-proof originals;
they do not grant native TerminalFrame admission.

The Rust prelude KAT uses diagnostic k4 parameters and syntactic k16 claims.
Its parameter original must be reconstructed exactly: take the verified k6
coefficient-generator prefix and W/U, compute the k4 inverse transform, encode
the canonical parameter bytes, then hash that full encoding. Neither a sliced
k6 Lagrange vector nor its parameter digest represents k4. This bounded
reconstruction supplies no k16 parameter set or verifier authority.

The controls retain every squeeze edge, carried state, empty/even/odd padding,
the logical-history padding alias, context/input/mask/D mutations, and malformed
or unbounded input refusal. Initial alpha/z/zeta retain the raw scalar map;
actual round u uses `nz` without another squeeze. The reference reader absorbs
final c and reads the unabsorbed full-group G suffix, including identity, with
no later squeeze. Ordinary D/L/R still require finite points. A changed
unsqueezed suffix can leave earlier challenges unchanged without validating
the fold equation.

`transcript_source_manifest.json` lists the exact controls, reference imports,
parameter and constant data, native/circuit schedule owners and Rust KAT test.
Missing roles, foreign entries, noncanonical hashes and changed bodies refuse.
Those checks are source consistency, not authenticated artifact admission. The
transcript traces are syntactic; they produce no native fold proof and do not
assert the IPA equation. No external Python package or `target/` input is needed.

## Bounded verified-source Terminal projection

`terminal_fixture.py` constructs deterministic public test proofs of a minimal
zero-constraint relation with one 69-word Direct Bounded column, base RP57 and
a folded-generator suffix. The unchanged full reference verifier admits the
existing official Vesta k7 parameter bytes and checks each 800-byte proof,
including its seven-round generator decision. These are diagnostic proofs,
not A operation proofs or production randomness. Other claims in the frame
remain syntax, without generator decisions or burn authorization.

`BoundedTerminalFrame` accepts exact descriptor/key/parameter originals from one
fixed diagnostic catalog, an original proof and a copied public frame. It runs
full verification before replaying that source's first transcript prefix and
deriving the ordered part, own-opening, predecessor and incoming slots. It
offers no raw-value or slot setters; immutability protects ordinary API use,
not hostile Python reflection. Its k7 own opening has nine leading normalization
zeros. This explicit adaptation does not satisfy native `TerminalPlan`'s k16
requirement. The minimal relation has no advice columns; its first squeeze takes
38 RP57 calls.

The separate `pipa-tm1` digest binds context, descriptor, the unreduced first Fq
squeeze, the exact own opening and all 32 masks. Its 60 words take 31 primitive
calls. The downstream syntactic challenge trace takes 54 calls across 19 squeezes;
it produces no fold proof or equation verdict. These counts exclude the rest of
upstream proof verification and are not latency measurements. Two fully valid
proofs share one frame/key/raw prefix but have different own openings and
terminal digests. Omitting the own slot makes their diagnostic digest inputs equal.

Controls reject all 69 original-frame substitutions and malformed proof bytes.
Separate proof-valid cases exercise modes, padding, point canonicality and
inactive fields. Inactive correction points need not be zero, but must be well
formed. A synthetic raw-zero versus raw-Fp comparison changes copied digest words
only; it does not force a verifier output or create an admitted source. Source
and index substitutions, immutable copies, masks, rho and healthy replay after
rejection have separate controls. `terminal_source_manifest.json` pins the new
implementation and reference inputs without adding parameter authority or
changing the reference verifier. Native k16 admission, A constraints, circuit
refinement and installed catalogs remain separate qualification requirements.

## Conditional ideal-permutation game

`ideal_game.py` is a source-independent finite partial-bijection game. It never
calls RP57. A private path begins at its first salt-bearing input
`(initial_capacity, domain, salt)` and carries each output capacity through
rate-two absorption. Its supplied output tapes disclose one rate coordinate.
Every public attempt names both primitive endpoints, including inverse, replay
and refused-candidate attempts.

Public endpoint records must be independent of private salts/capacities given
the disclosed words. Virtual salt and output-capacity tapes must be independent
uniform draws conditional on those disclosures. The common initial capacity
must be fixed before, or independent of, the virtual private capacities,
conditional on the disclosed words. The
model cannot infer these hypotheses from arbitrary caller values.

For B fresh attempts, H private edges and Q public attempts, the first-hit
comparison is

`min(1, [2Q(B+H) + B(B-1)/2 + H(H+1)/2] / p)`.

It charges a fresh salt collision, a private output capacity equal to the
initial or an earlier private capacity, a public endpoint equal to a salted
initial input, or a public endpoint capacity equal to a private capacity.
Each public attempt contributes two endpoints. The bound applies to complete
virtual tapes without conditioning secrets on earlier misses. No-bad tapes
form a consistent partial permutation; input/output vertex coalescence is legal.
Rejected attempts preserve all earlier edges, and a changed same-salt request
cannot masquerade as exact retained replay.

The controls enumerate all 83,521 independent p17 single-edge tapes: 13,889 bad
and 69,632 no-bad, below the nonvacuous 5/17 bound. A separate 2,187-tape two-edge
case checks carried capacity and replay even when its bound is vacuous.
Forward/inverse conflicts retain the original table. Negative controls expose
a salt/capacity, inspect private table occupancy, or choose the initial capacity
after the private tape; these violate premises, not the protocol.
`ideal_source_manifest.json` pins only this local model, its tests and this
README. It introduces no new assumption for the selected C12 argument.

All groups retain their separate scope. Source agreement supplies no
fresh-answer law for fixed public RP57. These checks establish neither adaptive
privacy, entropy/resource guarantees, recursive continuation, current catalog
validity, physical durability nor full C12 or release qualification. See the
[transfer boundary](../../specs/kagemusha_recursion_soundness_v1.md#retained-arguments-and-selected-source-transfer).
