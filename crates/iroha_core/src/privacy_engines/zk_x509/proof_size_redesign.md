# zk-X509 proof-size redesign

The implemented proof still exceeds the unchanged 9,437,184-byte cap. Activation
remains unavailable. No query constraint, query, relation column, blinding
coefficient, FRI fold, degree bound, or hash bit has been removed.

The first implementation step authenticates each binary FRI `(low, high)` pair
as one ordered 64-byte leaf. The relation's immutable shared-core descriptor
selects this layout for both X509 subproofs, including their terminal roots.
The transcript binds the layout. The prover, streaming replay, verifier,
frontier decoder, and exact size calculation use the same descriptor; proof
bytes cannot select a different layout. Other aggregate protocol families
retain their scalar-leaf commitments.

For `N` evaluations, a pair tree has `N/2` leaves. Every query already discloses
both values, so neither privacy exposure nor the binary FRI recurrence changes.
The leaf framing binds the pair's order, index, round, lane, and protocol context.
The maximum frontier now opens at most 136 pair leaves, instead of 272 scalar
leaves. This saves 656,640 MAIN bytes and 210,816 CA bytes.

| Exact current bound | Bytes |
| --- | ---: |
| MAIN aggregate including DEEP | 15,791,168 |
| CA aggregate including DEEP | 2,484,096 |
| Claim and outer framing | 13,354 |
| Complete X5S1 | 18,288,618 |

`stark/proof_size_redesign_tests.rs` derives these bounds from the actual
registration and shared codec accounting. The raw MAIN trace rows alone still
occupy 12,235,648 bytes. More hash deduplication cannot fix that obstruction.

The compact-CA verifier now additionally checks its complete 1,379-residue
constraint quotient at the Fp4 DEEP point. Its base-field and extension-field
evaluators instantiate the same polynomial AIR implementation. Public
challenges and terminal claims embed explicitly; the generic arithmetic
interface offers no field-value ordering or integer extraction. The verifier
derives each of its 80 fixed polynomials from the native schedule and evaluates
it at the same point, retaining one coefficient column at a time. All existing
current/next query checks remain. The stronger relation is profile-bound and
does not change the byte counts above.

The fixed-schedule owner also implements genuine Fp4 evaluation as a prerequisite
for MAIN. It uses `L_j(z) = (z^N - 1) * g^j / (N * (z - g^j))`, with one quartic
batch inversion, and shares the existing affine, sparse and repeated-row sum
kernels with base-field query evaluation. The schedule and its digest stay
unchanged. Native subgroup points and excessive public schedule work reject
before interpolation; all table allocations are bounded by native size, not
native size times column width. MAIN now uses this evaluator for the algebraic SHA/P-256 fixed schedules at
its DEEP point. This does not authorize dropping any relation query.

The MAIN arithmetic port covers all 49 registrations: 41 P-256 registrations,
projection, byte memory, RFC 5280, strict DER, and four complete SHA call-bus
registrations. Every scalar and Fp4 path shares the same polynomial constraints
and public terminal bindings. The typed registry uses family-specific contexts
and checks exact canonical registration metadata, including both fixed rows for
DER. The full MAIN verifier additionally sums every registered constraint
quotient at the transcript-derived extension point, using that registration's
native vanishing polynomial, and compares the result against all six opened
composition chunks. This check is tied to the existing DEEP low-degree relation;
all authenticated current/next query constraints remain. Public fixed schedules
are evaluated from verifier-owned data with bounded native-column scratch.
This stronger check changes the descriptor but does not change the byte bound
or select a reduced codec. Native execution and independent full-construction
qualification remain required.

The SHA capacity evaluator uses polynomial selection
`digest * dynamic_address + (1 - digest) * fixed_address`. The previous host
branch tested whether the opened digest selector equalled one; that matched
native rows but changed the relation at other LDE points. The raw word AIR,
all 335 capacity residues and all 796 complete SHA call-bus residues now share
generic base/Fp4 arithmetic, including RFC byte/length tuple compression and
compact-CA call start/terminal bindings.

The corrected SHA degree ledger is six including fixed selectors. The ordinary
padding equation already reaches six, as does the Boolean first-event pair
after polynomial address selection; the call-bus terminal gates also fit six.
At native size 524,288 and mask degree 1,815, the conservative quotient bound is
`6 * (524288 + 1815) - 524288 = 2,632,330`. It fits the existing six composition
chunks of 589,824 coefficients each (maximum combined degree 3,538,943).
The global degree-seven limit, hiding masks, FRI queries, byte bounds and cap
remain unchanged. The corrected per-registration degree and selection rule
change the profile descriptors and digests without a compatibility alias.

