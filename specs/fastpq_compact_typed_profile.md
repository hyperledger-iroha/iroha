# Conditional typed-compiler profile and resource arithmetic

2026-09-06. **Decision artifact, not a profile change or concrete security
qualification.** The independently reviewed ideal proof is
[fastpq_compact_typed_compiler.md](fastpq_compact_typed_compiler.md).
This calculation retains its canonical-binary-interface factor of two, all
shared verifier oracle work, the requested 54-target union and adversary
binary-oracle budget 2^32. It does not silently substitute the proof's initial
position count for adversarial queries.

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

Adding 21 chain calls gives **44,562 H calls**; adding 22 G calls gives
**44,584 verifier expansion calls**. Thus

```
T = 2*(2^32+44,584) = 8,590,023,760.
```

The factor two counts the exact group-oracle simulation of binary canonical
output access in the reviewed ideal-H proof. The expansion lists every raw
G output, chain edge and authenticated node/leaf used by verification. Its
weighted witness bound is

```
K_weight <= 44562/p^6 + sum_(j=1..22) 2^-R_j.
```

These counts are per segment. A bundle checking S segments must instead charge
all S expansions: T=2*(2^32+S*44,584), with K_weight at most S times the displayed
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

| Segments | H / G calls | Group-query budget T | Scaled acceptance interval / 1024 | Honest-abort bound over 54 bundles |
| --- | ---: | ---: | ---: | ---: |
| 1 | 44,562 / 22 | 8,590,023,760 | (743,744) | < 2^-130 |
| 2 | 89,124 / 44 | 8,590,112,928 | (743,744) | < 2^-129 |
| 128 | 5,703,936 / 2,816 | 8,601,348,096 | (745,746) | < 2^-123 |

The acceptance intervals divide by 1024 and then multiply by 2^-128. The fixed
400-candidate tape passes the separate chosen 2^-128 honest-abort certificate
for two segments but does not certify it for 128. A permanent honest abort is
not false acceptance. This arithmetic neither approves 128-segment resource
limits nor reinterprets the 54 external targets as segments. The checker rejects
assertion-disabled Python and out-of-range segment counts, and pins one-segment
parity with the original calculation.

## Fixed challenge tapes and honest abort

Every field-vector round takes the required number k of canonical Goldilocks
coordinates from k+6 independent u64 candidates in tape order. Reject each
candidate at least p. Abort the entire attempt if too few remain. Partition
accepted coordinates into extension elements in the fixed basis/order.
The uniform rejection probability is r=(2^32-1)/2^64. Abort implies at least
seven rejections, so

```
eta_field(k) <= binom(k+6,7)*r^7.
```

Successful vectors are exactly uniform; their values are independent of which
candidate positions were accepted. The whole tape is bound even after the
required coordinates have been obtained.

| Message | Count | Required base coordinates | u64 candidates | Tape bytes per message |
| --- | ---: | ---: | ---: | ---: |
| Ignored initial dummy | 1 | 0 | none | 48 |
| Column mixing | 1 | 1,368 | 1,374 | 10,992 |
| Constraint mixing | 1 | 3,692 | 3,698 | 29,584 |
| Joint pair | 1 | 8 | 14 | 112 |
| FRI beta | 17 | 4 | 10 | 80 |
| Complete query subset | 1 | not field elements | 400 labels of 19 bits | 950 |

The dummy decoder ignores its **384-bit random raw tape**. This preserves its
empty IOP message while avoiding a zero-width G collision denominator.

For q=375 and L=2^19, read 400 independent 19-bit labels directly from the
950-byte final tape. Take the first 375 distinct labels and sort; abort if
fewer exist. There is no modulo/rejection bias because the domain is exactly
2^19. Permuting domain labels preserves all tape probabilities and the
completion event, so successful subsets are uniform. Before completion the
conditional probability that a label is not new is at most 374/L.
After completion define non-new indicators as zero. Failure requires at
least 26 such indicators, and therefore

```
eta_query <= binom(400,26)*(374/524288)^26.
```

The exact certificates establish:

* sum of field-round abort bounds <2^-153;
* final subset abort bound <2^-136;
* entire attempt's honest abort bound <2^-136;
* union over 54 honest attempts <2^-130.

399 candidates fails the chosen 54-attempt <2^-128 abort certificate; 400 is
the first passing candidate count under this particular conservative formula.
This is not a claim about optimal tape sizing or a lower bound on true abort.

