# Maximum X509 proof on Intel under WSL2

The integrated `f5d2b994` maximum-depth-three diagnostic completed on
2026-11-01 UTC and is **rejected**.
It produced and independently verified the complete 9,435,984-byte proof, but
proving took 1,349.733843 seconds against the unchanged 300-second target.
Native lifecycle certification and final source/executable custody pass on this
candidate. The separately requested phase-detail bound also fails: the largest
undivided phase holds 122,991 ppm of proving time against 10,000 ppm. Successful
verification and lifecycle observation do not repair either failed bound.

| Current diagnostic observation | Actual result |
| --- | --- |
| Build | Genuine locked `--release` `iroha_core_privacy` library test; default `zk-stark` and `simd`; privacy opt-level 3 and canonical model opt-level 1 overrides; 16 Rayon workers |
| Proof | 9,435,984 bytes, exactly current encoded geometry; unchanged cap 9,437,184 bytes |
| Proof SHA-256 | `8456fe0de19de4b6f948f74086677ede05cc6d1b893ef6280b73340c7a250af6` |
| Verification | Producer self-check and independent verifier replay pass; replay 4.151497 seconds |
| Negative controls | Wrong genesis, all 32 nonce-byte substitutions and tampered proof reject |
| Time | Proving 1,349.733843 seconds; whole child 1,360.171020488 seconds; both exceed 300 seconds; unchanged time assertion exits 101 |
| RSS observations | `wait4` peak 8,806,334,464 bytes; retained sampled peak 8,807,747,584 bytes; each independently below the RSS bound |
| Requested resource bounds | Explicit harness RSS bound 12,884,901,888 bytes; kernel-confirmed soft and hard `RLIMIT_AS` 34,359,738,368 bytes |
| Phase attribution | 254,750,026 ns unattributed of 1,349,733,846,049 ns; 189 ppm, within 1% |
| Phase detail | Initial base transform exclusive time 166.004378601 seconds, or 122,991 ppm; fails the requested 10,000 ppm bound |
| Native lifecycle | Both finalization reads return status 0; identical certified records retain no first error and zero reported per-CPU loss; original WNOWAIT and `wait4` join exit 101 |
| Invocation verdict | Rejected with `command_exit:101` and `record_rejected:0`; final source and executable checks pass; no accepted public acknowledgement |

The observer uses the independently approved engineering source
`552f2b8476ac57622050660f6fd1cd7adfe29a778a356292176ce2531f6b708c` and its normal
publisher artifact. The separately captured prover source is
`b054dbdf546d8032cf50578ac319a879df94f4919970786bc312017538df85f3`, with executable
SHA-256 `881db84821cfe921a09698165aa6e2ec7e6211759199238b7652b5b417ea32d4`.
This is not a same-source or independently authenticated signed-release result.
No owned build or benchmark ran concurrently; start and finish checks found zero
Cargo/rustc processes. This does not establish an idle host or named-Xeon
baseline, or isolate a speedup from the earlier contended run.

The three coordinator transform regions total 425.116619676 seconds: initial
base transforms take 166.004 seconds, auxiliary transforms 142.853 and query
transforms 116.260. They occupy separate top-level phases, so this sum is valid;
it does not identify full-output, selected or Horner subroute costs. Composition
residues/fold take 87.238 seconds, base replay 84.622 and auxiliary replay 79.812.
The inclusive composition total is 438.352 seconds. Inclusive parent and child
durations overlap and must not be added. Twenty-two non-root regions exceed the
requested undivided-phase bound. The emitter's empty
findings list covers its intrinsic checks; the harness additionally enforces
the requested phase-detail bound. These measurements identify investigation
targets without establishing an optimization's benefit.

A test-only forward FFT power-table reuse trial passes 28 correctness controls
and 96 exact-output records on real P256 auxiliary prefixes. The balanced trial
retains the same reference output for both variants and clears each measured
output before the next; both outputs fit the existing allocation charge.
The ratio of summed evaluation-plus-clearing times is 0.9793, but substantial
variation, including in the one-batch control with no constructor saving, prevents
a reliable causal or whole-proof gain claim. No production change follows.
The exact image is `1f190349d76a85ae3103f4825a9407b2a260be0fdfd1b3e5ef9a700c94c93654`;
receipts and source copies are in untracked
`dist/csharp-public-power-reuse-20261031/attempt2/`. The earlier 72-record attempt
remains archived with its unequal output residency and execution-order limitation.

