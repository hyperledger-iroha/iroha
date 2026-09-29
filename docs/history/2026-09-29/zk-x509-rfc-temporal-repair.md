# X509 temporal relation and bounded quotient repair

This record covers the source following the failed September 28 maximum
credential attempt. That earlier immutable producer returned `DerWitness`
without a proof after 1,159.504 seconds. Its original receipt and measurements
remain in the [September 28 record](../2026-09-28/zk-x509-exact-root-and-deep-work.md).
Activation remains unavailable; none of the checks below is a release certificate.

## Relation and exact geometry

The DER producer now emits long-length and primitive-boundary state in the order
required by its unchanged AIR. MAIN supplies the embedded DER documents in the
same order as RFC provenance. Complete source preflights then exposed calendar
column collisions, incorrect Gregorian arithmetic and missing temporal operand,
census and integer-range bindings. These required an RFC AIR/layout correction.

The RFC registration now has 285 base, 280 auxiliary and 102 fixed columns,
1,681 constraints and maximum degree four. Its fixed schedule has 72 time slots,
each with 15 byte rows and seven calendar phases, plus 73 two-phase comparison
slots and 73 eight-row range decompositions. The authenticated DER time census
binds document, role and occurrence identity. Public comparison operands and
strictness come from the verifier; differences are limited to 38 bits to prevent
field-wrap witnesses. Four affine logarithmic-lookup lanes use `1 + dot` factors,
constrain singular inverses to zero and retain both ordinary and singular counts.
The compressed-relation inventory is 30; the conservative 171-bit bound remains.

All 49 MAIN registrations, native domains, mask lengths, query counts and global
degree/chunk bounds remain. MAIN has 5,811 trace columns, including 3,900 at log19.
The exact codec bound is 9,420,938 bytes: 16,246 below the unchanged 9,437,184-byte
ceiling. Nine source-bound geometry tests pass. Ordinary and maximum actual-source
RFC preflights pass all 285 base and 280 auxiliary columns, populated AIR prefixes,
boundary constraints and DER handoff terminals in 180.76 and 156.68 seconds.
Full generic Fp4/degree-four and omission, duplication, identity, time, strictness,
public-operand and range adversaries pass in the isolated source harness.
The retained receipt is
`dist/zk-remediation/2026-09-28/rfc-release-preflight-isolation/affine-final-result.json`.
This harness does not construct the whole MAIN assembly or a credential proof.

## Bounded fixed-polynomial storage and cleanup

One verifier-public fixed matrix moves between quotient stripes in place:
inverse FFT recovers the previous shifted coefficients, a public shift ratio
rescales them, and forward FFT produces the next stripe. Native polynomial
degree is below every stripe extent. The allocation ledger includes one native
column during padding growth. Original private masks and trace polynomials,
transcript order and proof bytes are unchanged by this storage transformation.

Twenty optimized actual-source matrix, stripe, cache, observer and clearing-owner
controls pass, including independent Horner/coefficient recovery across shifts,
padding, exact quotient/chunk parity, poisoning and unwind. The separate
composition precursor selection passes eight controls: guarded partial results,
displaced allocation cleanup, independent arithmetic, late evaluation failure
and explicit clearing before cancellation truncates initialized cells. Receipts:

- `dist/zk-remediation/2026-09-29/main-fixed-coset-kernel/result.json`
- `dist/zk-remediation/2026-09-29/main-composition-ownership-tail/result.json`

The current source ledger charges 3,697,993,152 bytes of peak transformed buffers
and leaves 596,974,144 bytes for the borrowed whole assembly after source,
scratch and runtime reservations. The isolated maximum RFC owner alone occupies
421,806,624 bytes. These capacity calculations alone do not establish admission;
the integrated measurement below supplies whole-assembly evidence. Process RSS
and address-space containment remain unverified. Additional public fixed
recovery work is counted explicitly: 6,535 inverse transforms and
32,549,109,760 butterflies. No latency improvement is inferred from the design.

## Native profile reconstruction and remaining integrated evidence

The first debug attempt retained a stale public fixed-width assertion (189 versus
the derived 210). After that assertion was corrected, the next build completed
in 11 minutes 43 seconds. Its immutable test binary and receipts are under
`dist/zk-x509-prover-evidence/rfc-calendar-debug-20260928T165822Z`;
the binary SHA-256 is
`7244cce85ae2952dddd3a3acb1d1344d6e802f27fb90108957342af83ed40a19`.
Unrelated SCCP/configuration source drift prevents treating this as a coherent
whole-workspace candidate. X509 and its captured cryptographic source scope did
not drift during the build.

