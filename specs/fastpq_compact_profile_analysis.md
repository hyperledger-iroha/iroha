# Compact FASTPQ profile capacity and qualification

Sizing refreshed: 2026-09-08; historical measurements below retain their
2026-09-06 snapshot context. This is an implementation-coupled sizing analysis,
not a production parameter approval. The repeated-opening prototype sized
below remains test code. The normal library now exposes fixed quantity
ordinary/AXT offline verification through `offline_compact`, using a distinct
375-query SHAKE shared-opening candidate; these tables do not size that route.
Production replay and its qualification gate remain in force. The required
target is aggregate 128-bit qROM security. No profile below has an established
bound for the complete implemented protocol.

## Geometry and established algebra

The complete one-delta relation in
[`compact_transfer_air.rs`](../crates/fastpq_prover/src/backend/compact_transfer_air.rs)
uses 342 base columns and 923 numerator slots: 597 hash-local, 83 hash-transition
and 243 SMT slots. One transfer delta means two ordered balance updates, each
with 32 levels and old/new node hashes: 128 hashes. Each hash occupies 408
logical rows and 104 constrained padding rows, so one delta fills all 65,536
physical rows. There is no unused row budget for a second delta. Public native
key/value/leaf hashing is separate, bounded by public input bytes; the private
trace hashes only internal SMT nodes.

For degree-below-N committed columns, the period-512 selector polynomials have
degree N-N/512. A hash numerator has degree at most 3N-N/512-2, and its satisfied
quotient therefore has degree below 2N. Every SMT numerator is trace-linear
times an independently interpolated known polynomial of degree below N, hence
has degree at most 2N-2 and quotient degree below N. Multiplying two arbitrary
fixed masks is not covered by this bound; combined masks must be interpolated
directly. See
[`fixed_schedule.rs`](../crates/fastpq_prover/src/backend/fixed_schedule.rs),
[`compact_hash_quotient.rs`](../crates/fastpq_prover/src/backend/compact_hash_quotient.rs)
and [`compact_smt_quotient.rs`](../crates/fastpq_prover/src/backend/compact_smt_quotient.rs).

The joint polynomial `J = Q + rho*T + sigma*X^N*T` batches the exact bounds
`deg(T)<N` and `deg(Q)<2N`. Its repository lemma concerns fixed interpolants and
independent Fp4 challenges. It does not by itself prove proximity, consistency
of arbitrary committed row oracles, or adaptive Fiat-Shamir security. The
full-Fp4 column and numerator batching, quotient identity, complete row
openings and joint FRI are implemented; their end-to-end security reduction
remains a separate obligation.

## Repeated-opening prototype size under current canonical Norito framing

[`CompactProof`](../crates/fastpq_prover/src/backend/compact_protocol.rs) carries
complete current/next rows per query, four separate Merkle paths per query,
and complete [`FriQueryOpening`](../crates/fastpq_prover/src/proof.rs) chains.
Paths, folded values and terminal evaluations are repeated across queries.
Let `w` be row width, `L=2^ell` the LDE size, terminal size `t`, and
`r=ell-log2(t)` binary reductions. Ignoring codec framing, one query contains:

```
row values             16*w
Merkle siblings        48*(4*ell + sum(ell-j-1, j=0..r-1) + 1)
mixed and quotient     64
FRI pairs and folds    96*r
terminal evaluations   32*t
explicit indices       12 + 8*r
```

The extra terminal sibling reflects the current duplicated-single-leaf Merkle
convention. Proof roots add `48*(r+4)` bytes. At w=342, ell=19, t=4, r=17,
this is 19,300 bytes per query before framing, including 247 siblings.

Framing is material: canonical default Norito uses compact per-field lengths,
u64 vector counts and per-element lengths. An Fp4 now has an exact 32-byte
canonical coefficient payload; its enclosing field or vector element supplies
the length prefix. The former 37-byte struct payload is not the current carrier.
For a payload of n bytes define `P(n)=n+varint_len(n)`. Then a homogeneous
vector is `V(k,n)=8+k*P(n)`, a default struct is the sum of `P(field_size)`, and
the current proof frame adds 40 bytes (its alignment adds no padding).
Writing that struct sum as `S(...)`, the exact nested payload lengths are:

