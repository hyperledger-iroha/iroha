# KAGEMUSHA recursive proof components

This crate implements the native accumulator and fold algorithms in
[`kagemusha_lambda_omega_v1.md`](../../specs/kagemusha_lambda_omega_v1.md)
§§3.3–3.4 and §8, the complete constrained PIPA-R and PIPA-AS succinct
verifiers, and the incoming-mode rule and static obligation ledger. Component
constraints and native equivalence are tested; recursive relation composition
and release qualification remain open.

`codec` links byte-tape messages to canonical proof scalars and finite points,
including total soft decoders with fixed dummies. Scalar cells retain injective
S6 limbs. The transcript absorbs Pallas scalars as two limbs and Vesta scalars
as one native word. Its challenge map proves the canonical base-field integer
and Vesta's exact conditional subtraction of p. Instance membership separately
proves `Field`, `Bounded` and `Bits(0..253)`; consumers must include every soft
verdict in their acceptance predicate.

`AccumulatorT` encodes exactly 544 bytes (`G`, sixteen nonzero challenges).
`FoldInput` checks source k and leading-zero padding. `FoldWitness` encodes a
canonical 32-byte base-field salt followed by the 1,088-byte non-hiding IPA
body. These fixed protocol encodings have no implicit tags or alternate decoder.

`create_fold` opens `h − h(z)e_0`, self-checks its succinct equation, and returns
an undecided accumulator. `verify_fold` also returns an undecided accumulator;
call `decide` before accepting it. The decision uses the independent complete
MSM kernel. All Horner joins are complete, including identity intermediates.

At least one explicitly supplied source-k16 slot is required. Short-only
callers add `AccumulatorT::trivial(...).as_input()` as an additional fixed slot
in their bound input list. No function silently appends a slot. This prevents
the necessarily zero first L point of a non-hiding, short-only polynomial.
Rare identity messages and zero round challenges are errors, not panics or
implicit salt retries.

`FoldInput::corrected` computes a deciding replacement, requires a different G,
and retains the original claim. The consuming relation must still authorize
Corrected mode and bind both claims; this native helper does not authorize burn.

The prover owns at most 8 MiB of polynomial, powers and generator work vectors
at k16, borrows pinned parameters, and admits bounded generator chunks and MSM
scratch through `FoldConfig`. No runtime environment switch controls behavior.

The full k16 adversarial tests are ignored in the default debug suite. Run
`cargo test -p iroha_plonk_recursion --release -- --include-ignored` to exercise
both curves, fixed proof/output vectors, input binding and independent decisions.

`verifier` follows the descriptor's transcript, expression, opening and IPA
tables. Hard mode requires the exact verdict; soft mode checks every decode and
equation and returns a fixed source-k dummy on failure. `accumulation_circuit`
uses the same arithmetic and transcript lanes for local folds. Incoming mode
selection binds the selected k, point and challenges: Trivial uses the pinned
k16 accumulator, while Accept and Corrected retain the original source k.

`obligation` constrains the global incoming-mode rule and enumerates the fixed
per-operation ledger. Consuming relations still bind each slot's authenticated
source, mode and claim and must close both curves' generator decisions.

TODO: complete relation-level composition and obligation-ledger integration,
independent reference-verifier fixtures,
and the M4 Fiat–Shamir/composition security review before artifact qualification.