Native output supplies all 29 profile fields. Two independent reconstructions of
the exact 16,935-byte frame produce
`7cf3286b4560be90d2305b33c9aaea30a39e6841f4045cf68bca68895d1b6063`.
The profile pin and two deterministic proof known answers have been updated from
that output; actual proof verification and canonical re-encoding succeeded before
the old known-answer assertions failed. Literal deltas are retained separately at
`dist/zk-remediation/2026-09-29/x509-final-profile-literals`.

The kernel/resource/derivation selection executed 66 passing controls, two stale
resource/work-count assertions and one explicitly ignored timing diagnostic.
Both expectations are corrected from the current geometry, without changing
limits. The six unsigned derivation, policy, gas and binding controls passed.
The maximum assembly attempt stopped at the deliberately stale profile pin
before witness construction; it provides no whole-assembly payload measurement.
Assembly errors now preserve that profile cause, and the recursive-erasure test
must construct the canonical profile instead of returning success when it fails.

## Pinned frozen-candidate results and test-fixture correction

The subsequent ordinary locked/offline debug build completed in 1,208.82 seconds
with no drift in its 20,877-entry source census. Its immutable binary is retained
under `dist/zk-x509-prover-evidence/frozen-calendar-pinned-debug-20260928T172601Z`,
with SHA-256
`0e3dc07e3b6c74d89d4da0024babe03a7451cc376866f4909437b1b10a63b80e`.
The source manifest is
`76638ce92cc40a57e290e20027a1f10ea0e4672f683dffe0107ce834701c332c`.
This is scoped algorithm evidence from a frozen historical Core candidate. The
current main checkout has since retired the replay-binding IVM circuit and API;
the older candidate's policy/API controls do not qualify those current surfaces.

The profile constructor and actual maximum assembly controls both pass. The
whole MAIN assembly owns 542,564,850 bytes, below its 596,974,144-byte admission
allowance by 54,409,294 bytes. The previously failing profile pin now matches,
and the assembly error-cause and mandatory recursive-scrub controls pass.
The exact retained selections are:

| Selection | Passed | Failed | Ignored | Seconds |
| --- | ---: | ---: | ---: | ---: |
| Profile constructor and maximum assembly | 2 | 0 | 0 | 59.43 |
| MAIN resources, replay and ownership | 85 | 0 | 0 | 184.46 |
| Profile and native proof known answers | 26 | 0 | 0 | 284.39 |
| Historical typed policy, wallet and unsigned derivation | 36 | 0 | 0 | 164.03 |
| RFC native constraints, excluding two full-column preflights | 53 | 4 | 0 | 213.15 |
| Source geometry | 9 | 0 | 0 | 0.39 |

The four RFC failures remain recorded. They exposed stale test assumptions:
the ordinary fixture has no revoked entries, the fixed non-padding schedule is
284,014 rows, the independent affine degree inventory is `[0, 1, 847, 270, 563]`,
and the new DER-authenticated calendar identity phase cannot use an all-zero
synthetic positive row. The two-file correction uses the real 64-entry maximum
fixture for the omission adversary and actual calendar component phases for the
copy adversary. It preserves the rejection assertions, pins the observed exact
degree inventory and independently recomputed family sum, and changes no
production relation or limits. The test-only RFC descriptor digest becomes
`e1e688eabe67ed71f7da49e07c0bccf25f92a047055367c1d68c741feb40c6fa`;
the compiled 29-field engine profile remains unchanged.

The exact preimages, postimages, patches and review are retained at
`dist/zk-remediation/2026-09-29/x509-rfc-reviewed-amendment`.
Only those two files were applied to the frozen candidate. Its successor census
is `b66c0bb30a4f229a7f746fc963ae89b9ac367aa9a9137a726d0a4d6a06846c4b`.
The ordinary locked/offline opt-level-three baseline build passes in 7,574.44
seconds with no source drift. Its 528,310,336-byte immutable executable has
SHA-256 `de561a88ff13314c1cd52c77ac80c1fddb544a6e0949165e0dd3367fa677a4dc`.
All eight repaired RFC/profile/assembly controls pass with no failures or ignored
tests in 4.949 seconds; external process RSS is 1,426,472,960 bytes. This clears
all four prior RFC test failures natively, preserves the engine pin and again
measures 542,564,850 bytes for maximum whole-assembly payload. These controls do
not construct a complete proof. The baseline complete-proof plan was superseded
before launch by the storage change below; that unstarted proof is neither a
pass nor a failure. The original plan, redirect, binary and control receipts
remain under `dist/zk-x509-prover-evidence/frozen-calendar-opt3-20260929T035530Z`.

