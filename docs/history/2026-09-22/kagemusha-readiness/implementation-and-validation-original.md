## Active implementation — 2026-09-13

Work remains confined to `/Users/takemiyamakoto/dev/iroha`, branch
`optimizations`. The encrypted polynomial-store foundation, bounded
write-once advice assignment and single-column basis conversions are now applied. The store authenticates the proof
context, field, basis, column/phase, ordinal and chunk geometry. Its 512-handle
limit counts live owners; dropping a predecessor releases capacity without
reusing its ordinal. Assignment buffers one 256-row numerator/denominator chunk
per column, preserves rational zero-denominator behavior and ordered tail
values, and destroys the active writer after an operational error or unwind.

The existing prover still owns its full scalar banks. These new components are
not yet a stored consuming prover, and their logical buffer bounds do not
establish full-process RSS, latency or production qualification. The actual vendor suite passes 25 tests in each of default and no-multicore
configurations (50 executions), covering metadata, both-field assignment
comparison, all basis-conversion pairs and failure/unwind handling. The new
basis conversion retains one guarded field column and uses the existing baseline
FFT arithmetic, preserving current prover dispatch. The encrypted Core adapter
passes all 16 tests for authentication, owner lifetime, assignment and basis
conversion on the freshly rebuilt development harness. All 1,827 captured
Core/Crypto/Axiom/curve and build/lock inputs remained unchanged throughout
compilation and execution. These results do not qualify the
existing MSM scratch lifecycle or any complete proof.

A later implementation batch is applied after the validator source
capture. It adds ordered IPA phase commitments, bounded expression and
retained-graph tiles, MSM scalar-encoding cleanup, Base assignments returning
only actual cells, and monotone native Poseidon BUS emission. The graph helper
caches advice tiles and retains one row of graph intermediates. The BUS retains
one block of guarded endpoints and replays its original copy order. The block uses
the existing safe zeroization API; the cleanup regression covers both Pasta
fields on normal return and unwind. These BUS regressions remain unexecuted.

The batch also includes a strict internal single-phase Assignment bridge and
consuming completion into globally ordered receipts. Unknown witnesses,
reference-return requests, backward writes, ignored instance errors and premature
phase changes invalidate the whole assignment owner. Selector/fixed/copy calls
retain the ordinary prover's PK-frozen semantics, including direct selector
conversion that removes the original selector count. Completion preserves the
original snapshots and guarded blinds; a failed or unwound bounded read destroys
all receipts. Concrete Core producer/key admission remains incomplete. The owned
single-phase prefix below now enforces final-producer destruction and external
synthesis-unwind cleanup for its entry point; the standalone phase primitives
still require their caller to provide those guarantees.

The claim carrier RLC now emits one guarded logical row at a time and retains only
cell metadata for its equality bindings. It preserves the full 4,090-value capacity,
physical row order and sorted pack-copy order. The removed vector held at least
26.4423 MiB of scalar payload; this is source accounting, not measured RSS. Eight
new tests compare the frozen vector oracle and small seeded proofs in both Pasta
fields, and exercise failure/unwind cleanup. They remain unexecuted. The legitimate
standalone curve lock was generated from its complete manifest graph without
changing dependencies or discarding development targets.

Completed advice receipts now feed bounded expression and retained-graph tiles
through a private chunk reader. The owner is consumed before admission or reads
and restored only after a successful evaluation and consumer return; errors and
unwind destroy all receipts and blinds. Admission binds the retained field,
domain, Lagrange basis, advice layout, instance dimensions and exact challenge
values and phase schedule, including empty advice. Seven new tests compare the
original arithmetic in both Pasta fields and exercise preflight, backend,
consumer, corruption and unwind failures; all seven pass in both Axiom feature
configurations. Fixed/key and
instance provenance, coset ownership and full consuming-proof integration remain
unfinished.

The required full-proof and device limits remain unchanged. The complete stored
prover and producer memory work remain incomplete. Before the owned prefix below,
the Axiom harness passed all 102 selected tests in each of
default and no-multicore configurations: 69 new tests and 33 retained checks per
run, 204 executions in total. Compilation exposed six fixture type errors;
the corrected generic bounds and helper types preserve all assertions. Captured
vendor sources, planned retained-test inputs and each compiled harness stayed
unchanged through execution. The curve suite also passes all 13 selected tests
and Base passes all 10. The remaining 14 new Core tests await their build window.
The earlier 50 vendor and 16 Core executions above precede
these changes. These focused host results do not qualify the complete proof;
formatting, source review and patch checks are not proof equivalence.

