# Maximum X509 proof on Intel under WSL2

The recorded 2026-10-24 maintained-source candidate generated and independently
replayed the complete depth-three maximum credential proof on the Intel
i7-11800H under WSL2. The run **failed qualification**: proving took
1,752.653518 seconds against the unchanged 300-second target, and live process
observation failed.
Phase attribution now passes, with 0.0147% of root time unattributed. These are
separate outcomes; the completed proof does not repair the rejected harness run.

| Observation | Actual result |
| --- | --- |
| Build | Actual maintained `cargo test --offline --locked --release -p iroha_core_privacy --lib --no-run`; default `zk-stark` and `simd`; eight Rayon workers for proving |
| Proof bytes | 9,412,944; exactly the encoded geometry bound, below the 9,437,184-byte protocol cap |
| Proof SHA-256 | `7b08f6bc08321c1ce20c64a645d657040d8d6a98804442b823585726c37cfafe` |
| Verification | Producer self-check and independent consensus-verifier replay pass; replay takes 4.808441 seconds |
| Negative controls | Wrong genesis, every one of 32 nonce-byte substitutions, and a corrupted proof reject |
| Kernel peak RSS | 8,298,721,280 bytes, below the unchanged 12 GiB limit; this kernel total does not repair failed sampled observation |
| Address space | 32 GiB `RLIMIT_AS`, applied by the harness and confirmed through `/proc` |
| Phase attribution | 257,466,252 ns unattributed out of 1,752,653,517,861 ns; 147 ppm, below the 1% limit; no phase findings |
| Harness verdict | Rejected: child exit 101 and a sample-stage `ProcessObservationError`; no timeout or orphan |

The executable SHA-256 is
`0c15e153e7ab16322873967ee0c192627b404f169d10a29d5c4db9a0c48cb2ec`.
The build and invocation retained the same 23,595-path source census,
`3e8f8fdc225d8f87c8e7597d7b6c8181d6b6c4f7cdf1e02cf00040c9f00701f5`,
and unchanged executable bytes. This is a working-tree execution record, not
an immutable release-source seal. The checkout has an active merge. The harness
launches the recorded pathname; this run does not establish immutable original-
image launch.

Build evidence is in
`target/qualification/zk-x509-maintained-rerun/build-20261024T074253Z-de5432759076/build.json`.
Invocation evidence is in
`target/qualification/zk-x509-maintained-rerun/proof-maintained-sha-phase-observer-20261024/invocation.json`.
Its harness report is under
`harness/run-20261024t080443z-2333023-4f1503a1/report.json`, with SHA-256
`ba7d60ea64a5a51a603711d8dd0129b0f5db52db133eea5ec4f45b48b440b8d9`.
The public proof and receipt remain under
`dist/zk-x509-prover-evidence/iroha-x509-full-credential-2333070-1792829083758049165/`.
Generated artifacts are untracked. Standard output and error were retained only
as digests and byte counts; no private witness output was retained.

The source's final unchanged time assertion explains exit 101; the digest-only
output does not retain its panic text. Bounded observer controls reproduce the
same generic exception during both normal exit and an intervening executable
change. They do not identify the exact guard that failed in this proof, and
there is no waiver for a sampling error merely because the child later exits.
The earlier 2026-10-23 proof and its rejected time, attribution and observer
report remain preserved; this new run does not change that verdict.

The implemented P256 compact-row path has copied-source evidence from ten
correctness controls and 120 exact-output paired component measurements. Median
compact/dense total-time ratios are 0.9780 for certificate work and 0.9717 for wallet work, with
60 pairs per role; every measured total ratio is below one. These modest
component savings do not establish phase weights or a full-proof speedup, and
the changed integrated source still needs a fresh complete invocation. The
exact timing receipt is
`target/qualification/p256-value-compact-row-draft/timing/measurement/result.json`
(`24eb589a5635aa6206151bc9cc2a7c75f0bd6fc2556cae6bd42513cb14e04a2c`).

Full-output FFT evaluation now uses the prefix path only for the public shape
of eight adjacent columns; smaller batches retain dense evaluation. Compile-time
specialization selects the shared recursion for selected-row or full-output
use without changing field arithmetic. The copied candidate passes 40 correctness controls.
Its 24 full-output helper pairs at common log 22 are all faster than the unchanged
dense helper, with median total-time ratios from 0.5901 to 0.8295 across native
logs 5, 8, 15, 16, 18 and 19. Those measurements include actual output-column clearing
but exclude outer dispatch and input construction. The 48 selected-row pairs
preserve exact output and show mixed order effects; they do not establish
performance equivalence. These component results do not establish phase savings
or a full-proof speedup, and integrated qualification remains required.