## Fixed-family source storage

The RFC producer now stores row prefixes selected solely by the verifier-fixed
family: 66 cells normally, 123 for SourceNode, 102 for Grammar, and all 285 for
Calendar and Decimal. Construction rejects every nonzero omitted field. Replay
reconstructs the exact full row before restoring carried selectors and applying
the existing normalization. No AIR, transcript, profile digest or proof limit
changes. Old and replacement capacities, including unused capacity and the
surviving public schedule, are charged during their overlap against the existing
1-GiB source scratch allowance. Both sides retain clearing owners on success,
errors and unwinds; construction-only private owners are released first.

Eight isolated owner/eraser controls pass. A separate optimized actual-source
RFC harness, with debug assertions enabled, passes two maximum-fixture
adversarial controls, four Fp4 controls, and both complete column preflights.
The ordinary and maximum cases each replay all 285 base and 280 auxiliary
columns, populated base AIR rows, boundary constraints and DER/RFC terminals.
Their times are 111.040 and 113.047 seconds; the combined phase takes 224.097
seconds and records 482,508,800 bytes of process RSS. No captured source drift
occurred. The receipt is
`dist/zk-remediation/2026-09-29/x509-family-storage/release-preflight/complete-receipt.json`.

A separate instrumented run of the unchanged maximum-fixture source preserves
all assertions and records 51.819 seconds for base columns and 58.037 seconds
for auxiliary columns, with 111.681 seconds for the fixture. Its instrumentation
and receipt are retained under `x509-family-storage/column-cost`. These timings
identify repeated row reconstruction as material work before proof transforms;
they are not a complete-prover latency measurement.

The actual maximum RFC heap retains 80,058,504 bytes, down from 421,806,624;
the ordinary heap retains 63,097,464 bytes. Subtracting that reduction from the
earlier whole assembly gives a heap-only projection of 200,816,730 bytes. The
new owner also adds a native `usize` to each of 18 inline family headers: 144
bytes on this host. Including those headers predicts 200,816,874 bytes, leaving
396,157,270 bytes inside the existing allowance. This would admit the existing
four-column Metal staging charge of 336,675,324 bytes if the integrated
measurement and backend selection agree. These were arithmetic predictions
before the integrated control below measured the whole assembly; actual
complete-prover backend selection remains a separate measurement.

The exact three-file amendment, preimages, postimages and independent semantic
review are retained under `dist/zk-remediation/2026-09-29/x509-family-storage`.
Following the eight native baseline controls, exactly those three files were
applied to the frozen candidate. Its 20,878-entry source census is
`2a2da48ecdce3f0ec0e7009a63e577f347104bfd28d83faaabbe7ccaa5f4cf11`.
Scoped formatting and all nine source-geometry controls pass after capture.
The ordinary opt-level-three build started at 06:27:06 UTC on September 29,
after FASTPQ released the shared target with its immutable executable captured,
and passed in 4,737.127 seconds with no source drift. Its 529,617,600-byte retained
executable has SHA-256
`cc3f185cbd74e22ef57e75646308052205494c9509d55c57313ea44ee290d829`;
the compiler artifact confirms optimization level three, disabled debug
assertions and disabled overflow checks. The build and 72 native controls plus
complete-proof attempt are retained under
`dist/zk-x509-prover-evidence/frozen-family-storage-opt3-20260929`.
The build uses two Cargo jobs for compilation throughput, with unchanged
optimization and proof limits; this is not proof resource evidence.
The later dependency audit found 40 of its 63 local artifacts marked fresh by
the shared Cargo target. Their exact historical source provenance is not fully
established, so these results retain that dependency-provenance limitation;
the clean candidate-specific rerun below is required for coherent evidence.
All 72 controls pass: four clearing-owner controls in 0.102 seconds,
all 57 RFC native constraints in 18.477 seconds, and nine profile/assembly
controls in 5.354 seconds. The actual maximum whole assembly owns 200,816,874
bytes, exactly matching the header-corrected projection above and leaving
396,157,270 bytes under the unchanged allowance. The profile/assembly phase
records 1,020,510,208 bytes of process RSS; this is not complete-prover RSS.
The ordinary and maximum complete column preflights also pass in 107.672 and
105.582 seconds, recording 689,471,488 and 778,027,008 bytes of process RSS.
All five phases have zero failures and ignored tests and total 237.187 seconds;
their exact receipts are collected in `controls-summary.json` in that evidence
directory. The actual maximum-credential proof started at 07:50:42 UTC from the
retained executable after releasing the shared build target. It failed without
a proof at `MainProofConstruction(TranscriptMismatch)`. Producer time is
1,062.696011 seconds; external process time is 1,062.81 seconds (1,284.41 user,
17.84 system), and runner wall time is 1,062.815966 seconds. External peak RSS
is 7,993,442,304 bytes, below the unchanged 12-GiB ceiling for this failed attempt.
The 300-second target is missed; neither proof size nor successful complete
verification is established.

