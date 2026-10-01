# FASTPQ masked DEEP protocol contract

Source contract: 2026-09-30; q77 native qualification pending. The normal-library
[`offline_compact`](../crates/fastpq_prover/src/backend/offline_compact.rs)
quantity producer and verifier select this single fixed profile. Core transfer
proofs and AXT envelopes use canonical artifacts with bounded verification.
This implementation contract does not establish cryptographic or deployment
qualification. A verifier result grants no execution authority,
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
| Initial queries / sampled candidates | 77 distinct positions / 87 candidates |

The [public-column owner](../crates/fastpq_prover/src/backend/compact_public_columns.rs)
fixes the 342-to-301 projection and evaluates known polynomials at the actual
extension points. Each retained base polynomial is
`A_j(X)=C_j(X)+(X^N-1)r_j(X)`, where `C_j` interpolates the physical source and
`r_j` has 162 independent base-field coefficients. The 41 public columns are
unchanged. The complete numerator has exclusive degree bound 196,803; exact
4N-domain interpolation and division produce a quotient of degree `<131267`.
Every remainder coefficient must be zero.

The quotient is split by coefficients as `Q=Q0+X^N Q1`, then independently
masked as `Q0'=Q0+X^N T`, `Q1'=Q1-T`, with 78 Fp4 coefficients in `T`.
The resulting exclusive chunk bounds are 65,614 and 65,731: the high chunk is
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
[`compact_sha3`](../crates/fastpq_prover/src/backend/compact_sha3.rs) prefix/body framing
with opaque SHA3-256 commitments and atomic SHAKE256 raw tapes. The complete framed context remains
in every logical hash input; its reusable prefix is not a substituted digest.

| Message | Decoded whole tape | Bytes | Following commitment |
| --- | --- | --- | --- |
| 1 | Dummy | 32 | 301-column row root |
| 2 | 923 independent Fp4 alphas | 29,584 | Joined Q0/Q1/R root |
| 3 | One Fp4 OOD point `z` | 80 | All 604 OOD answers |
| 4 | One Fp4 batching scalar `lambda` | 80 | Initial composition FRI root |
| 5..8 | One Fp4 beta each | 80 each | Next FRI root |
| 9 | Fifth Fp4 beta | 80 | Complete terminal root |
| 10 | 77 sorted distinct positions | 744 | None |

Every successful transcript materializes ten whole raw tapes totaling 30,920 bytes.
Raw words are arbitrary bytes: noncanonical field words are rejected within each
finite tape and are never reduced or replaced with extra output. All rejected
and unused bytes enter the following chain commitment. OOD answers are hashed
in current-row, next-row, quotient-half order. A base-field OOD point aborts.
The final 744-byte tape contains 93 raw words and must yield all 87 accepted
field candidates before selection of the first 77 distinct positions. Wrong
phase, finite exhaustion or malformed message permanently aborts the attempt.
The fixed primitive and framing still require complete concrete transcript,
Fiat–Shamir/qROM, soundness and hiding qualification.

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
nested stripes. Q0/Q1/R and all FRI layers use coefficient replay too. The first
commitment passes retain internal Merkle nodes under the same immutable attempt
context. Once the transcript chooses queries, replay regenerates only stripes
containing queried leaves or leaf-level siblings, including every coordinate of
FRI fibers. Each opening reconstructs the original root with the existing
canonical multiproof before exposing values. Entropy, challenges, leaf/parent
framing and proof encoding remain unchanged. No complete 20,199,768,064-byte base
LDE or full leaf-digest array is retained.

The whole `ProducerPlan` charges source, coefficients, masks, maximum selected
rows, active stripe, quotient/FRI coefficients, all retained internal-node caches,
coverage, shared immutable prefixes, canonical bodies, executor scratch and
codec/self-verification buffers. SHA3 nodes have 32 bytes. Keccak permutation
charges distinguish logical H/G queries from internal work, including public
readiness KATs and retained device pools. Native current-plan measurements are
pending; old six-lane plan totals do not describe this candidate. The prior full
replay work remains charged even when only selected stripes are evaluated.
It preflights payload, arithmetic/inspection work, hash calls and proof bytes
before private transforms. The offline wrapper also checks source conversion,
private SMT, bundle and decode budgets. These are checked payload/work charges,
not RSS or latency bounds. Defaults allow a 2 GiB segment charge and 2^42
structural work units; an oversized plan fails before private computation.