The curve run exposed a valid-input defect: large MSM sent identity bases into
an affine batch routine that assumes nonidentity points. Such inputs now use the
existing complete parallel implementation. The original BN256 regression and
new mixed/all-identity cases in both Pasta fields pass. The cleanup test now
checks explicitly unfilled slots and exact fill/drop correspondence instead of
assuming that one Rayon worker stops all queued jobs at a panic. Axiom's two
102-test suites and Base's 10 tests were rebuilt and rerun after this fix, giving
227 passing focused executions on the corrected dependency before the numerator
codec below. Those results do not establish full-proof latency, process RSS or
device qualification.

Completed Base numerator segments now store zeros and ones as two-bit tags,
with bounded rank checkpoints and exact remaining field values. The active tail
stays dense; a completed segment stays dense when actual allocation capacities
show no saving. Exact Assigned variants, denominators, cell identities and
physical schedules are preserved. Owned initialized numerator/tag/rank buffers
are erased on release; this does not cover caller copies or all process memory.
Allocation counters include retained field slots, metadata and transition
coexistence; their values and access timing depend on witnesses.

The updated Base harness passes 47 focused checks: 13 new codec tests, 23 retained
storage/cell checks, the preceding 10 assignment checks and one explicitly enabled
k=16 real-proof regression across the 65,536-cell boundary. The latter preserves
complete VK/PK bytes, physical coordinates/copies and seeded proof bytes against
the original raw Assigned schedule, with cross-verification. It is a synthetic
BN256/KZG fixture; both Pasta fields have exact storage differential coverage.
The current Base library also passes strict Clippy with warnings denied, after
equivalent lookup-option, saturating-arithmetic and default-construction cleanups;
all 47 checks were rerun on that source. This is a library-scoped lint result.
These vendor selections total 264 passing executions before the owned prefix
below. At that boundary, 93 of 107 distinct new tests had run; 14 new Core tests and the updated allocation
diagnostic await the Core build window. Full Claim memory and latency remain
unmeasured for this codec; no acceptance limit is relaxed.

An internal consuming IPA prefix now keeps the exact owned proving key, completed
advice, storage provider, RNG and transcript together. It derives the assignment
schedule from that key, runs the original floor planner, then drops the original
producer before instance commitments and final-phase randomness. Instance
commitments use one guarded polynomial at a time. The original domain API creates
that polynomial; source values are copied only after its cleanup guard exists.
Errors and unwind discard the partial continuation. Structural configuration
checks do not authenticate a circuit relation or parameter artifact, and the
pending owner exposes no proof or complete suffix.

All five prefix tests pass in default and no-multicore configurations, comparing
ordinary consuming-proof transcript/RNG frontiers in both Pasta fields for
direct, queried and hybrid instances, with compressed and uncompressed selectors.
Malformed inputs, multiphase keys, ignored assignment errors, backend/transcript
failures and unwind exercise rejection and owner destruction. The full focused
Axiom selections pass 107 tests per mode (214 executions), including 74 distinct
new tests. Together with the preceding Curve 13 and Base 47 runs, the latest
package selections total 274 executions; each retains its own source receipt.
Of 112 distinct new tests prepared across the batch, 98 have run. Concrete Core
artifact/producer admission, stored argument commitments, key-bound polynomial
conversion, quotient/openings and full proof/resource qualification remain open.
Application and static receipts are under
`target/kagemusha-validation/stored-prover-next-window-20260912`.

The subsequent candidate applies consuming advice-coefficient staging and
key-derived lookup-expression tiles. Fixed values come from the retained key;
original instance prefixes are padded on access without another polynomial bank.
Successful consumers must also pass a final check of every live advice identity
before the complete owner is restored. The 15-file change passed source review,
exact patch application and formatting checks. After correcting one test-only
Polynomial range from `..len` to its supported `0..len` indexing, all 21 new tests
pass in both feature modes. Each complete Axiom selection passes 128 tests
(256 executions), with zero failed/ignored tests and unchanged captured source,
controls and harness inputs. Compiles took 18.535 and 12.941 seconds; execution
is separately recorded, including the serial phase selection's 81.622 seconds.
The maintained batch has 133 distinct new tests prepared, 119 executed including
prior Curve/Base evidence, and 14 Core tests still unrun. The bounded Axiom window
was returned; Core and additional builds require a new explicit release. Exact
receipts are `axiom-coefficient-auxiliary-default-r2/result.json`,
`axiom-coefficient-auxiliary-serial-r1/result.json` and
`coefficient-auxiliary-final-r1.json` under the validation directory above.