The selected-row and full-output receipts are
`target/qualification/full-output-prefix-static-draft/validation/measurement-selected/result.json`
and
`target/qualification/full-output-prefix-static-draft/validation/measurement-full-width8/result.json`.

The borrowed SHA row emitter avoids widening every private word row when only a
selected column range is requested. The copied source passes 22 focused controls
and the exhaustive before/after-binding replay across all 89 columns, four
segments and disclosure counts zero through four. On the i7-11800H, 224 balanced
pairs across 56 cases preserve exact output: 211 pairs are faster and 13 slower;
all case medians are faster. The median candidate/baseline time ratio is 0.9690,
and the ratio of summed times is 0.9706. Measurements include source construction,
optional binding, selected fill and local source destruction with eight workers;
output allocation, comparison and final output-owner clearing are outside the
reported interval. This component result does not establish full-proof savings
or named-Xeon performance. The timing receipt is
`target/qualification/sha-base-borrowed-timing-recovery-plan/compressed-tmpfs-registry-runtime/actual-sha-timing-r1/result.json`
(`61339bfecfbb86ad92901c53c82be85c9b783aed146d8d89d64d397b1d617f11`).
All 59 ordinary SHA call-bus tests pass on that same binary: the focused 22 and
a separate disjoint 37-test run. The latter receipt is
`target/qualification/sha-borrowed-remaining-runtime-candidate/actual-ordinary-remaining-r1/result.json`
(`18bd6c1dc590f4f4fa6aca8827b0c2a5155c1401fa3a5c1a612ef1e0be42e4fa`).
The final integrated source still needs matching downstream qualification; the
recorded strict production/test lint failures remain open.

A copied Linux native process-observer candidate passes twenty unit controls,
strict Clippy, 28 pure controls and 33 native lifecycle controls. Its separately
recorded full harness passes all 39 finite controls, including direct short-lived exits,
independent report replay, fork rejection, pipe failures and original deadlines.
The normal publisher's Rust component validation records 109 controls, two
documentation tests, strict Clippy and the codec guard, with a genuine native
bridge artifact and canonical provenance.

The corrected recorded Linux observer source now passes normal local publication
and the unchanged complete 25-method suite: six public delivery controls and 19
native observer methods. This includes timeout handling, real address-space limits,
limit/emitter agreement and cleanup after a report-write failure. All original
processes complete and are reaped; final checks join the recorded source, tools,
streams and 480 retained files. The canonical source before and after is
`58bbeb7bff5887aa347d4eae5800742a080b7f3cea2c4ebbaaf4c29222034c0e`.

Publication is recorded in
`target/qualification/linux-root-observer-native-candidate/final-integrated-engineering-plan/registry-followon-publication-pinned/actual-publish-registry-publish-r1/receipt.json`
(`0e3bf8654ad96aa90012689f39de8b903c5fae6695b0036b5a6d1be77a29eb7d`).
The complete suite is recorded in
`target/qualification/linux-root-observer-native-candidate/registry-followon-runtime-pinned/full25/actual-registry-public25-r1/receipt.json`
(`36b089511284a1672a7d33e9ce8a6e57dbdb55a69ef6855446b15977cc2c75ba`).
It passes all 25 methods in 3,909.300 seconds. Independent root and owner audits
accept the recorded artifacts and unchanged inputs. These are engineering-source
results; later source changes and installed releases require their own qualification.

The earlier 25-, nine- and three-method attempts remain rejected. They exposed a
sample after its deadline, an unrelated ambient exception forwarded to a retained
source reader, a summary-parser defect and a Git inspection that refreshed cached
index metadata. The corrections retain the original deadline, forward only the
reader operation's exception, parse interleaved output and use read-only Git
inspection. The preserved failed receipts are not relabelled by the new pass.
The independent 39-case path retains its separate recorded scope. These finite
checks do not repair the failed maximum-proof observation or prove a full invocation
within the required time.
The remaining outcomes are a complete invocation within 300 seconds and reliable
process observation, with byte, RSS and address-space limits unchanged. This
i7/WSL2 execution does not qualify a named Xeon baseline, native Windows or macOS
execution, installed releases, or four-validator delivery.