Internal-node retention, coverage, pending owners and opening scratch are
charged before work. Root/frontier, erasure and independent proof verification
controls remain required. Measured proof resources and source scope are recorded
in [production readiness](fastpq_production_readiness.md#captured-proof-evidence);
checked payload charges are not measurements of peak RSS.

Fresh entropy comes from an explicit `TryCryptoRng`; the normal offline wrapper
uses `OsRng`. Failed attempts do not reuse masks. CPU and required-device policies
select bulk leaf and independent lower-tree parent hashing. The ordered upper
tree stack and transcript hashing use CPU. Parent batching preserves every
natural level/index and reuses caller-owned clearing digest slots.
Required-device availability is checked with public input before private-tree
work, entropy and transforms, and actual dispatch checks quarantine again.
There is no implicit CPU fallback for required-device failures. Leaves are
prepared in fixed batches of at most 1024, independent of worker count; the
shared plan charges canonical bodies, frames, ordered parallel job results and
executor buffers before allocation. No hardware
or whole-prover performance qualification follows from this dispatch wiring.

## Bounded wire verification

The sole child frame is `fastpq_prover::deep_compact::MaskedCompositionProofV1` in
[`deep_proof`](../crates/fastpq_prover/src/backend/deep_proof.rs). It carries row and
joined Q0/Q1/R roots, six FRI/terminal roots, complete OOD answers, queried rows
and Q0/Q1/R triples, minimal sibling frontiers, five sets of complete strided FRI
fibers and the complete 128-value terminal. Row cells are fixed canonical u64
fields. Each FRI fiber has one arity byte and arity-minus-one raw canonical Fp4
values. The smallest known incoming coordinate is omitted deterministically;
the verifier reconstructs it, checks every shared incoming coordinate and
hashes/folds the complete original fiber. No proof-selected omission, vector
count or per-value framing is accepted. Every index/frontier is derived.

[`deep_engine::verify_committed`](../crates/fastpq_prover/src/backend/deep_engine.rs)
performs one bounded canonical decode, the OOD identity, authenticated opening
checks and at most 385 fold checks. Shared fibers preserve every distinct
incoming equality before authentication and deduplication; the exact count is
77 plus the distinct fiber counts in rounds zero through three. Fiber `j` at length `M`
and arity `r` contains positions `j+k*(M/r)`, not adjacent positions. Domains
advance by the `r`th-power map. The entire authenticated terminal must represent
one polynomial of degree `<2` on the folded coset. Both linear terminal
coefficients are retained and all values are checked. A singleton terminal tree has its required
duplicate-child parent.
The verifier constructs no private witness, trace, FFT or full LDE.

The fixed DTO upper envelope is 500,084 bytes, below the 512 KiB child target;
this shape bound is not an end-to-end proof-generation measurement. Decoding
intersects the caller's allocation allowance with 8 MiB and retains stricter
outer scopes. The enclosing ordinary/AXT carrier accumulates bytes, statement
bytes, 77 queries per segment and all decode charges. It publishes ordered row
roots only after every child verifies. Two maximal children use 1,000,168 bytes;
actual context and framing must still fit the independently enforced bundle and
artifact limits. No arbitrary-size batch is promised to fit 1 MiB.

The metadata ID is SHA-256 of the canonical
`fastpq_prover::deep_compact::QuantityArtifactProfileV1` descriptor in
[`compact_artifact`](../crates/fastpq_prover/src/backend/compact_artifact.rs).
It binds the exact geometry, ten tapes, field/hash parameters, child-frame schema,
quantity schemas and both ordinary/AXT relation identities. The profile identity
also binds the joined mask oracle, affine masking batch,
`fri-degree2n:terminal-degree2-128` and `omit-first-known-fiber:v1`.
The previous profile ID and child layout are not accepted alternatives. Their
proof, codec, replay and transcript implementations are removed. Immutable old
identity bytes remain only as rejection fixtures. The conditional
[375-query analysis](fastpq_compact_typed_profile.md) is historical and does not
qualify this profile. Native generated profile/seeded/quantity proof pins remain
unavailable until reviewed outputs of the current source and binaries exist.
