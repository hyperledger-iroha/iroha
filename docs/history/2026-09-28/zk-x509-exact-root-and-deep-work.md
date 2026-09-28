# X509 exact-root transform and DEEP work, 2026-09-28

This record is scoped implementation evidence. The complete X509 profile remains
unavailable pending full proof, resource, soundness and independent qualification.
Neither kernel timing nor source operation counts qualify the 300-second target.

## Existing Metal transform baseline

On the M1 Ultra (20 CPU cores, 128 GiB), the ignored
`goldilocks_transform::tests::required_metal_native19_common22_exact_root_parity_and_timing`
test passed against immutable binary SHA-256
`7a5cc1733bddc04d8da66075c439c3671ba0fda02cfbb482c5e21a83fca45d64`.
The diagnostic ran from 13:41:01 to 13:41:44 JST. Multiple Cargo/rustc jobs were
active; this was not an exclusive-host throughput measurement. The separate
1.35-second Metal parity suite at 13:43:30 did not overlap this run.

| Eight resident columns | CPU | Existing Metal |
| --- | ---: | ---: |
| Native log19 inverse | 54.07 ms | 824.28 ms |
| Common log22 forward, dispatch8 | 558.21 ms | 6.092 s |
| Common log22 forward, dispatch4 | same CPU reference | 11.536 s |
| Common log22 forward, dispatch2 | same CPU reference | 22.732 s |

All complete outputs matched CPU, including the 1,816-coefficient mask extent;
independent Horner controls passed. Timing includes dispatch, staging, waits and
cleanup, and excludes source construction, coset packing, hashes, constraints,
CA and the complete proof. `/usr/bin/time -l` reported 42.90 seconds and maximum
RSS 1,667,268,608 bytes for the whole diagnostic.

The binary predates the final successful-wait timeout refinement in Metal ticket
cleanup. Compilation overlapped that refinement, so no whole-source digest is
claimed for this intermediate binary. The forward/inverse arithmetic kernels
were unchanged by that refinement. MAIN's production factory was changed to
CPU after this measurement; the subsequent staged-kernel result below justified
restoring bounded automatic Metal selection.

The authoritative commitment hash is SHA3-384. The earlier exploratory estimate
of 161.7 billion custom Poseidon permutations was incorrect and is retracted;
it is not a workload floor or a measurement.

## Staged Metal exact-root kernel

The replacement kernel dispatches independent tiles and globally ordered
butterfly stages. Actual Rust API measurement passed complete output equality
and independent Horner checks in immutable binary SHA-256
`0b3af1fae8e97f2ca4623af433759c167f9a64fb48d935bad7a15651f964832b`.
Eight resident native-log19 inverse columns took 41.789 ms on Metal versus
98.599 ms on CPU. Common-log22 forward columns with the same high mask extent
took 119.840 ms with dispatch8, 102.466 ms with dispatch4 and 130.368 ms with
dispatch2, versus 690.993 ms on CPU. These timings include staging, waits and
clearing. The diagnostic took 2.92 seconds with approximately 1.365 GB maximum
RSS under concurrent builds and two FASTPQ facade runs; it does not measure a
complete X509 proof.

A subsequent test-only correction to a `Weak` ownership observation produced
binary SHA-256
`28f8886c0462dfe0c5ca9ff9bd939990d0c37cea88646301854bbffe73eb0720`
from 211-file source digest
`dd9804ca91b75e416b141f7331789c9a66ebb9f1bc81763657da5b9f2ba2b554`.
All 32 selected tests passed, including actual Metal success/error/unwind
clearing, original-input preservation, FFT/IFFT equality, ticket deadlines and
uncertain-completion quarantine. The arithmetic and production ownership source
did not change between the measurement and these corrected test observations.

MAIN selects Metal only when its complete extra staging allowance fits beyond
the original source, scratch, runtime and arithmetic admission. The measured
structural fixture admits two device columns while eight outputs remain
resident. Unsupported backends or insufficient spare capacity use CPU;
uncertain device completion remains terminal even for a later CPU proof.
No memory ceiling or runtime reserve was increased. Native IFFT batching still
uses the bounded CPU path; this selection accelerates common-domain evaluation.

## Complete MAIN work and bounded reductions

The source-bound `complete_main_work_inventory_includes_quotients_and_all_native_replays`
test records the complete 49-registration geometry. Its initial count before
quotient caching includes 53,215,232 quotient evaluation rows, 28,038,635,520 AIR
residue evaluations, 20,126 quotient native IFFTs, 91,140,895,232 quotient native
butterflies, 134,276,390,912 stripe forward butterflies, and 561,381,376 Fp4
quotient inverse butterflies. Two joined commitment passes additionally require
518,860,570,624 common-domain butterflies and 377,353,142,272 row input bytes.
These are algorithm counts, not timed execution or a general protocol lower bound.