A separate [joined DIF trial](intel_avx2.md#joined-dif-transform)
executes the actual full-output common22 geometry and reports a 0.917399 ratio
of summed work-plus-cleanup time across 96 balanced records. All twelve case
sums improve, while eight of 48 individual pairs regress. Its component
correctness, owned allocation and native-instruction checks pass; equal shape
weighting does not represent the maximum workload. A subsequent production
integration shares the native field owner with the existing full-batch CPU
transform and preserves its scalar fallback. Its default and feature-off controls
pass. The current maximum includes that integration and the unit-output correction
that removes unobserved counter-return transport. This single run provides no
controlled before/after speedup or observer-overhead estimate and does not qualify
the 300-second target.

The earlier rejected maximum-proof test created public outputs inside the
retained `source/dist` tree, changing a retained parent directory's timestamps.
The final source keeper reported `source original or selected name changed`.
Retained-source diagnostics now require test-only `IROHA_X509_PUBLIC_OUTPUT_ROOT`
to select an existing absolute directory outside the source and harness
child-record directory.
Ordinary/max proof, holder and public-candidate replay diagnostics share this
check; production prover and node behavior are unchanged. Both output-root
controls pass on this genuine release image, and the complete maximum invocation
retains source custody. The earlier native finish/reap refusal remains
unexplained; this successful lifecycle does not establish its cause.
The earlier sampled RSS exceeded `wait4` RSS. A separate 128 MiB native workload
reproduced this ordering in two of three runs, and the matching published kernel
source confirms that page-table sampling and batched high-water accounting
differ. The harness follow-up preserves both exact values and applies the bound
to each independently, without imposing an unsupported ordering or adding a
tolerance. See [the resource contract](linux_resource_harness.md). The small
probe and source references are in untracked `dist/linux-rss-accounting-20261031/`;
its inventory SHA-256 is
`d4cf98aae2bda9c795b32b9498804ef4ffb1b08a5b396962d51e2a0d2df3f7e5`.
This repair does not alter the original rejected maximum-proof evidence.
The tightened harness passes all 20 native controls and six public delivery
controls with the reviewed Python hashes unchanged. This maximum additionally
passes actual native finalization under the unchanged memory and address-space
bounds; time and phase detail remain failed.

The initial base commitment now delegates its original diagnostic parent to seven
test-only operations: stream setup, column admission, row absorption, finalization
preparation, leaf finalization, Merkle reduction and root binding. Their coordinator
windows include the existing worker joins. Repeated batches and tiles aggregate
under fixed labels; the 75 flat X509 counters retain their indices. Auxiliary and
query commitments receive inactive contexts. At the current maximum geometry,
404 batches and 1,024 tiles produce exactly 2,859 completed guard lifetimes across
seven additional tree nodes in the actual run. None is interrupted, unwound or
truncated. All 149 phase nodes complete and recorder-health counters remain zero.

| Base commitment operation | Actual calls | Exclusive seconds |
| --- | ---: | ---: |
| Stream setup | 1 | 0.488367 |
| Column admission | 404 | 7.374739 |
| Row absorption | 404 | 60.717832 |
| Finalization preparation | 1 | 0.000065 |
| Leaf finalization | 1,024 | 0.727805 |
| Merkle reduction | 1,024 | 5.173172 |
| Root binding | 1 | 0.000004 |

These seven sibling operations account for 74.481982902 seconds. The remaining
base-parent exclusive time is 12.876587332 seconds, or 9,541 ppm; it includes
work outside the hooks and is not an isolated clearing measurement. Row absorption
remains an undivided 44,986 ppm region and includes the original parallel packing,
hash updates and accounting, not only SHA3 permutations. The measured coordinator
windows include their synchronous worker joins. `active_threads_max = 1` records
the guard-opening thread and does not mean that proving was single-threaded.

The preceding hook candidate passed 132 focused privacy controls, including all 16 new
context/commitment controls and the now 37-control X509 observation suite, plus
five measurement-layering checks. A separate non-test library builds without a
measurement dependency or observer symbols. The tests preserve roots, retained
cuts, failure outcomes and clearing, and exercise actual worker return ordering.
The broader run also corrects an obsolete test pin to the existing 11,662
diagnostic transforms; the untouched earlier image reproduces that stale-pin
failure. No proof layout or resource limit changes. Both earlier failed attempts
remain preserved beside the passing receipts in untracked
`dist/base-commitment-phase-controls-20261031/`; the final test image is
`18711ab957867095798f81352652b132b43558273bd914028fc761831e6b05bf`.
Its 710 selected owner/configuration inputs are not complete compiler-input
custody. The current maximum additionally exercises the hooks with the integrated
DIF and unit-output changes. Actual transform subroute attribution, whole-proof
overhead, phase detail and the 300-second target remain open.
Worker node wall time sums overlapping calls; worker wrappers alone cannot
satisfy the undivided-phase bound.

Current proof, receipts, 10,193 retained samples, phase records, original native
bytes and build/source records are preserved in untracked
`dist/maximum-x509-intel-20261031-integrated-rejected/`. Its 735-file inventory has
SHA-256 `523159d0b3ae7eac60d7a87b63080a216cf6a09e22699424e0fb41b34c54ed41`
and includes the separate independent reviews. The original
rejected public manifest is
`24e541c055d1f500edbd5f42c45cd3efc15832667e6425936dfedd35d33a072f`.
The preceding 1,364.449936-second diagnostic remains intact in
`dist/maximum-x509-intel-20261031-current-rejected/`, with its original 719-file
inventory `f712c16c9601bc4d3b34f2061a79bdf78876e68a2edbf320bc6687848549065b`.
The earlier 2,005.107135-second proof and its native/source refusals remain intact
in `dist/maximum-x509-intel-20261031-refused/`, whose postmortem SHA-256 is
`7bd94f141ff325291f4efa686a3f146ed04f151b0a3815a3e7cccf5c505d2e24`.
Archival digests are not accepted invocation pins. No private witness or child
output tail was retained. The 7,200-second diagnostic timeout did not replace
either unchanged 300-second target.

The following records describe earlier source scopes and proof geometry.

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
The current candidate still needs a complete invocation within 300 seconds and
the requested phase detail, with byte, RSS and address-space limits unchanged.
Its successful native lifecycle observation is scoped to the current result
above. This i7/WSL2 execution does not qualify a named Xeon baseline, native
Windows or macOS execution, installed releases, or four-validator delivery.
