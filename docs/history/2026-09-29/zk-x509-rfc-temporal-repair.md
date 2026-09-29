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
Scoped formatting passes; native reruns of the repaired controls remain pending.
The prepared ordinary opt-level-three build will retain an immutable executable,
rerun the exact controls and full-column source preflights, then attempt the
complete maximum credential proof. No successful complete proof, 300-second
latency result, 12-GiB RSS result or activation claim follows from these debug
results.
