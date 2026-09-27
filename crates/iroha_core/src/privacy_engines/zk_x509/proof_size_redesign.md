# zk-X509 complete-relation proof geometry

The selected first-release codec totals **9,204,362 bytes**, within the unchanged
9,437,184-byte cap. Activation remains unavailable pending complete native KAT,
adversarial, independent soundness/privacy, and resource qualification. Geometry
is not release evidence. All 49 registrations, 5,623 MAIN trace columns, 136
queries, hiding coefficients, FRI folds, degree bounds and hash widths remain.

| Exact codec bound | Bytes |
| --- | ---: |
| MAIN aggregate including DEEP | 7,692,192 |
| CA aggregate including DEEP | 1,498,816 |
| Claim and outer framing | 13,354 |
| Complete X5S1 | 9,204,362 |
| Remaining cap headroom | 232,822 |

`stark/proof_size_redesign_tests.rs` derives these sizes from the actual layouts
and codec. `scripts/check_zk_x509_proof_geometry.py` independently counts roots,
query fields, full DEEP fields, terminal fields and maximal canonical frontiers.
The MAIN section allowance is derived from the same outer cap after reserving
the complete CA envelope. No cap was increased.

The verifier enforces complete Fp4 constraint quotients before the reduced-row
DEEP/FRI check. MAIN includes all 49 registrations with their native vanishing
polynomials and all six composition chunks. CA includes every one of its 1,379
residues and all four chunks. Scalar and extension evaluators instantiate the
same polynomial kernels, including public terminal bindings. The generic field
interface offers no ordering or integer extraction from an extension element.
Every fixed polynomial is evaluated from verifier-owned native schedule data.

Only current trace rows are disclosed at each on-domain query. Both DEEP values
of every column remain: the verifier binds `T(z)` and `T(z*g)` using
`(T(x)-T(z))/(x-z)` and `(T(x)-T(z*g))/(x-z*g)`. Both quotients need the same
committed `T(x)`. The complete AIR is checked at `z`; removing the redundant
on-domain next row does not remove a relation constraint. The shared core uses
one implementation of these quotients and the FRI recurrence for full and
reduced layouts. The scalar callback API rejects reduced layouts; the separate
complete-OODS entry is invoked only after the full relation check.

[DEEP-ALI Protocol 17 and §5.3](https://drops.dagstuhl.de/storage/00lipics/lipics-vol151-itcs2020/LIPIcs.ITCS.2020.5/LIPIcs.ITCS.2020.5.pdf)
provide the primary construction. Applying it to this masked, multi-trace,
split-composition implementation requires an independent analysis; the paper
is not a certificate for this code.

MAIN's six logical groups retain their exact native domains and registration
slices. Their polynomials are sampled in canonical provider/column order, then
streamed into one common-domain base root. X5B1 binds that root and the CA base
root before deriving its existing 272 ordered challenges. Auxiliary polynomials
are then sampled and committed into one separate joined root. Query replay
reuses the retained masked coefficients and checks both reconstructed roots.
The wire, transcript, leaf hashing and frontier accounting share one immutable
layout; proof bytes cannot select a fallback. The joined leaf marker `u16::MAX`
cannot alias an individual group index. Base and auxiliary roles remain distinct.

The joined owner retains native coefficient vectors, at most eight zeroizing
LDE columns, and one wiping digest state per common-domain row. Requested row
openings are counted separately. Packing buffers and incomplete private opening
scratch clear on failure/drop. This is an allocation design, not measured RSS.
The complete prover still needs maximum-profile time/memory qualification.

Binary FRI authenticates each ordered `(low, high)` pair in one 64-byte leaf,
including the terminal tree. Every query still discloses both values and uses
the same fold equation. Pair framing binds order, index, round, lane and protocol.
This saves 656,640 MAIN bytes and 210,816 CA bytes. Other aggregate families
retain their scalar-leaf and current/next trace layouts without changed bytes.

The SHA selector now uses the polynomial
`digest * dynamic_address + (1 - digest) * fixed_address`; the former host branch
was wrong away from native Boolean rows. Its actual degree is six including
fixed selectors, within the unchanged global cap seven. The conservative SHA
quotient degree 2,632,330 fits six chunks of 589,824 coefficients. Binding-sink
optional selection has degree three, correcting its previous degree-two ledger.
Both corrections are reflected in descriptors and profile digests.

Tests cover full base/Fp4 polynomial lifting, all 49 typed registrations, each
terminal family, fixed schedules against independent IFFT/Horner evaluation,
complete MAIN quotient dispatch, paired and joined Merkle commitments,
canonical reduced codecs, malformed shapes/fields/frontiers, root/column order,
worker-count parity, erasure paths, and full current-only DEEP/FRI binding with
mutations of either DEEP point. Native execution is reported separately from
source checks. No partial relation or conditional recursive-replacement budget
is an accepted proof path.

TODO: complete fresh native validation and independent whole-construction
soundness/privacy review, produce the deterministic full-profile X5S1 KAT and
adversarial corpus, and measure the maximum prover on supported hardware before
populating qualification pins or enabling the profile.
