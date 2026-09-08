# Conditional typed-compiler profile and resource arithmetic

2026-09-08. **Conditional ideal field-oracle arithmetic, not production
qualification.** The [current V1 framing](fastpq_compact_v1_framing.md) and
[protocol contract](fastpq_compact_protocol_contract.md) use the six-lane
`F_p^6` digest for H and fixed field-product G tapes. The underlying
[typed compiler](fastpq_compact_typed_compiler.md),
[adaptive-context](fastpq_compact_adaptive_context.md) and
[round-by-round](fastpq_compact_round_by_round.md) arguments retain their exact
premises. The field-block extension below is an internally reviewed conditional
result; source structural checks and exact arithmetic do not complete its
implementation mapping or concrete-security qualification.

The model charges the requested 54 external targets and one adversary budget
of `2^32` binary digest queries, including retries. The 375 proof-query positions
are a different parameter. Serialized canonical field coordinates are not
uniform binary strings. No original SHAKE instantiation assumption transfers.

## Exact result

**375 initial positions is the smallest count for which the displayed bound
certifies the requested conditional aggregate error below 2^-128.** At 375,

```
743/1024 < 54*conditional_bound*2^128 < 744/1024.
```

At 374 the corresponding interval is (1081/1024,1082/1024). Moreover, for every
q<=374, the compiler bound is at least

```
54*6*(2*2^32)^2 * binom(360447,374)/binom(524288,374) > 2^-128.
```

This proves minimality for this bound and model without assuming that the
resource-dependent upper bound is monotone. It does not prove attacks at any
smaller count, nor does it approve 375 after adding unqualified concrete-hash
or other security errors. At 376 the scaled interval is (510/1024,511/1024).
The commit-round bound becomes dominant at 396, limiting gains from further
query increases.

All decisions use exact Fraction/integer arithmetic. For A=6*T^2*delta,
B=2*K_weight and z=2^-128/54-A-B, the test is z>0 and z^2>4*A*B.
Displayed irrational bounds use an integer-square-root enclosure with grid
2^-384 and checked rational inequalities. No floating-point value certifies
passing or minimality.

## Verifier oracle work

Let P(d,m)=sum_(ell=0..d-1) min(m,2^ell). A depth-d binary tree with m selected
leaves has at most P(d,m) distinct reconstructed parent hashes. At each level
the selected ancestors number at most m and at most the available nodes.
This upper bound is achieved by maximally spread leaves in one tree; a sum of
different trees' maxima need not be jointly attainable, which is harmless
for an upper bound.

Its minimal sibling frontier has exactly parents-m+1 nodes, since the
reconstructed full binary subtree has leaves=selected leaves+siblings.
The sole terminal leaf follows the source's special rule: hash the leaf and
then one parent with the leaf as both children. It costs two H calls.

The source selects at most 2q complete rows (current and next), q mixed leaves,
q quotient leaves, and min(q,2^d) FRI group leaves at depths d=18,...,2.
The terminal has four values in one leaf. There are 21 committed roots,
21 chain H calls, 22 G calls and no implicit context-initialization H call:
the candidate uses the full context in its typed input and a constant anchor.

| Component at q=375 | Leaf H calls | Parent H calls |
| --- | ---: | ---: |
| Complete AIR rows | 750 | 7,773 |
| Mixed values | 375 | 4,261 |
| Quotient values | 375 | 4,261 |
| 17 FRI binary-group trees | 4,258 | 22,486 |
| Complete terminal | 1 | 1 |
| Total membership work | 5,759 | 38,782 |

Adding 21 chain calls gives **44,562 H calls**. The 22 whole G messages expand
931 separately addressable six-lane blocks, giving **45,493 physical digest
calls** per verifier. Thus

```
T = 2*(2^32+45,493) = 8,590,025,578.
```

Let `C=F_p^6`, and fix `B_j` blocks for message j. Model the complete bounded
digest-input domain by a uniform ideal function `F:D -> C`. Injective reversible
full-context framing, distinct message/block coordinates and disjoint H/G input
images allow exact regrouping:

```
G_j(x) = (F(e_j(x,0)), ..., F(e_j(x,B_j-1))) in C^B_j.
```

All malformed, out-of-range or other-protocol digest inputs within the declared
finite domain remain consistently answered auxiliary H entries. They are not
silently excluded from the collision image. Public internal-permutation access
is a separate concrete-instantiation obligation.

For a binary request at a block address, compute the complete tuple into a clean
ancilla, XOR the selected coordinate's canonical encoding, and uncompute. This
uses two group-oracle queries total; binary encoding does not incur another
factor of two. The exact basis identity, cleared ancilla and phase-free reversible
routing extend to coherent adaptive and superposed requests. The whole-tuple
adversary is stronger; the compiler applies to that simulator's atomic database,
not to a database of partially inserted physical blocks.

The finite abelian response groups have orders `s_j=p^(6*B_j)`. Replacing binary
response groups by these product groups requires the same context-uniform,
every-prefix error bounds and successful decoder laws. Nontrivial characters
still have zero sum. With the existing conditional state errors `epsilon_j`, use

```
delta = min(1, max(3*(T-1)/p^6, max_j(epsilon_j+(T-1)/p^(6*B_j))))
K_weight <= 44562/p^6 + sum_(j=1..22) 1/p^(6*B_j)
conditional_bound <= min(1, (sqrt(6*T^2*delta)+sqrt(2*K_weight))^2).
```

For the checked envelope the computed delta is below one, so the checker can
use the displayed untruncated maximum without weakening the upper bound. The
framing recognizer, full auxiliary domain, actual AIR errors and concrete
six-lane/related-instance/permutation residual still require qualification.

These counts are per segment. A bundle checking S segments must instead charge
all S expansions: T=2*(2^32+S*45,493), with K_weight at most S times the displayed
weight, plus any additional outer oracle calls. The separately reviewed
adaptive-context property can cover a false accepted child without a context
union factor when its exact family premises hold; otherwise an explicit
segment union is required. This artifact does not authorize multiplying the
one-segment resource cap or reinterpreting the 54 external targets as segments.

These are counts in the candidate formal compiler. Public-state authority
checks, unrelated hashes, wire CRC work, field/AIR arithmetic and actual
oracle-internal permutations are additional implementation costs, not omitted
memberships. If another implementation adds H calls for a context digest,
profile digest or auxiliary commitment, its extra edges and calls must be
included and this arithmetic rerun.

## Bundles and honest abort

`python3 scripts/fastpq/check_compact_bundle_profile.py` reproduces the conditional
bundle calculation with exact rational arithmetic and source geometry guards.
The family premise must ensure that any false accepted bundle has a false child
in the admissible family. The complete winning expansion contains every child's
typed cells; one extracted false child then witnesses the same global bad-database
property. Its comparison weight and verification queries include all children.
No additional outer H/G calls are assumed. Native public-state hashes and concrete
primitive errors still require their separate qualification.

| Segments | H / G-block calls | Group-query budget T | Scaled acceptance interval / 1024 | Honest-abort bound over 54 bundles |
| --- | ---: | ---: | ---: | ---: |
| 1 | 44,562 / 931 | 8,590,025,578 | (743,744) | < 2^-137 |
| 2 | 89,124 / 1,862 | 8,590,116,564 | (743,744) | < 2^-136 |
| 128 | 5,703,936 / 119,168 | 8,601,580,800 | (745,746) | < 2^-130 |

The acceptance intervals divide by 1024 and then multiply by 2^-128. The
source selects a fixed 401-canonical-coordinate tape using the separate chosen honest
abort envelope of 54 bundles times 128 segments, or 6,912 attempts. The 54
external acceptance targets and shared adversary budget remain unchanged.
A hypothetical 400-coordinate tape passes the 54-single-attempt abort bound,
but fails the 6,912-attempt bound. A permanent honest abort is not false
acceptance. This arithmetic neither approves 128-segment resource limits nor
reinterprets the external targets as segments. The checker rejects
assertion-disabled Python and out-of-range segment counts, pins one-segment
acceptance parity, and checks acceptance and honest abort for every bundle size
from 1 through 128.