```
round_j = S(4, 4, V(2,32), 32, V(ell-j-1,48))
fri     = S(4, 8+sum(P(round_j), j=0..r-1), 4, V(t,32), V(1,48))
query   = S(4, V(w,8), V(w,8), V(ell,48), V(ell,48),
            32, V(ell,48), 32, V(ell,48), fri)
frame   = 40+S(48, 48, 48, V(r+1,48), V(q,query))
```

Applying these rules to the exact structs, including every varint width, gives:

| Relation / candidate | N | Blowup / terminal | Queries | Raw payload bytes | Canonical frame bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Existing hash-only diagnostic, w=310 | 512 | 8 / 4 | 136 | 1,588,608 | 1,737,603 |
| Complete repeated-opening prototype | 65,536 | 8 / 4 | 136 | 2,625,808 | 2,826,491 |
| Diagnostic calculator minimum | 65,536 | 8 / 4 | 200 | 3,861,008 | 4,156,091 |
| Lower-memory qualification lane | 65,536 | 8 / 4 | 256 | 4,941,808 | 5,319,491 |
| Expanded-domain qualification lane | 65,536 | 16 / 8 | 160 | 3,270,768 | 3,511,011 |
| Largest-domain comparison lane | 65,536 | 32 / 16 | 128 | 2,778,608 | 2,974,531 |

The 136-query hash and complete-transfer lengths match both
`encoded_frame_len` and actual canonical encoding in
`canonical_wire_sizes_match_complete_hash_and_transfer_opening_shapes` at
1,737,603 and 2,826,491 bytes. Full typed-transfer proving and bounded
verification also passed at 2,826,491 bytes in
`complete_typed_transfer_verifies_after_private_witnesses_are_dropped`, using
an explicit 4 MiB diagnostic envelope; the replay resource default still rejects
this larger compact diagnostic.
Other rows remain source-derived projections, not implemented profiles or
latency evidence. For every row above and in the larger-capacity table below,
changing the Fp4 payload from 37 to 32 bytes leaves all enclosing varint widths
unchanged, reducing the frame by exactly `5*q*(2+3*r+t)` bytes. Future profiles
use the current repeated-opening layout for this comparison; a schema change
requires recomputing the table.

The legacy 512 KiB cap is impossible for this complete row-opening layout:
rows alone are 744,192 bytes at 136 queries and 1,094,400 at 200. A 4 MiB
envelope admits the current framed q=200 projection with 38,213 bytes of
headroom; this size result does not qualify that query count. Narrow
u32 base-row registers do not allow u32 LDE opening encodings: evaluated trace
polynomials range over the whole canonical Goldilocks field.

## Concrete engineering profiles to qualify

The lanes above compare resource choices, not claimed security strengths.
Keep N=65,536, width=342, constraints=923, Fp4, 48-byte commitments, binary
folding and no grinding fixed while obtaining a reviewed bound. For the
existing uncompressed wire, provision a **6 MiB proof envelope for the
blowup-8/q<=256 lane**, or **4 MiB for blowup-16/q<=160 or
blowup-32/q<=128 lanes**. These ceilings admit their source-derived shapes
with explicit framing headroom. They are proposed bounded validation
envelopes; they must not replace production limits until final schema,
security and measured resource review agree. The final accepted query count
must be exact and profile-bound, not any prover-selected value below a cap.

