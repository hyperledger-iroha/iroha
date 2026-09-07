# Compact FASTPQ protocol contract

Source snapshot: 2026-09-06. This records the test-only compact protocol's
implemented contract, not a production admission or soundness qualification.
Production verification still requires replay. The prototype makes no
zero-knowledge claim. Capacity, measured proof sizes and candidate profiles
belong in [the profile analysis](fastpq_compact_profile_analysis.md).

## Caller and algebraic inputs

[`FixedAir`](../crates/fastpq_prover/src/backend/compact_protocol.rs) supplies
the exact column order, numerator-slot order, circuit identity, public statement
bytes and evaluation function. These are trusted verifier inputs, never a
schema or program selected by proof bytes. Geometry fixes `N` to a supported
power of two, `L=8N`, width `1..=512`, and `1..=1024` numerator slots. The
[domain check](../crates/fastpq_prover/src/backend/fixed_domain.rs) verifies
exact root orders, subgroup agreement and a disjoint nonzero evaluation coset.
Write `D={a*g^i : 0<=i<L}`, `H=<g^8>` and `Z_H(X)=X^N-1`.

Base cells are canonical elements of `F_p`, `p=2^64-2^32+1`; all aggregation
and FRI challenges/values use the full
[`K=F_p[u]/(u^4-7)`](../crates/fastpq_prover/src/field.rs).
The relation contract requires each numerator, after substitution of
degree-below-`N` trace columns, to have degree below `3N`. Honest divisibility
by `Z_H` then gives quotient degree below `2N`. The interface does not prove
this degree obligation for an arbitrary Rust implementation.

The [public-transfer constructor](../crates/fastpq_prover/src/backend/compact_public_transfer.rs)
derives SMT ports from validated public facts and compares all seven
caller-expected `PublicIO` fields: `dsid`, `slot`, `old_root`, `new_root`,
`perm_root`, `tx_set_hash` and `ordering_hash`. The
[public facade](../crates/fastpq_prover/src/backend/compact_public_api.rs)
requires ordinary and AXT transfer semantics at separate entry points.
[AXT](../crates/fastpq_prover/src/backend/compact_axt_air.rs) additionally binds
the complete context produced by shared public-fact validators. These checks
do not establish execution authority, permission membership or source finality;
touched-balance roots are not authenticated consensus state roots. Outer AXT
expiry, handle/replay, amount-resolution and post-proof commitment checks remain
caller obligations.

## Committed objects and transcript order

The prover interpolates base columns on `H`, evaluates them on `D`, and commits
complete LDE rows. Verification never reconstructs those columns or their FFTs.
The committed objects are:

| Object | Leaf payload | Merkle role |
| --- | --- | --- |
| Row oracle `A` | All `w` base cells in column order | `AirTrace` |
| Mixed oracle `T` | One Fp4 value | `Lde` |
| Quotient oracle `Q` | One Fp4 value | `AirComposition` |
| FRI layer `V_r` | Binary strided pair; complete terminal vector at the last layer | `Fri(r)` |

[Leaf and node hashing](../crates/fastpq_prover/src/backend.rs) uses all six
canonical Digest384 coordinates. Leaf values are concatenated little-endian
base/Fp4 coefficients in one framed byte field. Nodes frame the left and right
48-byte digests as two separate fields. Typed domains bind catalog, protocol,
profile, role, phase, level, index and counter: ordinary leaves have level and
counter zero; FRI leaves use level `r` and counter zero; FRI internal nodes use counter `r` and
their actual tree level. The final singleton tree duplicates its leaf in the
root hash.

The exact [compact transcript](../crates/fastpq_prover/src/backend/compact_protocol.rs)
and [shared replay](../crates/fastpq_prover/src/backend/compact_protocol/shared_openings.rs)
perform this sequence:

1. `Transcript::initialise` receives default/zero `PublicIO`,
   `FASTPQ_FINAL_V1.name`, version `1`, and
   `fastpq:prototype:compact-single-phase:v1`. Its canonical Norito frame binds
   the transcript schema string, parameter/profile names, grinding bits,
   field/extension/hash identities, digest width, root geometry, coset, FRI
   arity/blowup/reduction/query settings, degree expansion and terminal size.
2. Append `compact:fixed-schema-and-statement`: canonical Norito tuple
   `(protocol_tag, circuit_identity, N:u32, L:u32, width:u32,
   numerator_count:u32, 2:u32, statement:Vec<u8>)`. The actual caller public
   statement is bound here despite the zero initialization placeholder.