## Fixed challenge tapes and honest abort

Every G block is an element of `F_p^6`; a successful ideal field-vector decoder
reads the required prefix in consecutive groups of four. Its coefficient-abort
probability is exactly zero under this ideal model. The concrete decoder still
checks canonical encoding of every tape word, including unused suffix words,
before interpreting any message. All raw words enter the following chain H.

| Message | Count | Required base coordinates | Field-product blocks | Tape bytes per message |
| --- | ---: | ---: | ---: | ---: |
| Initial dummy | 1 | 0 | 1 | 48 |
| Column mixing | 1 | 1,368 | 228 | 10,944 |
| Constraint mixing | 1 | 3,692 | 616 | 29,568 |
| Joint pair | 1 | 8 | 2 | 96 |
| FRI beta | 17 | 4 | 1 | 48 |
| Complete query subset | 1 | First 401 of 402 words | 67 | 3,216 |

The dummy's positive `F_p^6` tape preserves an empty mathematical message with a
nontrivial response group. It is not a uniform 384-bit string.

Write `p=kL+1`, `L=524288`. Each query label has exactly k preimages among the
accepted field words `0..p-2`; the sole rejected coordinate is `p-1`. Read exactly
401 candidate coordinates, reduce accepted coordinates modulo L and return the
first 375 distinct labels, sorted. Insufficient distinct labels permanently abort.
There are no extra blocks or retries. The unused 402nd coordinate must also be
canonical. Label permutations preserve the entire successful-tape distribution,
so successful subsets are uniform under iid uniform canonical field coordinates.
Before completion, rejection or an already-seen label has conditional probability
at most `(1+374*k)/p`; after completion define that indicator as zero. A failed
tape contains at least 27 non-new indicators, giving

```
eta_query <= binom(401,374)*((1+374*k)/p)^27.
```

The exact checker proves this bound below `2^-143` per attempt, below `2^-137`
for 54 attempts, and below `2^-130` for 6,912 attempts. Under this sufficient
formula, 399 candidates fail the 54-attempt certificate; 400 pass that smaller
union but fail the 6,912-attempt envelope. Every count 375..400 fails that larger
certificate, and 401 is the first passing count. This is not a true-abort lower
bound or a claim that the tape is optimally sized. The fixed complete geometry
identity and canonical final protocol tag remain those in the framing contract.

There are **44,688 G output bytes**, of which **41,472** enter subsequent chain
hashes. The final 3,216-byte query tape has no following root. The verifier
recomputes tapes; they are not transmitted in the proof. Attempts, including
aborted ones, remain charged to the adversary's shared query budget.

At q=375 the largest state error is the final query ratio, below 2^-202.
The commit-round maximum remains below 2^-214. The H collision/attachment
term is below 2^-349, and the comparison B is below 2^-367. These component
exponents are intermediate bounds, not production security-bit claims.

## Wire and internal work projection

The current canonical SharedProof layout uses a 40-byte Norito header,
compact field/element lengths, fixed u64 sequence counts, 48-byte digests,
32-byte encoded extension elements, 3,093-byte encoded complete-row elements,
71-byte mixed/quotient query elements, and 72-byte FRI group elements.
The exact formula agrees with the current Rust structural sizing assertion of
6,713,525 bytes. The checker has one canonical field carrier and no optional
retired-proof input. This equality does not execute Rust or authenticate a proof.

Projecting the same field layout to q=375 with minimal frontiers gives an
upper bound of **4,279,877 bytes per segment**. The corresponding maximal-byte
shape has sibling counts
7,024 row, 3,887 mixed, 3,887 quotient and 18,245 FRI digests.
These are not independent upper bounds on each sibling frontier: opening all
leaves can reduce a frontier to zero. The combined framed values/frontier
size, however, increases with the opened count m. A group contributes 73 bytes
including its element-length prefix; each sibling contributes 49. As m grows
by one, P(d,m) never decreases, so the sibling count can fall by at most one.
That loses at most 49 payload bytes and one compact-length-prefix byte, while
the group sequence gains 73 bytes. Its own prefix cannot shrink, so the
combined round body grows by at least 23 bytes; framing that larger body is
also monotone. Complete rows have a still larger per-element cost. The
certificate checks the exact framed formulas for every m through 512 in all
FRI depths and through 1,024 row openings. Thus maximal-byte shapes occur at
the stated count caps even where their frontier alone is smaller.