The runtime selected the admitted four-column Metal path and completed 805
dispatches over all 3,213 initial base columns, with no CPU or failed common-domain
transforms. Base mask sampling takes 444.993319 seconds, including 442.962667
seconds of source construction and 1.248330 seconds of mask draws. Base
commitment takes 567.292353 seconds and compact CA proving 29.124959 seconds.
The bound-source phase fails after 7.259022 seconds; DER binding (0.039592) and
RFC binding (0.178030) both complete. Auxiliary masks, auxiliary commitment,
composition, DEEP/FRI, query openings and envelope/self-check are not reached.
These phase durations are nested and must not be summed indiscriminately.

The full log, assessment, parsed phase receipt and copied public failure receipt
remain in the same evidence directory as `maximum-proof.log`,
`maximum-proof-assessment.json`, `failure-phase-analysis.json` and
`failed-public-receipt/receipt.txt`. The latter has SHA-256
`6d0c1c2c195815c290830c163874d38c5fc3caa35eb824f63a078a4e7e493ae3`.
This is scoped historical-candidate evidence. Current-Core integration and
activation remain unavailable. The next work must repair the bound-source
terminal mismatch and reduce the complete prover's source/commitment work under
the same limits; the assembly reduction alone is insufficient.

## RFC-to-SHA issuer-channel correction

The late bound-source failure exposed a stale issuer-SPKI channel address in
both the native SHA adapter and its algebraic fixed-schedule compiler. Each
used projection-prefix plus 22, which is the CRL signer's P-256 public-key
channel. The canonical strict-DER producer and RFC role schedule place issuer
SPKI at prefix plus 24, after the CRL and wallet keys. The existing isolated
SHA tests repeated the stale address instead of checking the producer.

The repaired candidate shares one public-shape address derivation between
native and algebraic SHA schedules. New controls derive all SHA message and
length addresses from actual DER producer declarations for every disclosure
count, compare the maximum credential's four RFC/SHA role products in every
lane, replay its four bound segments and thirteen compact-CA boundaries, and
compare native base-column fingerprints before and after binding. A separate
maximum-fixture control exercises the complete bound-source constructor before
masking or commitments, and rejects the prior broken profile. These normal
Core controls run separately from the isolated checks below; their outcomes
and the newly exposed native stack failure are recorded at the end of this note.

The exact-source schedule isolation first reproduced all five original
SHA3-384 shape digests byte-for-byte. The corrected compiler changes only
manifest fields 22 through 26; the other 24 fields, P-256 schedule and
16,935-byte frame remain unchanged. Independent SHA-256 framing derives
`f82e78a995ce1b9ca1e91628901e9acd9b01a6841c13e062e30ad6dfdf028795`.
The new pin is captured with the mapping repair. Three actual-source isolated
controls pass in 16.821 seconds: call/role substitution rejection, every issuer
call row's four RFC event streams across all five shapes, and native/algebraic
fixed-column parity at the complete schedule boundaries. The retained binary
has SHA-256
`d61a30a45e2f257c9c767877a0578a5f8bcc8a44197a87f14226085d84504eec`;
all fifteen scoped source hashes remain unchanged. Evidence is under
`dist/zk-remediation/2026-09-29/x509-rfc-sha-binding/profile-derivation`.