There are **43,046 G output bytes** altogether, of which **42,096** enter
subsequent chain hashes; the final 950-byte query tape has no later root.
Raw tapes are recomputed by the verifier and need not be transmitted in the
proof. Repeated attempts remain charged to the adversary query budget.

At q=375 the largest state error is the final query ratio, below 2^-202.
The commit-round maximum remains below 2^-214. The H collision/attachment
term is below 2^-349, and the comparison B is below 2^-367. These component
exponents are intermediate bounds, not production security-bit claims.

## Wire and internal work projection

The current canonical SharedProof layout uses a 40-byte Norito header,
compact field/element lengths, fixed u64 sequence counts, 48-byte digests,
37-byte encoded extension elements, 3,093-byte encoded complete-row elements,
81-byte mixed/quotient query elements, and 82-byte FRI group elements.
The arithmetic has been cross-checked against a retained public 136-query
proof's 1,608,631-byte size and the prior Rust sizing fixture's 2,534,462-byte
loose-shape size. The retained proof check is optional on a fresh checkout;
if present, its exact published SHA-256 and byte length are required.

Projecting the same field layout to q=375 with minimal frontiers gives an
upper bound of **4,326,227 bytes per segment**. The corresponding maximal-byte
shape has sibling counts
7,024 row, 3,887 mixed, 3,887 quotient and 18,245 FRI digests.
These are not independent upper bounds on each sibling frontier: opening all
leaves can reduce a frontier to zero. The combined framed values/frontier
size, however, increases with the opened count m. A group contributes 83 bytes
including its element-length prefix; each sibling contributes 49. As m grows
by one, P(d,m) never decreases, so the sibling count can fall by at most one.
That loses at most 49 payload bytes and one compact-length-prefix byte, while
the group sequence gains 83 bytes. Its own prefix cannot shrink, so the
combined round body grows by at least 33 bytes; framing that larger body is
also monotone. Complete rows have a still larger per-element cost. The
certificate checks the exact framed formulas for every m through 512 in all
FRI depths and through 1,024 row openings. Thus maximal-byte shapes occur at
the stated count caps even where their frontier alone is smaller.

This is a sum of per-tree byte maxima and an exact size formula at those
maximizers. Later measured candidate proofs are recorded separately in the
[framing evidence](fastpq_compact_shake_framing.md); the projection itself is not
a measurement or a production profile change.

Using the source's looser preflight ceiling of m*depth sibling entries gives
**6,759,875 bytes**. Such a largest shape can pass a structural sizing step
while failing the later exact frontier check; it is a relevant hostile decode
resource bound, not a valid proof maximum.

Both estimates exclude outer artifact/bundle framing, public statements and
any future fields required by the finalized schema. They do not supply a
Norito allocation ceiling: the later same-profile measurements in the framing
evidence separately record actual and loose-shape cumulative codec charges. A 4 MiB segment ceiling is already too small for
the valid upper projection. Neither default admission limits nor existing
bundle limits were raised.

SHAKE256 has 136 output bytes per rate block. The G tape table requires 325
output blocks in total, including each call's first block, or 303 additional
squeezing permutations after those first blocks. If call j has a finalized
byte input length a_j, the ordinary padded sponge permutation count is

```
sum_j [floor(a_j/136) + ceil(output_bytes_j/136)].
```

The later prefix/body framing specification fixes the input encoding and
records cached-absorption equivalence and measured local resource evidence.
H's internal permutation cost also remains separate from its 44,562 logical
calls. No end-to-end runtime is extrapolated from these counts.

## Reproduce and review

Run
`python3 scripts/fastpq/check_compact_typed_profile.py`.
Use `--output PATH` to choose the certificate destination and
`--retained-proof PATH` for an optional retained 136-query cross-check.
The default output is under `target/fastpq-production-validation`; a missing
retained proof is reported as unavailable and does not skip mathematical or
structural source checks. No network or retained PDF is required.
The JSON certificate has full source hashes and the exact parameter table.
It checks every nonempty leaf subset through 16 leaves (65,808 cases) against
the parent maximum/frontier identity, optionally validates the wire formula against the
retained public artifact, and certifies the rational security/abort decisions.

The existing 54-target accounting is retained as a conservative external
union here. This document does not identify that number with adaptive context
cardinality or assume a fresh 2^32 budget for each proof inside one attempt.
Concrete hash instantiation errors remain outside the displayed result.

Source geometry, digest width, canonical layout flags and exact SharedProof
field shapes are checked before arithmetic. Full hashes record the remaining
source context for review; recomputing those hashes is not a semantic proof of
Rust equivalence. The checker does not certify allocation charges, performance,
actual new proof bytes or production readiness.