The bounded registration coefficient cache retains a public base-then-auxiliary
prefix within existing quotient-stage headroom. Its source-bound census retains
2,893 columns and reduces quotient native IFFTs to 8,281 and their butterflies
to 32,319,713,792. The cache drops before quotient inversion; stripe domains,
residue counts and output order are unchanged. Eleven actual-source cache,
stripe and clearing controls passed independently; Core integration is pending.

Native sources now construct serially into a bounded batch, then eight in-place
native IFFTs run in parallel. Applying original masks replaces one clearing
allocation at a time; no randomness, polynomial, ordering or proof bytes change.
Two optimized actual-source tests passed independent inverse-DFT and original
coefficient parity, native-point evaluation, worker-count parity, partial source
errors, unwind and real-cell cleanup. Integrated Core execution remains separate.

SHA deterministic replay now reconstructs one bounded call traversal for up to
eight adjacent columns, instead of reconstructing it for each column. Extraction
stays within the original eight-column budget and immutable phase capability.
Mask sampling keeps the original source-before-entropy chronology. An
actual-source guard test passed atomic success and full-destination clearing on
late malformed output and unwind; full segment projection/terminal parity and
phase/extent negatives are included for coordinated Core execution.

DEEP replay reuses powers of the exact transcript-bound current/next points.
Every individual opening is still checked, including errors which could cancel
in an aggregate. For each native group, weighted polynomials are combined before
synthetic division. Unequal lengths are zero-padded. The same resulting FRI input
is produced with 1,791,828 recurrence steps instead of 4,056,725,602; per-cell
weighted base-field sums remain. Four optimized actual-source tests passed
original-division parity, independent polynomial identities, distinct native
points, context/claim mutations and live-cell cleanup on success/error/unwind.

Two power arrays, two weighted arrays and eight coefficient owners use at most
101,011,968 bytes, below the existing 310,494,720-byte replay allowance. No source,
runtime, memory, proof-byte, query or degree limit is relaxed. Integrated full
Core and maximum-proof validation remain outstanding for this slice.

The optimized actual-source `native19_eight_column_deep_grouping_parity_and_timing`
diagnostic also passed full coefficient equality for eight dense columns of
526,104 coefficients. Old individual Horner evaluation took 421.13 ms; shared
power construction took 56.91 ms and dot products 50.26 ms. Original divisions
took 724.58 ms; grouping including all individual claim checks took 192.71 ms.
The standalone actual-source Rust harness took 1.67 seconds with maximum RSS
137,625,600 bytes. This is a kernel comparison under shared host load, excludes
all source/FFT/commitment/proof work, and does not substitute for Core integration.

## Registration-local quotient coefficient cache

The quotient producer now retains a public base-then-auxiliary prefix for each
registration and replays only its uncached suffix on subsequent stripes. It
retains the original masks and coefficient order and drops the cache before the
full quotient IFFT. Single-stripe registrations do no redundant preload. The
budget subtracts masks, the unchanged replay allowance and the conservative
registration-stage charge from the already-admitted arithmetic maximum. Source,
scratch and runtime allowances are never treated as spare cache capacity.
Actual coefficient capacities, clearing-owner headers and the cache struct are
charged. Construction failure, partial replay and unwind all retain clearing
ownership.

The independent actual-source optimized harness passes all 11 cache, stripe and
private-owner controls in 0.08 seconds, including every row against independent
Horner evaluation for zero/partial/full prefixes, malformed source responses,
capacity overflow and observation of real cells before/after erasure. Its runner
is `/tmp/iroha-quotient-cache-kernel-20260928/run.py`; `receipt.json` and
`source-sha256.json` in that directory retain scope and source hashes. This
compiles the actual arithmetic/owner/cache sources with namespace/error glue;
it does not compile the full Core layout integration or generate a credential.

The public source census and pending native all-49 layout test expect 2,893 cached
columns, reducing quotient native IFFT replays from 20,126 to 8,281 and native
butterflies from 91,140,895,232 to 32,319,713,792. P-256 arithmetic retains 149 of
283 columns with 630,895,488 bytes available; RFC5280 retains 161 of 377 with
681,227,136 bytes. Smaller repeated families fit entirely. The arithmetic maximum
remains 3,694,852,800 bytes and source/runtime envelope 9,190,049,088 bytes.
Constraint rows, residues, stripe FFTs, proof parameters and encoding are
unchanged. Integrated Core and full-proof timing/resource qualification remain
pending; these operation counts are not measured end-to-end speedups.

## Constructor ownership and complete proof diagnostic

The follow-up source audit found private buffers that preceded final clearing
owners: partially decoded witnesses, DER/RFC parsing and semantic projections,
projection channels, byte-memory materializers, assembly disclosure and P-256
inputs, and partial CA rows. These construction paths now establish clearing
ownership before private writes and before later validation can fail. Fixed
extents are reserved before writes; growing DER/RFC tables use bounded
copy-preserve-clear replacement. Successful transfers keep the original values.
This covers the named owned buffers, not every register or compiler-created copy.
No relation, transcript, format, query, degree or resource limit changes.