The five-file frozen amendment has a 20,879-file source manifest with SHA-256
`ca122e78c4685e8d107751bcf5e6aa40867abae886aa2a70ca2a9fc4617754e8`.
Its first normal debug build stopped before Core in 7.968 seconds with no source
drift: Cargo marked a current-checkout Norito derive artifact fresh against
the historical candidate, emitting calls to `trace_struct_decode` absent from
the candidate's Norito and derive source. The attempt is retained under
`dist/zk-x509-prover-evidence/frozen-rfc-sha-binding-debug-20260929`.
No candidate source is changed to accommodate this cache mismatch. The
debug-only workspace cleanup dry run reported 35,963 files and 77.6 GiB and
deleted nothing. The coherent retry uses the initially empty candidate-specific
target `target/zk-x509-rfc-sha-debug-20260929`, preserving the shared debug cache.
Its driver requires every candidate-local artifact to be freshly compiled and
retains local dependency bytes and hashes with the executable. This retry
passes in 934.264 seconds with no source drift and no compiler warnings:
all 63 local artifacts are newly compiled, with 109 dependency files retained.
The 1,510,818,880-byte executable has SHA-256
`2cbe74a0bcc22e19e490cb88c0d565231ce066b323cf58ec7af5ee6593b1bed2`.
Its evidence is retained under
`dist/zk-x509-prover-evidence/frozen-rfc-sha-binding-debug-clean-20260929`.
The focused profile/mapping selection passes all nine controls in 305.008
seconds, with 564,576,256 bytes of peak RSS. The actual maximum RFC/SHA role
and segment handshake also passes in 170.710 seconds, with 1,324,105,728 bytes
of peak RSS. These debug timings do not qualify complete-prover performance.
A test-only
follow-up selects populated SHA word/value column zero and asserts it is
nonzero before comparing fingerprints; the first debug capture's probe used
a fixed-column index in the base-column address space and does not establish
that coverage. This follow-up is included in the next frozen amendment below.

The complete bound-source test aborts on the default native test-thread stack,
before a libtest summary, after 202.43 seconds and 4,967,841,792 bytes of peak
RSS. The OS trace identifies `P256MainBaseSourceV1::bind_v1`, called directly by
the MAIN bound-source constructor. Static disassembly of the exact retained
binary shows a 225,392-byte outer frame and a 1,227,472-byte closure frame,
before caller frames. Both P-256 phase owners embed five large signatures
inline, including their reduction rows; conversion from the already reserved
vector and return through the closure duplicate that aggregate on the stack.
The filtered crash frames and both prologues are retained beside the debug
build as `maximum-bound-source-stack-abort.json`, `p256-bind-prologue.txt` and
`p256-bind-closure-prologue.txt`. This failure remains distinct from the ten
passing mapping/role controls. No increased stack size is used as a workaround.

Independent IO/projection known-answer tests ran from the same coherent binary
without traversing the P-256 bind. Both verify, re-encode exactly and pass unique
query assertions, then fail only at their intentionally stale hash literals.
The new projection hash is
`76ddd975964d782a842d9b488246634a5ae89c27b943c81e9e5dc133aaf87b39`;
the IO hash is
`72f054cad978ea9d15e6561a56fb081df01cd930b1dd81afadf29a477374c480`.
Their failed original runs and derivation are retained as `independent-*` files
in the same debug evidence directory; the updated literals still require a
rebuilt passing run.

The reviewed P-256 repair retains each fallibly reserved five-signature vector
through the base and bound phases. Phase guards enforce exact cardinality;
terminal construction borrows the fixed-size view without copying signatures.
Capacity accounting includes actual vector capacity and both vectors during
the bind transition, while child matrices move between owners and retain their
recursive clearing and poisoned single-use semantics. New default-stack
controls cover inline owner size, cardinality, unused vector capacity and
cleanup after rejected binding and unwind. The maximum default-stack bound
constructor is the required regression; its passing native rerun is below.