`FASTPQ_FINAL_V1` fixes q=136, maximum trace log 16, LDE log 19,
terminal 4 and maximum 17 reductions. The replay resource defaults separately
admit at most 256 transitions and derive 2,163,774 payload bytes and 2,372,261
framed bytes from their opening geometry. These replay bounds do not qualify or
activate the compact profiles discussed here. An implementation
must change the complete profile identity/parameters, transcript binding,
terminal rules, exact wire/preflight counts, limits and fixtures together.
Blowup 16 alone is not a supported improvement: the diagnostic inverse rate
is capped by terminal size 4, and the exact-degree implementation rejects
folding an exclusive bound of one further. The coherent comparison is
blowup 16 / terminal 8, or blowup 32 / terminal 16, each retaining 17 reductions
and terminal degree below one for N=65,536.

The raw 342-column LDE costs 1,434,451,968 bytes at blowup 8, twice that at 16
and four times at 32. The base columns cost 179,306,496 bytes. Current prover
preparation also retains 205,520,896 bytes for 49 fixed public LDE columns,
16,777,216 bytes of phase cycles and hash-mask data; it temporarily creates
25,690,112 bytes of fixed coefficients. Trace coefficient copies, Fp4 mixed/Q
oracles, FRI layers, Merkle trees and worker scratch are additional. These
component counts are not a peak-memory bound. Increasing blowup expands
prover work and memory even when fewer queries reduce verifier work.

For a generalized sequential program with D deltas, the proposed minimum
subgroup is `N=65,536*next_power_of_two(D)`. The following comparison keeps
blowup 8 and q=200 only to show scaling:

| D capacity | N | LDE rows | Reductions / roots | Raw trace LDE bytes | Framed proof projection |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 65,536 | 524,288 | 17 / 18 | 1,434,451,968 | 4,156,091 |
| 2 | 131,072 | 1,048,576 | 18 / 19 | 2,868,903,936 | 4,407,540 |
| 4 | 262,144 | 2,097,152 | 19 / 20 | 5,737,807,872 | 4,668,789 |
| 8 | 524,288 | 4,194,304 | 20 / 21 | 11,475,615,744 | 4,939,838 |

Only the one-delta program exists. Larger capacities require authenticated
inter-delta root boundaries, deterministic inactive-delta padding, generalized
public polynomials and exact capacity admission, new domain roots/logs and
FRI limits. Existing admission for 256 transition rows is not private compact
capacity. The separate offline per-delta proof bundle has roughly linear
proof bytes and verification work and explicit root chaining. It requires its
own aggregate security count; it does not increase this single-trace profile's
capacity.

## Merkle multipaths before a smaller wire target

A canonical multiproof helper derives the minimal sibling frontier from
trusted sorted query indices and verifies with bounded memory. Historical
2026-09-06 measurements of the test-only shared-opening prototype reported
972,311 canonical bytes for the full hash relation, 1,608,631 bytes for the
complete transfer, and 10.870 seconds for shared transfer verification in a
concurrent local test-profile run with an explicit 4 MiB diagnostic cap.
Those byte counts and timings belong to that historical encoding and run;
they are not current 32-byte-carrier or 375-query candidate measurements.
Production wire admission is unchanged. Shared openings use row leaves at
`I union (I+blowup mod L)`,
mixed/Q leaves at I, and FRI pair leaves at each derived layer index. Include
each terminal vector once, and derive folded values from the authenticated
next-layer pair or terminal vector. These changes can preserve all algebraic
checks while removing repeated material. They require a new reviewed wire,
not a generic byte compressor around unbounded decoded vectors.

For m distinct leaves in a height-h binary tree, let n_j be the number of
occupied ancestors at level j. The exact number of missing siblings is
`2 + sum(n_j,j=1..h-1) - m`, and
`n_j <= min(m,2^(h-j))`. This bounds multipath bytes without an independence
assumption. With at most m leaves, maximize this expression over `1..m`:
substituting m directly can underestimate the bound near full trees.
At N=65,536/blowup8/q200, this gives at most 20,438 sibling digests across
the row, mixed, quotient and all nonterminal FRI trees. There are at most
2,452 distinct FRI pairs. The resulting conservative payload bound is
2,246,288 bytes, excluding indices and codec framing. This is a useful design
budget, not the size of an implemented codec. Shared leaves may improve it.