This is a sum of per-tree byte maxima and an exact size formula at those
maximizers. Retired-candidate measurements are prior-snapshot diagnostics in the
[production record](fastpq_production_readiness.md); the
[current framing contract](fastpq_compact_v1_framing.md) describes the six-lane
owner separately. The projection itself is not a measurement or a production
profile change.

Using the source's looser preflight ceiling of m*depth sibling entries gives
**6,713,525 bytes**. Such a largest shape can pass a structural sizing step
while failing the later exact frontier check; it is a relevant hostile decode
resource bound, not a valid proof maximum.

Both estimates exclude outer artifact/bundle framing, public statements and
any future fields required by the finalized schema. They do not supply a
Norito allocation ceiling: the later same-profile measurements in the framing
evidence separately record actual and loose-shape cumulative codec charges. A 4 MiB segment ceiling is already too small for
the valid upper projection. Neither default admission limits nor existing
bundle limits were raised.

The complete DTO has an unconditional raw value lower bound of
`375*342*8 + 375*64 = 1,050,000` bytes. It exceeds both the unchanged 512 KiB proof
and 1 MiB AXT ceilings before any framing, indices, roots or frontiers. The
750-row maximum shape contains 2,052,000 row bytes separately; that maximum is
not a universal lower bound. None of these calculations raises an admission cap.

The [prefix contract](fastpq_compact_v1_framing.md) defines owned immutable
canonical-prefix reuse. It changes physical repeated absorption, not logical
oracle inputs. Every body's permutation cost, canonical encoding, state copy,
field/AIR operation and actual memory allocation remains additional concrete
work. No end-to-end latency is extrapolated from digest-query counts.

## Reproduce and review

Run `python3 scripts/fastpq/check_compact_typed_profile.py` and
`python3 scripts/fastpq/check_compact_bundle_profile.py`. Both accept `--output
PATH`; outputs default to `target/fastpq-production-validation`. They require no
retained proof, PDF or network. The JSON binds complete source hashes before and
after the checks and the exact parameter table. Source mutations are checked in
memory, including wrong context/protocol/block indexing, query/rejection schedule,
geometry, terminal behavior and retired schema substitution. Assertion-disabled
Python is rejected, as are invalid counts and the removed fixture selector.

The finite controls check every nonempty subset through 16 tree leaves
(65,808 cases), 625 toy field tapes for conditional subset uniformity and rejected
modulo bias, and the reviewed 2,187 toy oracle tables with 61,236 zero-ancilla
basis checks and 393,660 full-workspace permutation checks. These finite cases
can detect mistakes; they are not the universal oracle reduction or concrete
cryptographic evidence. The original projected-XOF theorem checker owns its
explicit historical binary schedule separately, with unchanged theorem controls.

The existing 54-target accounting is retained as a conservative external
union here. This document does not identify that number with adaptive context
cardinality or assume a fresh 2^32 budget for each proof inside one attempt.
Concrete hash instantiation errors remain outside the displayed result.

Source geometry, digest width, canonical layout flags and exact SharedProof
field shapes are checked before arithmetic. Full hashes record the remaining
source context for review; recomputing those hashes is not a semantic proof of
Rust equivalence. The checker does not certify allocation charges, performance,
actual new proof bytes or production readiness.

The checker entry points share one explicit input inventory, including both
profile checkers, the historical theorem checker, the imported geometry helper,
and the adaptive-context specification. Each captures that inventory before and
after its calculation and rejects drift. Changed-file and omitted-input controls
run against disposable copies; hashes establish provenance, not semantic proof.