Exactly three files—P-256 ownership, the two known-answer literals, and the
populated fingerprint probe—were sealed as `x509-rfc-sha-binding/review3`.
The 20,879-entry manifest has SHA-256
`dc095da52097bd8054cc1af43052962f1ab901bb2bab51f6c3827db3bdbbddf3`.
Nine source-geometry controls still pass. The normal debug fresh-Core rebuild
passes in 494.934 seconds with no source drift or compiler diagnostics, under
`dist/zk-x509-prover-evidence/frozen-p256-owner-debug-20260929`.
All 62 reused local artifacts match the exact compiler metadata and retained
bytes from the preceding clean candidate build; Core is freshly compiled and
109 dependency files are retained. The 1,497,535,184-byte executable has SHA-256
`0fc92ecb307f020c63ef74794bb25db669d28ff42a98c09987118e157111cadb`.

Five default-stack owner controls pass in 2.941 seconds. The actual inline
base and bound owners occupy 1,072 and 6,248 bytes, with individual signatures
of 19,616 and 24,192 bytes. Disassembly records 6,400 bytes for the outer bind
frame and 138,928 for its closure; these static sizes exclude caller frames and
are not runtime stack high-water measurements. Four profile/resource controls
pass in 54.703 seconds: the whole assembly remains 200,816,874 bytes against
596,974,144, while the updated source forecast of 5,382,964,662 bytes and serial
scratch forecast of 991,642,712 bytes fit the same allowances. The compiled
profile pin remains unchanged.

The actual maximum bound-source constructor passes on the default libtest stack
with no `RUST_MIN_STACK` override in 304.428 seconds, recording 5,679,136,768 bytes
of peak RSS. It preserves the shared P-256/SHA challenge binding, exact DER/RFC
and RFC/SHA handoffs, and both negative controls. The corrected maximum SHA
fingerprint/role/segment control passes in 176.294 seconds, with 1,281,228,800
bytes of peak RSS; column zero is populated in every segment and identical
before and after binding. These debug timings do not qualify the complete
optimized prover's 300-second target. Both updated known-answer checks pass in
268.373 seconds, with 634,601,472 bytes of peak RSS. All thirteen selected
controls pass with zero failures or ignored tests. Their complete receipts are
collected in `focused-summary.json` in the same evidence directory.
The ordinary opt-level-three build passes from the unchanged reviewed manifest
in 4,733.766 seconds, using two Cargo jobs and an initially empty
candidate-specific release directory. All 63 local artifacts are freshly
compiled, with no source drift; 109 dependency files are retained. The
522,904,176-byte executable has SHA-256
`91284620f1fd1deaf9b836de34fc5e2d4c7f131419cd7dac9405cba12472d072`.
All 109 selected optimized controls pass with no failures or ignored tests,
using 306.019 seconds of summed test-process wall time. The maximum RFC/SHA
handshake passes in 11.190 seconds and the default-stack full binding in
21.774 seconds with 5,627,084,800 bytes of peak RSS. All 57 RFC constraints,
ordinary/maximum complete column sweeps (105.331/106.444 seconds), profile and
assembly admission, and 23 engine/codec/protocol known-answer controls pass.
Receipts and `controls-summary.json` are retained under
`dist/zk-x509-prover-evidence/frozen-rfc-sha-binding-opt3-clean-20260929`.
The actual full maximum proof started at 11:13:04 UTC and failed at 12:09:13 UTC
with `MainProofConstruction(ConstraintOpening)` during composition. Producer
time was 3,368.579386 seconds; external wall time was 3,368.767274 seconds,
with 9,639,247,872 bytes of peak RSS. The 300-second target failed; memory
stayed below the unchanged 12-GiB ceiling. No proof was produced or verified.
All 20,879 frozen source files remained unchanged. The terminal assessment,
phase analysis, copied public receipt, and source/binary closure remain in the
same evidence directory. This fixed candidate is the scoped baseline for the separately
reviewed allocation-erasure amendment below.
The profile pin, AIR and proof/memory/time limits are unchanged by this owner
repair. Current-Core integration and release activation remain unavailable.

A subsequent ownership review found a remaining erasure limitation: moving an
inline reduction or low-S trace with `Option::take` can leave its old payload
bytes in the base signature allocation after the discriminant becomes `None`.
The thirteen passing controls establish the stated binding, stack and live-child
clearing results; they do not establish clearing of those vacated slots. The
retained optimized candidate remains fixed as the failed scoped baseline. A separate
repair guards construction temporaries and both phase-owner vectors, drops live
pointer-bearing children first, then uses the existing zeroize implementation
to wipe the full allocation including spare capacity. Physical allocation
observations and normal candidate qualification for that repair remain pending.
No proof, performance, erasure-completeness or release claim follows from the
failed baseline alone.