3. Append `compact:full-row-root`; derive one Fp4 coefficient per column with
   `compact:column-mix:{j}`.
4. Append `compact:mixed-root`; derive one Fp4 coefficient per numerator with
   `compact:constraint-alpha:{k}`.
5. Append `compact:quotient-root`; derive `rho` and `sigma` with
   `fastpq:v1:joint-fri:trace` and `fastpq:v1:joint-fri:shifted-trace`.
6. For every nonterminal round, append `fastpq:v1:fri_layer:{r}`, then derive
   Fp4 `beta_r` with `fastpq:v1:beta:{r}`. Append the terminal root with
   `fastpq:v1:fri:final`; no terminal beta is sampled.
7. Derive queries with `fastpq:v1:query_index:{counter}` only after all roots.

Every append/challenge hashes the prior transcript state, tag, and monotonic
counter under separate transcript phases. Each challenge replaces that state;
an Fp4 challenge takes digest lanes 0 through 3. This specifies the algorithm,
not a proof that its outputs are independent uniform random variables. The
profile has zero grinding bits. Shared wire tables/frontiers are not new
transcript messages: they encode openings of these same roots. Their nominal
Norito schema is `fastpq_prover::compact_prototype::SharedProofV1`, distinct
from both the prototype `SinglePhaseProofV1` and production proof schema.

## Queries, authentication and verified equations

`sample_queries` targets `min(136,L)` distinct indices. For each digest it
considers all six base-field lanes, rejects candidates at or above
`p-(p mod L)`, reduces accepted candidates modulo `L`, and deduplicates with a
sorted set. The shared sampler now allows at most `max(64,8*min(target,L))`
digest draws: 1,088 for the current 136-query profile. It rejects nonempty
domains above `p`, desired counts above the supported 512 ceiling, exhausted
draw budgets and exhausted transcript counters with explicit errors. Zero
domain/target retains its empty, zero-draw behavior. A successful final allowed
draw is accepted; failure returns no partial index set or fallback. Existing
successful tags, lane consumption and transcript state are unchanged. This
engineering ceiling does not establish a completion probability or security
bound; conditioning on its stopping rule remains a qualification obligation.
The implementation and injected-source regressions pass the 878-test integrated
suite. The full ordinary-transfer proof remains byte-identical to its retained
pre-change fixture. The [conditional sampler lemma](fastpq_compact_query_sampler.md)
proves uniform subsets conditioned on successful stopping and exact rational
abort bounds under iid ideal digest draws. Concrete hash, attempt selection
and qROM conditioning remain open.

For the resulting sorted set `I`, shared verification derives:

- Row indices `I union {(i+8) mod L : i in I}`; mixed/Q indices exactly `I`.
- At round length `L_r=L/2^r`, group indices obtained by reducing the previous
  indices modulo `L_r/2` and deduplicating. A group `j` contains evaluations
  at indices `j` and `j+L_r/2`, not neighboring indices.
- One complete four-value terminal vector in natural index order.

Every supplied index set and minimal sibling count must equal the derived
set/plan. [Multiproof plans](../crates/fastpq_prover/src/backend/merkle_multiproof.rs)
derive frontier coordinates and ordering; the wire supplies no node IDs or
unused siblings. All table values authenticate to the typed roots before the
equations below. The singleton terminal root needs zero supplied siblings,
while retaining the duplicated-leaf hash convention. The converter from
`SinglePhaseProofV1` checks repeated rows/groups, every legacy path coordinate,
and discarded index/fold/terminal fields before omitting them; conversion alone
does not validate the AIR or terminal degree.

At each queried point `x=a*g^i`, verification checks, in `K`:

```
T(x) = sum_j lambda_j * A_j(x)
Q(x) = sum_k alpha_k * C_k(x, A(x), A(g^8*x)) / (x^N - 1)
V_0(x) = Q(x) + (rho + sigma*x^N) * T(x)
```

The [quotient domain](../crates/fastpq_prover/src/backend/air_quotient.rs)
computes the nonzero denominator exactly. Numerator count and every base-field
residue must be canonical. Public masks are evaluated at `x`; LDE array indices
do not select base-row opcodes. The compact hash/SMT ledgers' degree accounting
is recorded in the profile analysis, including direct interpolation of combined
fixed masks.