Reject duplicates, extra/unused nodes, missing nodes, wrong tree/round/level
positions, malformed counts and noncanonical fields before expensive work.
Derive every required leaf position from the transcript and fixed geometry;
the proof must not choose an easier subset. Preserve each leaf hash's role,
round, index and exact payload encoding and the terminal tree convention.
Shared authentication must not reduce the number of logical AIR/FRI checks.
Bound node counts and allocation from the fixed query set before decoding
payloads. Add adversarial coverage for merged paths, cross-tree substitutions
and last-layer convergence, and establish byte parity for reconstructed
single paths. Multipaths can support a practical ~3 MiB exploration envelope
for q200, but actual canonical worst-case framing must establish the cap.

## Security accounting and qualification work

[`params.rs`](../crates/fastpq_isi/src/params.rs) implements a diagnostic sum
`T*Q^2 / 2^(q*log2(min(blowup/2,terminal))/2) + T^2*Q^3/2^384`, with T=54,
Q<=2^32 and a strict 2^-128 target. Under that model only, the minimum admitted
query counts are 200 at 8/4, 136 at 16/8 and 104 at 32/16. The comparison lanes
q256, q160 and q128 give sampling-term exponents of approximately 186.245,
170.245 and 186.245 bits, respectively. These numbers provide room to evaluate
additional terms; they are not substitute security reductions. Choosing 200
because the calculator passes would leave its explicitly documented blocker
unresolved.

The 136-query prototype's sampler rejects biased field residues, deduplicates a
`BTreeSet`, and returns sorted distinct indices. If a fixed bad set of b out
of L positions were sampled uniformly without replacement, its all-hit
probability would be `C(b,q)/C(L,q) <= (b/L)^q`. That combinatorial fact does
not prove that adaptive transcript-derived FRI acceptance has this bad-set
form. Initial distinct queries can also merge into the same FRI pair or later
layer; never count them as independent layer queries. Account for deriving
several indices from each six-lane transcript digest and for the actual
rejection/deduplication procedure in the final random-oracle argument.

Qualification must connect these specific row commitments, Fp4 batching,
shifted joint polynomial, fixed selectors, quotient identity, FRI folds and
terminal test to a concrete proximity and Fiat-Shamir bound. The original
[FRI paper](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ICALP.2018.14)
analyzes interactive proximity testing; its result is not an exact-degree
membership assertion for arbitrary committed oracles.
[Block et al.](https://eprint.iacr.org/2023/1071) analyze round-by-round
soundness of FRI/batched FRI and a route to Fiat-Shamir soundness for composed
protocols. FASTPQ still needs an explicit mapping to the chosen theorem's
conditions, including its particular joint batch and linked row oracle.
[Don, Fehr and Majenz](https://arxiv.org/abs/2003.05207) analyze multi-round
Fiat-Shamir in the QROM; a Sigma-protocol quadratic-loss slogan does not
establish this implementation's many-round quantitative loss.

The final review must account for actual field size p^4, digest range p^6,
challenge generation, aggregation errors, number of rounds, commitment
binding and all adaptive/multi-target losses. Six domain-separated lanes of
the same Poseidon construction require construction-specific review; writing
384 bits in a descriptor is not such evidence. T=54 and Q<=2^32 must match a
documented adversary and deployment scope, not merely fixture counts. The
external SMT uses Iroha's marked BLAKE2b-256 digest with 255 variable bits;
its required binding/preimage property must be qualified separately and is
not strengthened by the 384-bit STARK commitment or a larger query count.

After the complete reduction is fixed, choose the smallest practical lane
that meets its aggregate bound, freeze the exact first-release profile and
wire, then measure full native one-delta proving/verifying bytes, peak memory,
latency and cross-backend determinism. Public statement/root authorization,
production API/Core admission, decode-before-allocation limits and malformed
proof tests must use that same profile. Until those tasks complete, this
analysis establishes capacity and engineering candidates only.