The subsequent combined 34-file candidate is now applied. Stored polynomials
carry authenticated advice or lookup/index/side roles; basis transforms preserve
those roles and advice-only boundaries reject lookup receipts. Consuming lookup
compression preflights the exact retained-key expressions before squeezing theta
once from the original transcript, then emits input/table tiles in key order.
It retains the original advice/coefficient receipts, parameters, key, instances,
blinds and RNG owner. Errors and unwind destroy the continuation and guarded
scratch. This stage draws no proof randomness or commitment points. Core retains
its existing encrypted spool, shared window, ordinal allocator and 512-handle cap.

The full Axiom selection passes 167 tests in each of default and no-multicore
configurations (334 executions). Both source/build/harness captures remain
unchanged. The first compile failed on two missing trait imports; adding
`PrimeField` in lookup compression and `ParamsProver` in one unit-test module
resolved those errors. The corrected builds emitted no compiler diagnostics.
Receipts are `axiom-combined-role-lookup-default-r2/result.json`,
`axiom-combined-role-lookup-serial-r1/result.json` and
`combined-axiom334-pass-r1.json` in the validation directory above.

Core compiled successfully in 407.88 s with unchanged captured sources, but the
validation script then misresolved a workspace-relative dependency path and
stopped before listing or executing tests. All 53 selected Core checks remain
unrun, including the 19 new Poseidon/RLC/role checks and the updated allocation
diagnostic. The compiled artifact and failed admission are retained in
`core-combined-role-lookup-r1/terminal-result.json`. The CPU window was returned;
this compile alone supplies no Core runtime or allocation evidence. Reviewing
the saved dependency closure also found that the pre-build snapshot omitted
`javascript/iroha_js/test/fixtures/race-v1-codec.json`. Future validation must
capture that fixture before compilation; its current hash cannot retroactively
admit the earlier build. A fresh Core capture, compile and test run is required.
The target-local runner correction passes 25 Python tests and 58 subtests. It
binds relative paths to the compiler artifact's exact crate root, rejects
ambiguous or aliased anchors, confines generated inputs to the selected target
and adds the omitted fixture to future Core snapshots. Its saved-input replay
still rejects the earlier Core capture; no Cargo or Core harness was invoked.
The scoped receipt is `dep-info-workspace-fix-r1/result.json` in the validation
directory above.

A separate seven-file advanced-journal recovery candidate prepares a consuming
paired reopen with 15 new tests. It remains private, unapplied, uncompiled and
unrun under `target/kagemusha-validation/advanced-journal-recovery-20260913-r1`.
The current concrete freshness verifier authenticates signed checkpoint/prefix
values; qualified native verification of held journal bytes and speculative
response suffixes, and actual native recovery wiring, remain incomplete.

The [release-runner validation record](kagemusha_v1_release_runner_validation.md)
covers subsequent Python deadline, cleanup, argument-admission and recovery-metric
corrections. Lookup permutation, argument commitments/products, cosets,
quotient/openings, authenticated Core admission and complete-proof/device resource
qualification remain outstanding.

The physical-evidence verifier also rejects reuse of any earlier boot identifier
across its restart and power-loss prefix. All ten older-boot substitutions were
accepted before the fix with valid synthetic observer approvals. The corrected
verifier passes 195 tests and 144 subtests across the physical-device, release
evidence and provenance suites, including those ten rejections and five fresh
boot replacements. Inputs were unchanged throughout the run; these are synthetic
gate tests, not physical qualification. Evidence is under
`target/kagemusha-validation/physical-boot-freshness-20260912-r1`.

Before this batch, the native enrollment library and test sources passed a
separate `cargo check`. That check does not validate these later Core/vendor
changes; linked test execution and native artifacts remain pending. The six package
surface tests pass after correcting two stale JavaScript public-subpath
expectations, preserving all coordinator enum and fixture checks. These scoped
receipts are copied under `target/kagemusha-validation/mobile-taira-reconnect-20260912`
and `target/kagemusha-validation/package-public-path-20260912`.

