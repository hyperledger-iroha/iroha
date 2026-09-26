# FASTPQ DEEP offline protocol contract

Source contract: 2026-09-26. The normal-library
[`offline_compact`](../crates/fastpq_prover/src/backend/offline_compact.rs)
Quantity producer and verifier select this single fixed profile. This is an
implementation contract, not evidence that its complete proof generation,
cryptographic qualification or production integration has passed. Node admission
still uses replay; the offline success result grants no execution authority,
source finality or AXT spend authorization. The proof is unmasked and has no
witness-hiding or zero-knowledge claim. Current evidence and remaining release
gates belong in [production readiness](fastpq_production_readiness.md).

## Fixed relation and geometry

The sealed [`DeepRelation`](../crates/fastpq_prover/src/backend/deep_relation.rs)
bridge admits the complete SMT AIR and its ordinary/AXT prepared batch segment
wrappers. It binds the outer relation identity and exact complete statement;
the borrowed inner AIR must have identical statement bytes and geometry.
Artifact contents cannot select a verifier or replace independent expectations.
The public facade compares all seven PublicIO fields and the canonical statement
digest; AXT additionally compares complete binding, metadata, mirrors and remote
preimages. Every segment binds its ordinal, complete batch and ordered root chain.

[`deep_geometry`](../crates/fastpq_prover/src/backend/deep_geometry.rs) fixes:

| Item | Value |
| --- | --- |
| Base/extension field | `p=2^64-2^32+1`, `K=F_p[u]/(u^4-7)` |
| Trace rows `N` / complete AIR columns / numerator slots | 65,536 / 342 / 923 |
| Retained committed columns | 301; 41 public columns reconstructed independently |
| Evaluation rows `L` / blowup | 8,388,608 / 128 |
| Order-`L` root `g` | `0x35c4528b4aa62eb8` |
| Evaluation domain | `a<g>`, `a=FASTPQ_FINAL_V1.omega_coset` |
| Trace generator `omega` | `g^128 = FASTPQ_FINAL_V1.trace_root` |
| Ordered FRI arities | `[16,16,8,8,4]` |
| FRI domain lengths | `[8388608,524288,32768,4096,512,128]` |
| Exclusive degree bounds | `[65536,4096,256,32,4,1]` |
| Initial queries / sampled candidates | 64 distinct positions / 74 candidates |

The [public-column owner](../crates/fastpq_prover/src/backend/compact_public_columns.rs)
fixes the 342-to-301 projection and evaluates known polynomials at the actual
extension points. A trace column has degree `<N`; the combined AIR quotient has
degree `<2N` and is split by coefficients as `Q(X)=Q0(X)+X^N Q1(X)`, with both
halves degree `<N`. This does not split evaluation arrays into adjacent halves.

## Commitment and challenge order

[`deep_binding`](../crates/fastpq_prover/src/backend/deep_binding.rs) supplies the
fixed profile identity and `fastpq_prover::deep::StatementContextV1`. Its context
contains relation identity, public-column layout, complete geometry, field
parameters and original statement bytes. Identity length is 1..=256 bytes;
statement length is 1..=240 KiB. It uses the shared canonical
[`compact_v1`](../crates/fastpq_prover/src/backend/compact_v1.rs) prefix/body framing
and existing six-lane field digest owner. The complete framed context remains
in every logical hash input; its reusable prefix is not a substituted digest.

| Message | Decoded whole tape | Bytes | Following commitment |
| --- | --- | --- | --- |
| 1 | Dummy | 48 | 301-column row root |
| 2 | 923 independent Fp4 alphas | 29,568 | Paired quotient-half root |
| 3 | One Fp4 OOD point `z` | 48 | All 604 OOD answers |
| 4 | One Fp4 batching scalar `lambda` | 48 | Initial composition FRI root |
| 5..8 | One Fp4 beta each | 48 each | Next FRI root |
| 9 | Fifth Fp4 beta | 48 | Complete terminal root |
| 10 | 64 sorted distinct positions | 624 | None |