The shared joined-row commitment prerequisite derives immutable logical slices
from the existing layout and streams retained polynomials with mixed native
degrees into one common-domain base or auxiliary root. It keeps at most eight
zeroizing LDE columns plus one wiping digest state per common-domain row;
retained coefficients and requested opening rows remain separately accounted.
Its marker `u16::MAX` cannot alias an individual group index. No caller currently
uses that root in the MAIN transcript or wire. X5B1's six-root binding, proof
frontiers, and codec must change together before the root-count saving is real.
Independent Horner/materialized-tree tests cover joined rows, minimal frontiers,
worker-count parity and malformed inputs. Partial streamed opening scratch and
packing buffers clear on failure/drop; this is not a measured peak-RSS claim.

## Complete-relation candidate, not an implemented proof bound

The candidate keeps all 49 registrations, all trace columns, both DEEP openings
of every column, and all current security parameters. It requires two further
changes together:

1. Qualify the implemented complete MAIN Fp4 quotient check and its binding to
   the committed low-degree polynomials through the existing DEEP quotient.
   Only then can on-domain queries omit next rows:
   both `(T(x)-T(z))/(x-z)` and `(T(x)-T(z*g))/(x-z*g)` need the same `T(x)`.
2. Concatenate the six MAIN base rows into one common-domain base commitment,
   and likewise the six auxiliary rows. Keep their native strides, registration
   slices, challenge order, and separate base/aux transcript phases. This reduces
   roots/frontiers without deleting any relation columns. CA remains on its
   existing local domain.

[DEEP-ALI Protocol 17 and §5.3](https://drops.dagstuhl.de/storage/00lipics/lipics-vol151-itcs2020/LIPIcs.ITCS.2020.5/LIPIcs.ITCS.2020.5.pdf)
provide the primary-source construction behind the first step: the verifier
checks the constraint combination at an out-of-domain point and authenticates
the resulting quotient oracles through low-degree testing. Applying it to this
particular multi-trace, masked, split-composition implementation still requires
independent soundness analysis; the paper is not a certificate for this code.

Let `F(N,m)` be the shared codec's exact worst-case minimal Merkle frontier for
at most `m` opened leaves. For a proof with `g` trace groups, total trace width
`w`, common domain `N`, and `q=136`, the hypothetical further reduction is:

```
q*w*8 + 2*g*(F(N,2*q)-F(N,q))*48 + 2*(g-1)*(F(N,q)+1)*48
```

The terms remove next-row field bytes, their frontier cost, then the redundant
same-domain roots/frontiers. DEEP fields, composition/FRI values, masks,
terminal fields, and all claim framing remain in the count. The source-derived
candidate is MAIN 7,692,192 + CA 1,498,816 + framing 13,354 = **9,204,362 bytes**,
leaving 232,822 bytes below the existing cap.

TODO: validate the complete MAIN Fp4 quotient integration adversarially and
finish the common-domain trace commitment/transcript/codec protocol. Do not remove current
query constraints, reduce the published bound to the candidate, or open
activation before both are enforced. Independently qualify the resulting
soundness, privacy, deterministic KAT, complete relation, and resource bounds.

Focused checks cover independent pair framing, materialized/streaming parity,
mutation of either pair member and its context, exact codec round trips,
rejection of the other commitment layout, unchanged scalar arithmetic, and
source-derived size bounds. These do not establish full-profile readiness.
CA tests also compare every extension-field residue against independent
base-field polynomial interpolation, verify fixed-column evaluation against
known polynomials, and reject changed DEEP rows or recomposed quotient claims.

Fixed-schedule tests compare Fp4 results to independently interpolated native
columns and Horner evaluation, check embedded base-field query parity and the
highest-degree monomial, exercise every cyclic shift across several gcd cycles,
and reject native-subgroup points and an over-budget schedule before evaluation.
The MAIN comparison tests reconstruct every residue independently from thirteen
base-field samples, compare base embeddings, exercise all eleven registrations,
and reject malformed dimensions/noncanonical claims. A terminal-claim mutation
changes the final binding residues. The native scalar regression suite remains
the independent valid-witness reference for this port.

SHA tests independently interpolate a native digest selector and multiply its
polynomial coefficients before evaluating at LDE/Fp4 points. Nineteen scalar
samples reconstruct every Fp4 capacity residue; affine finite differences check
the six-degree ceiling and an explicit sixth-degree term attains it. A malformed
digest address remains rejected even when its local memory pair is rewritten to
match. The existing valid native SHA traces remain the schedule reference.

The complete SHA registration tests independently lift all 796 residues for
each of its four instances, check the total-degree ceiling, and reject wrong
segment identities, malformed field encodings and changed source/CA claims.
The typed availability test inventories all 49 canonical registrations. Window,
value execution/sorted, scalar-bus, arithmetic, sink, DER, RFC, projection and
byte-memory tests reconstruct extension residues from independent base-field
samples, check declared polynomial degree, reject malformed shapes/encodings,
and mutate each public terminal family. The registered sink's optional-selection
polynomial has total degree three, correcting its former degree-two declaration;
this remains below the unchanged global degree cap. Native compiled validation
is required in addition to these source-coupled checks.