The final actual-source DER harness passes 35 controls, including seven new
constructor/growth/borrowed-span tests. The codec harness passes 14 controls,
including every truncation, late invalid fields and exact reserved encoding.
The integrated Core run additionally selects actual-cell error/unwind tests for
assembly, CA, RFC, projection and IO, alongside the arithmetic changes above;
that run remains pending at this point in the record.

The ignored engine test
`privacy_engines::zk_x509::engine::prover_diagnostic::maximum_structural_credential_proof_with_retained_public_receipt`
is the next complete-proof measurement. It uses the real producer, `OsRng`, the
three-certificate/four-disclosure/64-CRL-entry fixture, and an independent full
credential replay. It retains only the public proof under its SHA-256 filename
and an outcome/timing receipt under ignored
`dist/zk-x509-prover-evidence/<unique-run>/` before checking the unchanged
300-second target. These actual proof artifacts survive temporary-directory
cleanup; the small helper unit tests use and remove temporary directories.
Wrong-genesis and corrupted-proof controls run before the final time assertion.
Run the optimized test binary directly with `--exact`, `--ignored`, `--nocapture`
and `--test-threads=1` under `/usr/bin/time -l`; timing Cargo would include compiler
memory and would not measure proof RSS. Address-space evidence remains separate.
The fixture reaches the stated structural ceilings, not every possible DER byte
length. No completed proof or maximum-resource qualification is claimed yet.

Two optimized actual-source diagnostic helper tests pass. They cover exact
public artifact retention, content-address collision rejection, durable receipt
append and an isolated real-Rayon panic: private panic payloads are omitted from
stdout/stderr, the prior hook is restored, and owned formatted panic strings are
cleared. These helper tests do not construct a credential proof.

## Norito witness cutover and current integration boundary

A subsequent repository-policy audit found that the local witness container used
an unneeded custom grammar. The sole accepted input is now a flags-zero,
uncompressed Norito frame with the explicit nominal schema
`iroha_core::privacy_engines::zk_x509::ZkX509WitnessV1`. The actual derived
serializer and bounded decoder retain the existing typed relation and document,
path, signature and disclosure limits. The largest admitted frame is 20,398
bytes. The old magic is rejected; it has no fallback decoder. Mathematical AIR,
hash preimages and credential proof framing are separate and remain unchanged.
The actual-source optimized codec harness passes 20 controls (16 codec and four
clearing-owner controls); the no-legacy-codec repository guard also passes.

The preparation marker is a field of the compiled profile, so this intentional
first-release change rotates the profile digest and affected deterministic proof
known answers. Native regeneration retains all 29 public manifest fields for an
independent SHA-256 reconstruction before replacing the sole pin. This work is
not evidence for the earlier witness grammar, nor does a pin establish release
qualification. The final complete-proof diagnostic must run the Norito candidate.

The preceding Core cleanup attempt compiled in 12 minutes 23 seconds but failed
while stripping its executable because the filesystem filled. No tests ran; the
4,000-byte result was not an executable. Its log and relevant-source manifest are
retained under ignored
`dist/zk-remediation/2026-09-28/core-cleanup-attempt/`. Following external disk
cleanup, a fresh integrated native profile/KAT command is running with public
artifacts under
`dist/zk-x509-prover-evidence/norito-native-profile-20260928T065630Z/`. Its capture
records subsequent pre-compilation FASTPQ frontier-cleanup changes explicitly;
it is a relevant-source manifest, not an immutable whole-workspace snapshot.

Native regeneration completed: all 16 Norito codec tests and the engine's
malformed-input preflight passed. The three deliberately stale profile/proof
assertions failed after producing their replacement values; this was a
17-pass/3-failure regeneration run, not a passing regression suite. Both proof
fixtures completed independent verification and their encoding/query assertions
before comparing hashes. The run took 253.81 seconds after compilation.

The retained 29-field, 16,789-byte manifest independently reconstructs SHA-256
`bbd1cc3fbefd0f0adba9583100a8213d0c1effc109c176914ee14c31dd4b2a67`.
A second implementation reproduced the exact frame bytes and digest. The sole
profile pin now contains this value. Projection and IO fixture hashes are now
`116ac702fd4e34a929b15290bb76f8d186546b39406e2b806a11238f5de392e3`
and `b3e728c149d3610fbc158dfde86dfb8bd75a0163084bf451bb982596a8e036b8`,
respectively; all existing assertions remain. The public frame, fields, native
log and independent reconstruction are retained in the regeneration directory
above. The current focused Core regression is queued separately under
`dist/zk-x509-prover-evidence/norito-final-core-20260928T112152Z/`; it includes
subsequent dependency changes and the typed Rust wallet change-note helper.
