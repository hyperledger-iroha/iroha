# zk-X509 complete-relation proof geometry

The selected first-release codec totals **9,420,938 bytes**, within the unchanged
9,437,184-byte cap. Activation remains unavailable pending complete native KAT,
adversarial, independent soundness/privacy, and resource qualification. Geometry
is not release evidence. All 49 registrations, 5,811 MAIN trace columns, 136
queries, hiding coefficients, FRI folds, degree bounds and hash widths remain.

| Exact codec bound | Bytes |
| --- | ---: |
| MAIN aggregate including DEEP | 7,908,768 |
| CA aggregate including DEEP | 1,498,816 |
| Claim and outer framing | 13,354 |
| Complete X5S1 | 9,420,938 |
| Remaining cap headroom | 16,246 |

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
reuses the original explicit masks and immutable native sources, and checks both
reconstructed roots. Every coefficient replay reconstructs the same polynomial;
no mask is resampled and no seed-based random generator is introduced.
The wire, transcript, leaf hashing and frontier accounting share one immutable
layout; proof bytes cannot select a fallback. The joined leaf marker `u16::MAX`
cannot alias an individual group index. Base and auxiliary roles remain distinct.

Retaining every native coefficient would require 17,018,207,808 payload bytes,
already above the unchanged 12 GiB ceiling. The MAIN owner instead retains
84,422,208 bytes of original mask coefficients and reconstructs columns from its
closed phase owners. Commitment passes retain at most eight clearing coefficient
and LDE columns and one wiping digest state per common-domain row. The quotient
stage evaluates one registration in interleaved stripes of at most 524,288 rows;
it folds coefficients modulo the stripe polynomial before FFT, preserving the
original full-domain values, native translations and final quotient IFFT.

`stark/main_resources.rs` counts these transform and commitment buffers from the
actual layout, including composition, FRI masks/trees, openings and wire scratch.
Before source construction or mask sampling, the prover charges the borrowed
assembly's actual vector capacities and reserves a 6 GiB native-source allocation
allowance, a separate 1 GiB construction/replay allowance and 1 GiB of process
headroom against the same 12 GiB ceiling. A second preallocation check validates
DER/I/O/projection extents and combines their full-width forecasts with the
canonical P-256 source layout and all 29 SHA fixed schedules. Serial SHA scratch
includes raw circuits, nested event vectors and complete base/fixed/aux matrices.
Every phase rechecks actual retained
source capacities, including the assembly, bound DER copy, P-256 sources, SHA
fixed schedules, I/O columns and projection auxiliary rows. Unused vector capacity
is charged. These allowances are admission policy, not measured allocator or RSS
bounds. Source construction/replay peaks and maximum-profile time/memory still
require native qualification. Packing buffers, replay coefficients, incomplete
private opening scratch and retained DER/SHA source cells use guaranteed clearing.
Fallible DER/SHA builders guard populated field, word-event and call-product tables
before transferring them into retained owners, including late binding failures.
Private word-event allocations move between clearing owners; public fixed schedules
retain their original values. Joined commitment hashing absorbs each resident batch
of at most eight columns in row order, parallelizing independent 1,024-row hash-state
chunks with one clearing 64-byte buffer per active chunk. It preserves the exact
framed bytes, and failures poison the builder before it can publish a root. This does not claim erasure of every transient
compiler-generated scalar or stack copy.
The conservative early reservation leaves 596,974,144 bytes for the borrowed
assembly. The corrected RFC source alone retains 421,806,624 bytes for the maximum
structural fixture; a fresh complete assembly measurement is still required.
Earlier measurements of the smaller RFC layout do not qualify this layout.

Public fixed polynomials use one owned matrix across quotient stripes. A later
stripe performs an inverse FFT, scales coefficient `k` by the ratio of the new
and old shifts to power `k`, and applies the forward FFT in place. Fixed
polynomials have degree below their native domain, which fits every stripe;
masked witness polynomials still use the original folded evaluation. This saves
427,819,008 fixed-matrix bytes at the RFC registration. The ledger charges one
native-column allocation overlap when padding a smaller fixed domain and counts
6,535 additional public recovery IFFTs (32,549,109,760 butterflies). These real
operations are included in diagnostic receipts; memory reuse is not a latency
qualification.

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

The RFC temporal relation now has 285 base, 280 auxiliary and 102 fixed columns,
with 1,681 residues of degree at most four. Its 72 fixed time slots form a complete
census of authenticated DER time-node identities, including optional certificate
and revoked-entry slots. Fifteen byte positions bind each decimal date, its actual
DER time tag, terminal `Z` and UTC padding. Seven calendar phases enforce bounded
Gregorian quotients, exact leap-year arithmetic and timestamp conversion. The
73 two-phase comparisons bind private calendar operands and verifier-owned public
window values; 38-bit slack and byte carry constraints prevent field-wrap
inequalities. Four affine logarithmic-derivative lanes use `1 + dot(challenge,
tuple)` and constrain singular inverses and singular counts explicitly. These
changes rotate the profile and require fresh complete proof/KAT and independent
soundness qualification; isolated native-column and degree tests do not replace it.


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

The ignored release-only `maximum_profile_assembly_payload_and_source_admission_diagnostic`
constructs the complete deterministic maximum structural fixture, records actual
assembly capacities and construction time, and checks unchanged source admission.
`maximum_profile_streaming_hash_cost_diagnostic` measures the actual joined row
hashing with all 5,811 columns and 128 public rows, comparing one-column and
eight-column batches with one worker. It does not measure parallel throughput. Its linear timing
estimate excludes source replay, FFTs, quotient/DEEP/FRI and CA work, and differs in
cache residency from a full proof. `maximum_profile_replay_fft_cost_diagnostic`
measures eight actual masked log19-to-log22 transforms, validates sampled outputs
by Horner evaluation, and reports the commitment-only transform scaling. These
diagnostics do not establish release readiness; run the assembly test in its own
process with `/usr/bin/time -l` for measured RSS.