For an authenticated round pair `(v_+,v_-)` at `(x,-x)`, the fold is
`(v_++v_-)/2 + beta_r*(v_+-v_-)/(2x)`. Both domain generator and offset square
after each round. Every original query retains its own index/value carry even
when later groups coincide. Each carry must equal the selected pair member,
then the computed fold, then its terminal entry. The exclusive initial degree
bound `2N` divides exactly by two per reduction, reaching `1` at terminal size
four. Verification checks the degree of the entire authenticated terminal
vector, not merely its sampled entries. See the shared verifier and
[`proof::compact_fri_support`](../crates/fastpq_prover/src/proof.rs).

## Admission and evidence boundary

The [raw codec](../crates/fastpq_prover/src/backend/compact_protocol/shared_openings/codec.rs)
checks the caller's frame-byte ceiling before header/CRC work. Geometry-derived
uniform sequence and cumulative element limits, a 32 MiB Norito allocation-charge
ceiling, depth 16 and a frame-length field ceiling bound decoding.
`decode_canonical_with_limits` validates schema/header/layout/checksum and exact
consumption, then compares canonical re-encoding without allocating another
frame. Exact dimensions and all base/Fp4 coordinates pass typed preflight before
Fiat-Shamir processing. Allocation charges are not exact RSS. Default 512 KiB
and explicit diagnostic limits remain distinct caller policies.

Source tests exercise mutation rejection, shared/legacy transcript and opening
equivalence, wrong terminal degrees, field/coset arithmetic, canonical framing,
resource errors and alternate ambient layouts. Those tests and retained full
proof diagnostics are implementation evidence; their existence or successful
execution is not a soundness reduction or approval of the query profile.

## Mathematical obligations still open

[`JointFriBatch`](../crates/fastpq_prover/src/backend/joint_fri.rs) contains an
exact-degree lemma: for fixed degree-below-`L` interpolants `Q,T`, the interpolant
of `J=Q+rho*T+sigma*X^N*T` on `D` detects violation of `deg(Q)<2N` or `deg(T)<N`
except with probability at most `1/|K|`, **conditional on independent uniform
`rho,sigma`**. A violating high coefficient is a nonzero affine polynomial in
those challenges. The unshifted term is necessary because multiplication by
`X^N` alone can wrap modulo `X^L-a^L` and hide high trace degrees.

The [primary-source applicability map](fastpq_compact_soundness_sources.md)
identifies the exact theorem hypotheses and mismatches for the remaining
reduction. In particular, no DEEP-FRI bound applies to the current protocol,
which has no out-of-domain opening step. The [conditional row-linkage lemma](fastpq_compact_row_linkage.md) additionally
establishes base-field recovery, common current/next agreement, exact-degree
numerator batching and the fixed bad-set query bound under explicit premises.
The [conditional formal AIR interactive reduction](fastpq_compact_air_bound.md)
now supplies common row and quotient recovery with agreement threshold `11/16`,
including the actual current/next intersection and initial FRI linkage. Its
exact arithmetic reaches a conditional `2^-128` total at 237 queries; the
current 136 queries do not reach that bound. It assumes correct AIR semantics
and numerator degree below `3N`, perfectly bound oracles and independent uniform
interactive challenges. These results leave the following protocol obligations:

- Review the actual AIR semantics and degree ledger against the conditional
  reduction, including public selectors and all ordinary/AXT constraints.
- Externally review the conditional reduction for these exact degrees, binary
  strided folds, shared unique queries and full terminal check. The
  [round-by-round state lemma](fastpq_compact_round_by_round.md) covers the ideal
  grouped-message IOP and partial oracle words; connect it to the actual
  multi-call challenge expansion before invoking a compiler theorem.
- Analyze adaptive Fiat-Shamir composition in the required qROM setting,
  including post-commitment challenge derivation, the six-lane query sampler,
  repeated proof attempts and a reviewed sampler runtime/stopping policy.
- Justify the concrete six-lane hash assumptions and multi-target composition
  across Merkle commitments, transcripts, profiles and concurrent proofs.
  Output width, domain separation and canonical framing alone supply no
  aggregate security bound.

The [conditional FRI component bound](fastpq_compact_fri_bound.md) now derives
an explicit interactive rejection bound for an initial committed oracle at
distance greater than 0.49, with ideal binding and fresh uniform randomness.
Its weighted surviving-query argument keeps commit errors separate from query
repetition. Its stronger distance premise is not supplied by the complete AIR
reduction, which uses the separate `11/16` threshold and passing-set measure.
The two numerical bounds cannot be substituted for each other. Compiling the
complete AIR result to the concrete transcript remains a separate obligation.

Qualification must combine these with the reviewed complete relation and
authenticated caller integration. Neither 136 queries nor a larger diagnostic
byte allowance establishes the required aggregate security target.
