# FASTPQ F07 doubled-degree terminal boundary — 2026-09-24

The inactive DEEP candidate now has the screened FRI exclusive degree sequence
`[131072, 8192, 512, 64, 8, 2]` over the unchanged
`[8388608, 524288, 32768, 4096, 512, 128]` domains and folds
`[16, 16, 8, 8, 4]`. Its sole transcript identity and canonical statement
context bind the new sequence; a root committed under the previous `<N` context
has a different hash. There is no fallback geometry or second production path.

The verifier checks all 128 terminal evaluations against one polynomial of
degree `<2` on the *folded coset*. It accepts an affine extension-field
polynomial there and rejects altered first or last values, index-linear values,
and quadratic values. The 64 sampled FRI chains remain checked against those
same authenticated terminal positions. This enforces this one degree obligation;
it does not establish the FRI proximity or concrete/qROM soundness bound.

The source-bound prover plan now uses the doubled degree when checking every
canonical mask and full 923-slot AIR degree. A half-trace base-field mask shape
has exact screened bounds: trace `98304`, numerator `262015`, conditional
quotient `196479`, and high quotient half `130943`, all below the relevant
`2N` limit. A full-trace mask shape instead produces high-half bound `196479`
and is refused. Extension-field masks cannot use the existing base-field row
wire, and any unmasked retained column still exposes private row values.
Every otherwise degree-compatible mask is also refused because independently
sampled base-field masks, quotient-half blinding, a committed composition mask,
and their complete transcript and disclosure proof have not been implemented.
The producer preflight cannot authorize a proof.

The current DTO remains 506351 bytes and has **no `R(x)` row value**. The
previous 508399-byte proposal still assumes an unimplemented 2048-byte inline
addition; it is a screen, not a produced or verified artifact. The 512 KiB
segment and 1 MiB AXT limits, production witness replay, and closed Core
compact admission remain unchanged. The new degree geometry may have different
soundness, privacy and resource behavior; independent review and full-size
measurement are required before those gates move.

Validation on the same `optimizations` checkout: `cargo test -p fastpq_prover
--lib deep_ -- --nocapture` passed **53/53** selected tests, including the
new folded-coset affine/negative controls, changed-context root commitment,
screened half-trace AIR bound, high-mask quotient refusal, and existing DEEP
codec/transcript/opening checks. `rustfmt --check --edition 2024` on the touched
Rust sources and scoped `git diff --check` passed. The unchanged geometry
screen script exited zero and its Python tests passed **9/9**. These are
test-only candidate checks; no full private proof, producer, Core compact
verification, source finality, AXT carrier, hardware run, or independent audit
was exercised.
