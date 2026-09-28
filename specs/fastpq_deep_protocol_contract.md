# FASTPQ masked DEEP protocol contract

Source contract: 2026-09-28. The normal-library
[`offline_compact`](../crates/fastpq_prover/src/backend/offline_compact.rs)
quantity producer and verifier select this single fixed profile. Core transfer
proofs and AXT envelopes use canonical artifacts with bounded verification.
This is an implementation contract, not evidence that complete proof generation,
cryptographic or deployment qualification has passed. A verifier result grants no execution authority,
source finality or AXT spend authorization. The implemented producer samples
trace, quotient and independent composition masks. The complete construction
still has no independently qualified zero-knowledge claim. Current evidence and remaining release
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
| Exclusive FRI degree bounds | `[131072,8192,512,64,8,2]` |
| Initial queries / sampled candidates | 64 distinct positions / 74 candidates |

The [public-column owner](../crates/fastpq_prover/src/backend/compact_public_columns.rs)
fixes the 342-to-301 projection and evaluates known polynomials at the actual
extension points. Each retained base polynomial is
`A_j(X)=C_j(X)+(X^N-1)r_j(X)`, where `C_j` interpolates the physical source and
`r_j` has 136 independent base-field coefficients. The 41 public columns are
unchanged. The complete numerator has exclusive degree bound 196,751; exact
4N-domain interpolation and division produce a quotient of degree `<131215`.
Every remainder coefficient must be zero.

The quotient is split by coefficients as `Q=Q0+X^N Q1`, then independently
masked as `Q0'=Q0+X^N T`, `Q1'=Q1-T`, with 65 Fp4 coefficients in `T`.
The resulting exclusive chunk bounds are 65,601 and 65,679: the high chunk is
not truncated to the split length. Independently sampled `R` has 131,072 Fp4
coefficients. Every chunk and composition fits the fixed `<2N` FRI envelope.
The [construction note](fastpq_deep_hiding_construction.md) distinguishes the
finite-opening rank argument from the outstanding complete FRI/Fiat–Shamir
privacy and soundness obligations.

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
| 2 | 923 independent Fp4 alphas | 29,568 | Joined Q0/Q1/R root |
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
`(t_0,X*t_0,t_1,X*t_1)`. The composition is `R` plus their power batch with weights
`lambda,lambda^2,...,lambda^606`; only the independent mask has weight one. Both shifts are required for the reconstructed degree
obligations; dropping them is a different protocol.

[`deep_prover`](../crates/fastpq_prover/src/backend/deep_prover.rs) constructs this
composition in coefficient space with exact division before evaluating its LDE.
It computes the AIR numerator on the shared 4N interpolation domain and exactly
divides by `X^N-1`; no zero-mask adapter or pointwise 8M AIR replay is used.
Private coefficient/evaluation buffers use the existing erased-storage owner.
Base trace replay retains the source, coefficients, masks and one N-row stripe;
it visits 128 stripes per complete row pass. The numerator uses only its four
nested stripes. Q0/Q1/R and all FRI layers use coefficient replay too. Streamed
Merkle stacks retain exact queried frontiers and compare replayed roots before
emitting openings. No complete 20,199,768,064-byte base LDE is retained.

The fixed trace replay subtotal is 499,759,968 bytes, including borrowed source,
coefficients, one stripe, entropy and maximum selected-row storage. The whole
`ProducerPlan` additionally charges quotient/FRI coefficients, active tree and
hash buffers, both full public prefix caches and codec/self-verification buffers.
It preflights payload, arithmetic/inspection work, hash calls and proof bytes
before private transforms. The offline wrapper also checks source conversion,
private SMT, bundle and decode budgets. These are checked payload/work charges,
not RSS or latency bounds. Defaults allow a 2 GiB segment charge and 2^42
structural work units; an oversized plan fails before private computation.

The September 28 fixed-SMT preflight fixture reports 1,065,090,768 payload bytes,
3,468,335,009,584 structural work units and 69,362,447 hash calls. These are
checked plan charges for that public context, not measurements of a complete
proof attempt. The [native validation record](../docs/history/2026-09-28/fastpq-masked-native-validation.md)
separates the passing library, kernel and actual Metal tests from the outstanding
full-size producer execution.

Fresh entropy comes from an explicit `TryCryptoRng`; the normal offline wrapper
uses `OsRng`. Failed attempts do not reuse masks. CPU and required-device policies
select bulk leaf hashing only; streamed parents and transcript hashing use CPU.
Required-device availability is checked with public input before private-tree
work, entropy and transforms, and actual dispatch checks quarantine again.
There is no implicit CPU fallback for required-device failures. Leaves are
prepared in fixed batches of at most 32, independent of worker count. No hardware
or whole-prover performance qualification follows from this dispatch wiring.

## Bounded wire verification

The sole child frame is `fastpq_prover::deep_compact::MaskedCompositionProofV1` in
[`deep_proof`](../crates/fastpq_prover/src/backend/deep_proof.rs). It carries row and
joined Q0/Q1/R roots, six FRI/terminal roots, complete OOD answers, queried rows
and Q0/Q1/R triples, minimal sibling frontiers, five sets of complete strided FRI
fibers and the complete 128-value terminal. Row cells are fixed canonical u64
fields. Each FRI fiber has one arity byte followed by exactly that many raw
canonical Fp4 values; no vector count or per-value framing is accepted. Every
table index and frontier is derived from transcript queries.

[`deep_engine::verify_committed`](../crates/fastpq_prover/src/backend/deep_engine.rs)
performs one bounded canonical decode, the OOD identity, authenticated opening
checks and 320 fold checks (five per original query). Fiber `j` at length `M`
and arity `r` contains positions `j+k*(M/r)`, not adjacent positions. Domains
advance by the `r`th-power map. The entire authenticated terminal must represent
one polynomial of degree `<2` on the folded coset. Both linear terminal
coefficients are retained and all values are checked. A singleton terminal tree has its required
duplicate-child parent.
The verifier constructs no private witness, trace, FFT or full LDE.

The fixed DTO upper envelope is 502,895 bytes, below the 512 KiB child target;
this shape bound is not an end-to-end proof-generation measurement. Decoding
intersects the caller's allocation allowance with 8 MiB and retains stricter
outer scopes. The enclosing ordinary/AXT carrier accumulates bytes, statement
bytes, 64 queries per segment and all decode charges. It publishes ordered row
roots only after every child verifies. Two maximal children use 1,005,790 bytes;
actual context and framing must still fit the independently enforced bundle and
artifact limits. No arbitrary-size batch is promised to fit 1 MiB.

The metadata ID is SHA-256 of the canonical
`fastpq_prover::deep_compact::QuantityArtifactProfileV1` descriptor in
[`compact_artifact`](../crates/fastpq_prover/src/backend/compact_artifact.rs).
It binds the exact geometry, ten tapes, field/hash parameters, child-frame schema,
quantity schemas and both ordinary/AXT relation identities. The profile identity
also binds the joined mask oracle, affine masking batch,
`fri-degree2n:terminal-degree2-128` and `fixed-fri-fiber-wire:v1`.
The previous profile ID and child layout are not accepted alternatives. Their
remaining engines and constructors exist only in predecessor test diagnostics; their conditional
[375-query analysis](fastpq_compact_typed_profile.md) does not qualify this profile.