The separately reviewed allocation owner reserves capacity before inserting
private values, exposes only mutable slices, and rejects insertion when full
without reallocating. Its destructor drops live children before wiping the full
capacity with the pinned zeroize helper. The test-only byte observer has an
explicit unsafe precondition and runs after the real wipe without changing
its result. Seven exact-source helper controls pass with the actual zeroize
dependency and a field-erasure stand-in; these do not qualify P-256 integration.
The two-file amendment and independent source review are retained under
`dist/zk-remediation/2026-09-29/x509-signature-allocation-erasure`, including
`bounded-helper-controls/receipt.json`. The frozen baseline has not been
amended. Native allocation-owner and default-stack binding checks, followed by
the complete optimized proof, remain required on the repaired candidate.

The completed baseline attributes 442.104174 seconds to base masks,
558.477729 to base commitment, 769.593118 to auxiliary masks, 944.937671 to
auxiliary commitment, and 603.991552 to interrupted composition. These phases
are nested and must not be added. DEEP/FRI, query openings, and envelope
self-verification were not reached. Metal completed 1,455 calls for 5,811
columns under the existing four-column device policy. The 2,510 fixed forward
columns, 1,247 fixed recovery columns, and both butterfly totals exactly match
the canonical registration prefix through all eight quotient stripes of SHA
segment zero. This localizes the failure before SHA segment one's first stripe.

A further source-reviewed amendment removes repeated P-256 arithmetic
validation during native column replay. A private immutable owner first runs
the existing complete arithmetic and role-topology checks under a clearing
guard, retains the checked public schedule, and lends lifetime-bound row views
to base and auxiliary readers. Arbitrary borrowed traces still use the checked
constructor. Auxiliary challenge validation and terminal recomputation remain
in their original order. Actual retained schedule capacity and simultaneous
validation allocations are charged against the unchanged allowances. The
source-only review, exact two-file delta, and proposed controls are retained
under `dist/zk-remediation/2026-09-29/x509-p256-validated-owner`.

The prepared three-file owner-only capture was superseded without being run.
A further source-reviewed amendment reuses only the bound owner's already
derived arithmetic terminals and streams at most eight adjacent auxiliary
columns into the existing batch allocation. The checked raw constructor remains
the oracle; no additional eight-column allocation, unchecked public constructor,
AIR, geometry, transcript, or resource-cap change is introduced. Its four-file
delta and source review are retained under
`dist/zk-remediation/2026-09-29/x509-p256-auxiliary-replay`. Native every-cell,
seeded masking, commitment, DEEP, and opening parity remain pending.

A separate correctness diagnosis found that the final native SHA padding row
has zero products and wraps to a live first row whose products are one, while
the recurrence gate excluded only the last live row. The exact frozen complete
796-residue SHA/word/RFC AIR reproduces 24 nonzero residues for all four segment
contexts, in both base-field and Fp4 evaluation. The corrected complete AIR uses
`1 - segment_last - physical_padding`, produces zero residues at this edge,
and rejects every nonzero padding base/auxiliary cell in the isolated controls.
The padding-zero, first-product, and terminal constraints remain. The full-AIR
source captures, binaries, logs, counter reconciliation, and pending native
regressions are retained under
`dist/zk-remediation/2026-09-29/x509-sha-cyclic-padding`. These are isolated AIR
controls, not a complete source/proof qualification.

The semantic repair explicitly updates the SHA descriptor. Independent framing
changes only field 17 of the 29-field manifest, yielding candidate pin
`9d2d34512de90d13a0f68d352bbcc887ba9ac2f2a89e5deb845ff5c4c64d45ff`.
Actual native constructor confirmation, rejection of the superseded digest,
and dependent IO/projection KAT derivations remain required. Fixed schedule
geometry, constraint degree, and proof/memory/time limits are unchanged.

The next jointly reviewed candidate combines these separately retained
correctness and performance amendments. Normal controls must include the
actual maximum-source SHA boundaries and mutations, default-stack maximum
binding, and rejection/erasure checks. Optimized qualification must explicitly
execute the otherwise-ignored every-cell and commitment/DEEP/opening parity
controls before the full maximum proof. Capture remains pending native
qualification planning and review. No measured speedup, proof-time compliance,
current-Core, activation, or release claim follows from source review.