Every successful transcript materializes 637 six-lane blocks (30,576 bytes).
All tape coordinates, including unused suffixes, must be canonical. Each of the
nine chain updates binds the entire preceding tape and commitment. OOD answers
are hashed in current-row, next-row, quotient-half order. Sampling `z` in the
base field aborts that transcript; it is not retried within the same attempt.
The query decoder examines at most 74 of its 78 coordinates, rejects values
outside the largest multiple-of-`L` prefix of `F_p`, and keeps the first 64
distinct residues modulo `L`, sorted. Insufficient distinct values abort; no
additional block is requested. These rules alone do not establish ideal-oracle
uniformity or a concrete Fiat–Shamir soundness bound.

## OOD identity and low-degree composition

The verifier evaluates all 923 AIR slots once at `z`, reconstructing all 41 public
columns at `z` and `omega*z`. With the supplied retained answers it checks

```
sum_k alpha_k C_k(z, A(z), A(omega*z))
    = (z^N - 1) * (Q0(z) + z^N Q1(z)).
```

For retained column `j`, let `I_j` be the linear interpolant of its two OOD
answers. [`deep_composition`](../crates/fastpq_prover/src/backend/deep_composition.rs)
defines `h_j=(A_j-I_j)/((X-z)(X-omega*z))` and
`t_k=(Qk-Qk(z))/(X-z)`. The 606 components, in order, are
`(h_j,X^2*h_j)` for each of 301 columns, followed by
`(t_0,X*t_0,t_1,X*t_1)`. The composition is their power batch with weights
`1,lambda,...,lambda^605`. Both shifts are required for the reconstructed degree
obligations; dropping them is a different protocol.

[`deep_prover`](../crates/fastpq_prover/src/backend/deep_prover.rs) constructs this
composition in coefficient space with exact division before evaluating its LDE.
It computes the AIR numerator on the shared 4N interpolation domain and exactly
divides by `X^N-1`; no zero-mask adapter or pointwise 8M AIR replay is used.
Private coefficient/evaluation buffers use the existing erased-storage owner.
The 301 base LDE columns alone hold 20,199,768,064 bytes, before quotient, FRI and
tree storage. Explicit checked payload/work budgets are not RSS or latency bounds.

## Bounded wire verification

The sole child frame is `fastpq_prover::deep_compact::ProofV1` in
[`deep_proof`](../crates/fastpq_prover/src/backend/deep_proof.rs). It carries row and
paired-quotient roots, six FRI/terminal roots, complete OOD answers, queried rows
and quotient pairs, minimal sibling frontiers, five sets of complete strided FRI
fibers and the complete 128-value terminal. Row cells are fixed canonical u64
fields. Every table index and frontier is derived from transcript queries.

[`deep_engine::verify_committed`](../crates/fastpq_prover/src/backend/deep_engine.rs)
performs one bounded canonical decode, the OOD identity, authenticated opening
checks and 320 fold checks (five per original query). Fiber `j` at length `M`
and arity `r` contains positions `j+k*(M/r)`, not adjacent positions. Domains
advance by the `r`th-power map. The entire authenticated terminal must be
constant. A singleton terminal tree has its required duplicate-child parent.
The verifier constructs no private witness, trace, FFT or full LDE.

The fixed DTO upper envelope is 506,351 bytes, below the 512 KiB child target;
this shape bound is not an end-to-end proof-generation measurement. Decoding
intersects the caller's allocation allowance with 8 MiB and retains stricter
outer scopes. The enclosing ordinary/AXT carrier accumulates bytes, statement
bytes, 64 queries per segment and all decode charges. It publishes ordered row
roots only after every child verifies. Two maximal children use 1,012,702 bytes;
actual context and framing must still fit the independently enforced bundle and
artifact limits. No arbitrary-size batch is promised to fit 1 MiB.

The metadata ID is SHA-256 of the canonical
`fastpq_prover::deep_compact::QuantityArtifactProfileV1` descriptor in
[`compact_artifact`](../crates/fastpq_prover/src/backend/compact_artifact.rs).
It binds the exact geometry, ten tapes, field/hash parameters, child-frame schema,
quantity schemas and both ordinary/AXT relation identities. The previous profile
ID and child layout are not accepted alternatives. Their remaining engines and
constructors exist only in predecessor test diagnostics; their conditional
[375-query analysis](fastpq_compact_typed_profile.md) does not qualify this profile.