The explicit Apple local-integration build path now accepts the current root
lockfile while retaining locked/offline builds and authenticated source/tool
boundaries. Its fixed owner-only output paths remain inside this checkout.
Every resulting artifact is marked `local-integration`; archive and release
publication reject that scope. This enables native integration work without
conferring canonical release provenance. Fourteen local-policy tests, all 29
archive tests, 24 source-seal tests, 12 Swift-pin tests and 19 strict validator
tests pass (98 in total). Native artifact
construction, complete Swift execution and canonical release qualification
remain pending. The archive tests use an isolated fixed source graph and
independently encoded expected ZIP bytes; release path guards remain enforced.

Current application and execution receipts are retained under
`target/kagemusha-validation/20260912-source-window`,
`target/kagemusha-apple-archive-fixture-followup` and the Apple follow-up test
records. Prior source-bound results below remain historical scoped evidence.

## Current validation boundary — 2026-09-08

The canonical checkout is `optimizations`. The completed R5 SDK chain at
`e7a8083753a46bad47535816fff7fe7d29ba8b05` includes freshly invoked host bridge
and fixture-generator builds, 16 direct balance-key fixture cases, 91 focused JVM
tests, 1,490 full JVM tests, 12 managed wallet Android tests and 108 managed client
Android tests. All executed tests passed with no skips; focused and full JVM
counts overlap. Its source and artifact boundaries were retained. The resulting
five repair files were committed in `8c322866d0556060f4794ab5d178af2acaba1a92`.
A separate Android host JNI test also passed actual software-key generation,
reload, signing and verification. These are host/managed SDK results, not device
or monetary qualification.

After the release71 source hold ended at
`f0322420c46aa6cfc35b37b4b0c4abe57457817f`, the Kotlin retry-archive correction
below passed all 13 focused tests. JavaScript Parliament parity passed all 24
tests, including the TypeScript consumer check for `RegisterInitialSortition`.
The corresponding Swift source and tests were restored to the canonical checkout
after the clean native checkpoint. The required canonical ABI-23
`dist/NoritoBridge.xcframework` is absent, so Swift execution remains open.
Neither focused SDK result rebuilds native proof authority for later Core changes.

The reviewed vendor memory candidate is applied. Its actual default and
no-multicore runs each passed 45 regular tests, 12 additional proof-test
executions and 12 lookup-test executions: 138 executions in total. The runs
retain exact source, executable, recipe and index boundaries. Test-execution
counts are not counts of constructed proofs. The exact Core structured-key
overlay passed 92 regular Core tests and two separately executed row-emission
benchmarks from the shared native build. All 54 journal/recovery tests also
passed. The real-proof API check exposed a test-only helper guard; extending
that guard to the existing profiling feature fixed the compile, and the focused
retry passed. The method body and tested behavior are unchanged. This is
focused validation, not a workspace or warning-free Clippy result. The separate
Base lane passed 14 regular tests, including small BN256/KZG key and seeded-proof
equality against the original map, and two separately executed benchmarks.
Its offline lock generation left the then-current root dependency graph
unchanged; one test-only trait import fixed its initial compile failure. The historical 5,302,980-cell map benchmark retained an identical
checksum while process RSS fell from 557,727,744 to 4,145,152 bytes. These are
unoptimized host map measurements. The separate 1,008-source Core row benchmark
retained the exact 131,046-row checksum while peak RSS fell from 189,562,880
to 28,262,400 bytes; both runs took about 52.6 seconds. Neither benchmark
measures full-proof or device resources.
Full-process memory/latency and the required genuine aggregate proofs remain
open. Local receipts are retained in
`target/kagemusha-main-native-jvm-validation-r5`,
`target/kagemusha-sdk-security-parity-validation-r1` and
`target/kagemusha-validation/kagemusha-combined-memory-r2-actual-20260908` and
`target/kagemusha-proof-work-inventory/base-test-lane/actual-r2`,
`target/kagemusha-validation/kagemusha-core-structured-shared-r2-20260908`,
`target/kagemusha-validation/kagemusha-wal-shared-r1-20260908` and
`target/kagemusha-validation/kagemusha-api-cfg-retry-r1`.

