# FASTPQ masked construction: September 28 native validation

This record covers normal-library checks, focused kernels, a complete fixed-SMT
masked child and complete one-child ordinary/AXT public artifacts. The sections
below record their timing, memory and reused-verifier observations. They do not
qualify multiple-child generation, deployment latency, source-state admission,
or the complete hiding and soundness reduction.

## Source scope

The validation snapshot covers 202 Rust, manifest and accelerator source files
under `fastpq_prover` and `fastpq_isi`, plus the root manifest, lockfile and
toolchain file. These inputs were unchanged at native test completion. This is
not a digest of the complete dependency source closure.

| Artifact | SHA-256 |
| --- | --- |
| Scoped source manifest (`/tmp/fastpq-source-20260928-native-ready.json`) | `24d7202b7fc401cb2fcf9e374fe737c6d45dd65bc63f94f146f5f1a7e106d523` |
| Normal-library check log | `28b128f68768afc93f0ae3df32940556644663498c3e72c56dcd64419b76cb99` |
| Focused native test log | `a9908eb69bc704a77fe74f9883d7181dd2abab7d3cfece36aae5a818ba6a82f4` |
| Fresh feature-enabled test binary | `c1be03fa1b38ebccde3a3ff9c3c053c8179a2979f6c8cb8822e22174f9cb2316` |
| Explicit full-domain transform log | `ebebb791d7385554cf51c85265821e8dd6e47971c697e8d16f398f54eeb7aadb` |

The local binary is `target/debug/deps/fastpq_prover-b4846f62af13793e`.
Temporary logs are execution receipts, not inputs required by the repository.

## Checks and results

```sh
cargo check --locked -p fastpq_prover --lib
cargo test --locked -p fastpq_prover --features fastpq-gpu --lib -- \
  deep_ quotient_pair_masking compact_prover_resources \
  offline_compact::resources digest384_batch private_frame fixed_slice_body_encoding
```

The normal library passes without Rust warnings. Cargo reports 8m22s for its
check and 9m47s for the feature-enabled test build. The focused suite passes
128 tests, with zero failures and four explicitly ignored diagnostics; runtime
is 8.54s. It covers masked trace/coefficient replay, strict quotient division,
composition, FRI folds and frontiers, bounded decode, required-device early
failure, resource planning, private framing and actual returned-digest erasure
on success, malformed result shape and partial-use unwinding.

The same binary separately passes these two actual Metal tests in 3.94s:

```sh
target/debug/deps/fastpq_prover-b4846f62af13793e --ignored \
  bounded_masked_leaf_metal_matches_cpu_for_every_oracle \
  continuation_preflight_completes_on_metal --nocapture --test-threads=1
```

They require Metal execution; required-device failure cannot substitute CPU.
The leaf test compares row, Q0/Q1/R, first and last FRI, and terminal leaves
against the CPU owner. The build reports the offline Metal toolchain unavailable
and uses the existing runtime source compilation path. This proves neither
device-buffer erasure nor whole-prover hardware acceleration.

The full 8,388,608-point four-lane FFT/inverse oracle also passes separately:

```sh
/usr/bin/time -l target/debug/deps/fastpq_prover-b4846f62af13793e \
  backend::polynomial_transform::tests::explicit_deep_transform_full_four_lane_fft_matches_coefficient_oracle \
  --exact --ignored --nocapture
```

The test checks independently evaluated low/high nonzero coefficients at 132
selected points, then recovers every coefficient and all high zero padding.
On this unoptimized binary it takes 68.99s in the test harness, 69.00s wall time,
103.91s user time and 0.66s system time. The macOS time tool reports 553,549,824
bytes maximum RSS, 545,309,656 bytes peak memory footprint and zero swaps.
Concurrent repository builds were active; this is a scoped correctness/resource
observation, not a dedicated hardware benchmark or whole-producer measurement.

The fixed-SMT whole-attempt planning test reports 1,065,090,768 payload bytes,
3,468,335,009,584 structural work units and 69,362,447 hash calls. Exact-bound
admission and one-unit-short rejection pass. Prepared ordinary and AXT quantity
relations also pass planning under the unchanged 2 GiB, 2^42 work and
524,288-byte child defaults. These are checked charges, not measured RSS or
elapsed time for a proof.

The Python geometry, hiding and source-budget selection passes 37 tests:

```sh
python3 -m pytest \
  scripts/fastpq/tests/test_deep_hiding_candidate.py \
  scripts/fastpq/tests/test_compact_source_budget.py \
  scripts/fastpq/tests/test_compact_geometry_screen.py -q
```

Scoped formatting/diff checks and `scripts/check_no_legacy_codec.sh` pass.
The full 8M-row producer diagnostic was not run by this focused suite. Kernel
results and a preflight plan do not replace its complete valid/invalid proof,
resource and cryptographic qualification obligations.

## External API and performance follow-up

The external `offline_compact` suite initially passes nine tests and fails one
stale fixture: the supposed decoder-negative fixture selects an opaque
`authorization` claim, so the canonical ingress correctly rejects its semantics
before decoding. The repaired fixture validates a `tx_predicate` binding before
requiring the precise canonical-artifact decode error; a separate assertion
preserves the earlier opaque-semantic rejection. No verifier policy changed.
The rebuilt suite passes ten tests, zero failures and one explicitly ignored
fresh-artifact capture test in 0.02s.

The proof diagnostic now has CPU and required-Metal entry points sharing the
same complete valid-witness, exact-cap, changed-context and tamper checks.
Both use the unchanged default resource limits before materializing private
columns; required-device readiness comes first. This addition does not claim
that either complete proof attempt has finished.

A bounded required-Metal sample times 1024 leaves and 4096 serial CPU parents
per oracle. Under the default test profile, parents take approximately
1.24–1.27s per 4096 nodes. Rebuilding with
`--config 'profile.test.package.fastpq_prover.opt-level=2'` gives approximately
1.22s per 4096 parents; `fastpq_isi` already uses test optimization level two.
The extrapolated parent work alone is about 10,300s across the two commitment
traversals. GPU leaf timings vary substantially under concurrent build load.
These samples identify a likely serial bottleneck; they are not complete-proof
measurements or release-hardware feasibility results.

The optimized 32/256/1024-job comparison passes in 41.69s, with 37,388,288 bytes
maximum RSS and zero swaps. Each cell below measures exactly 4096 actual Metal
jobs after one warm batch and gives preparation / executor seconds. Preparation
includes parallel canonical framing and, in this measured snapshot, sequential
continuation-job construction. Executor time includes host packing, device work,
readback and internal clearing; it is not isolated GPU kernel time.

| Canonical frame | Batch 32 | Batch 256 | Batch 1024 |
| --- | --- | --- | --- |
| Row leaf | 0.489 / 11.578 | 0.441 / 3.050 | 0.437 / 1.120 |
| Q0/Q1/R leaf | 0.475 / 3.635 | 0.441 / 1.175 | 0.434 / 0.172 |
| First FRI leaf | 0.482 / 5.846 | 0.440 / 1.160 | 0.432 / 0.177 |
| Row-tree parent | 0.478 / 3.721 | 0.442 / 1.174 | 0.436 / 0.172 |

Returned-owner and frame cleanup take less than 0.005s per cell. At batch 1024,
the executor payload charges are respectively 2,899,968, 524,288, 966,656 and
540,672 bytes; these exclude caller-owned frames/jobs, which the producer's
separate shared plan charges. First and last digests in every batch match the
independent CPU hash, and required-device execution cannot substitute CPU.
The log SHA-256 is
`79ae0b47238deccfee49b15f2bcf1b03129f55b843ff8c28fac97690d45c653c`.
The scoped source manifest for this comparison is
`b5995078c0d28c4423bfe0cfa8e09c28fd149603f822e226d088d8366120d41c`.
These measurements justify the subsequent 1024-job bound and parallel job
preparation; they do not validate those later edits or a complete proof attempt.

A later fresh build includes `ExpectedStatement::from_statement` and its
external API mutation/canonical-framing regression. The suite then passes
11 tests, zero failures and one explicitly ignored capture test in 0.02s.
Its log SHA-256 is
`21eafc5be29dd0c0451163eebd1f06d1f3df5390a53a2c810249154ddcb12e19`;
the external test binary SHA-256 is
`1d1d7b57f8af183b16483de8d9dd333d340e58b7168914f9a0b3e21e247b3e22`.
This evidence precedes the parent-batching and parallel-job edits.

## Bounded parent batching and parallel job preparation

The subsequent source snapshot fixes the shared capacity at 1024, prepares
continuation jobs in parallel with ordered errors, and uses one clearing
returned-digest owner for leaf and parent device batches. Independent lower
Merkle parents are batched across rows; the upper tree stack keeps its original
row order. Shared payload planning includes both index arrays, canonical frame
owners, intermediate result/job descriptors and executor buffers.

The fresh optimized build passes 134 focused tests in 12.63s, with zero failures
and nine explicitly ignored diagnostics. Tests compare roots, every small
frontier, canonical parent coordinates and 1024-row run boundaries against the
independent serial/full-tree reference. Malformed runs, partial callback writes,
noncanonical digests and upper-stack failure poison the traversal. Parallel job
construction is checked against independent canonical streams with one and four
workers. The external API passes 11 tests and the new public usage doctest passes.

Three separately selected required-Metal tests pass in 4.82s: all fixed leaf
oracles, parent batches and continuation preflight. Their first two tests compare
actual device results with canonical CPU hashes, including full 1024-job batches.
Maximum RSS for this scoped parity run is 34,422,784 bytes. At this checkpoint,
the complete producer diagnostic remains outstanding. Whole-attempt preflight reports
1,065,090,768 payload bytes, 3,468,335,009,584 work units and 69,362,447 hashes;
the larger hash buffers do not exceed the already-dominant phase charge.

| Current batching receipt | SHA-256 |
| --- | --- |
| 203-file source manifest | `46b4367b90c080606f0bc84efb355bcac0da0aed80ffa39f7897029c41bb5f63` |
| Fresh feature-enabled test binary | `f47eaa89d011883cfec5cc2a28e814718a3d900e86878333359384e8bcfb9fec` |
| 134-test focused native log | `7260ad2ac4722d8251ad5d1ca3a401af66185c46195a8d410b69c874815216ab` |
| Three-test required-Metal parity log | `30b99017542d3ca72ecf655c89849771e2cdbde015ddf8427439435f6d6f395e` |
| External API log | `e4167d9435363c07ed4aabe3ab1a8cd008b2abe1cdc2ebfc5311f202aaf979d4` |
| Public usage doctest log | `e80ac837c07ce6cba1d01f7193f2e30a040e5cadb19a0243b447df2d43da1f32` |

The source scope again excludes the complete dependency closure. No scoped source
changed between the snapshot, build and native/parity checks.

The same binary then passes both actual-Metal timing diagnostics in 10.63s
(132,988,928 bytes maximum RSS). With parallel job preparation, 4096 row hashes
at the production batch size of 1024 take 0.053125s preparation, 0.108573s in the
executor and 0.003888s cleanup. Corresponding parent timings are 0.045909s,
0.011772s and 0.000477s. The earlier executor measurements were substantially
slower under concurrent load, so this comparison does not isolate a GPU speedup
caused by the code change.

The separate raw typed diagnostic keeps the production bound unchanged and
measures 8192 samples at each larger size. At batch 4096, row preparation /
executor times are 0.089418 / 0.122298s; at batch 8192 they are 0.086058 /
0.135849s. Parents take 0.080854 / 0.012850s and 0.074939 / 0.011430s respectively.
Exact executor charges for row frames are 11,485,184 and 22,921,216 bytes; these
are not complete producer charges. Both retain independent CPU parity and
required-device error handling. Production remains at the validated 1024-job
bound for the full producer measurement. Timing-log SHA-256:
`29c1913d5186aac535507ada109f634666ee024db83d6f7d16ed07a6ee058214`.

## Complete required-Metal fixed-SMT child

The same optimized binary subsequently passes the complete masked producer and
its independent verifier controls:

```sh
/usr/bin/time -l target/debug/deps/fastpq_prover-62f97469cbc9ba80 \
  backend::deep_prover::tests::complete_required_metal_masked_deep_producer_roundtrip_and_statement_rejection \
  --exact --ignored --nocapture --test-threads=1
```

The binary was built with
`--config 'profile.test.package.fastpq_prover.opt-level=2' --features fastpq-gpu`.
The M1 Ultra host has 20 logical CPUs and 128 GiB RAM. Concurrent Rust builds
and memory compression were active throughout much of the attempt. This is a
contended correctness/resource measurement, not an isolated release benchmark.
The fixed batch size remains 1024. FFT/replay arithmetic uses CPU; actual Metal
executes bulk leaves and lower parents, while ordered upper parents and transcript
hashes use CPU. A required-device error cannot select a CPU substitute.

| Complete child measurement | Result |
| --- | --- |
| Valid canonical proof bytes | 482,978 |
| Build plus internal independent self-check | 4,344.265 s |
| Full test wall / user / system time | 4,346.20 / 10,789.91 / 842.93 s |
| Maximum resident set size | 1,118,158,848 bytes |
| Peak memory footprint | 1,104,381,536 bytes |
| Swaps | 0 |
| Test result | 1 passed, 0 failed |

The attempt is admitted under unchanged 2 GiB charged-payload, 2^42 work and
524,288-byte child limits. Its context-specific plan charges 1,065,090,838 payload
bytes, 3,468,335,009,584 work units and 69,362,447 hashes. The context adds 70 bytes
to the separate planning fixture above. The successful result includes a second
independent verification, a one-byte-short proof cap, a different statement
context and a modified proof rejection. The printed `iroha_crypto::Hash` of the
proof is `7d16efc5143e19aa9fe7d1c9d37605741fb338953ff282f91b3ab72af5509507`;
it is not a SHA-256 digest. This diagnostic does not retain the raw proof bytes.

The binary SHA-256 remains
`f47eaa89d011883cfec5cc2a28e814718a3d900e86878333359384e8bcfb9fec`.
The full run log SHA-256 is
`3cb5f9b4511c904326cc817cb2f0bccf10025fc37e0447f881af9991e7b89e3e`;
the during-run host observation SHA-256 is
`4619e91a47228996904ec9965c257f2eabc34b62696006dd93df421921f7e2cd`.

The binary corresponds to the 203-file snapshot above. At completion, three
workspace inputs differed because of concurrent committed work: the root
manifest enabled `sorafs_manifest/pqc`, the lockfile removed `x25519-dalek` from
`soranet-handshake-harness`, and the digest prefix source changed an opaque type's
visibility plus a test import position. No FASTPQ arithmetic/hash body changed
in that scoped comparison. This is evidence for the recorded binary/snapshot,
not a claim that the current workspace is byte-identical to it. Subsequent
accelerator cleanup and public-facade tests require their own build receipt.

This fixture proves the fixed two-update SMT relation, not the ordinary or AXT
quantity wrapper. Those wrappers have distinct bound relation identities and
complete statement contexts; rewrapping this child cannot produce a valid
positive artifact. Separate one-child public-facade generators retain their
artifacts for repeated verifier controls. Real multiple-child ordering/root-chain
qualification, authenticated source admission, full-transcript privacy/soundness
review and release-hardware performance remain separate requirements.

## Accelerator ownership and captured public-facade build

A subsequent source snapshot includes clearing Metal/CUDA host staging,
transactional failure cleanup, and bounded Metal ticket draining. Unknown
completion is sticky for the process: private device allocations remain owned,
and another memory-limited proof cannot discharge them by selecting CPU.
Ordinary completed errors still permit deterministic fallback. This snapshot
also includes the independent one-child ordinary/AXT generator and reusable
verifier controls.

The optimized feature build finishes in 16m21s including artifact-lock wait.
All 208 files in its scoped manifest match at binary capture. The scope covers
FASTPQ/ISI sources, root manifests/lock/toolchain and the CUDA host-cleanup
harness; it is not the complete dependency source closure. Both binaries are
copied outside the Cargo target before further builds.

| Captured final-lifetime receipt | SHA-256 |
| --- | --- |
| Scoped source manifest | `587dfcc16b3034475c25d9bdd8c84b82a54b23266e7a8db6e6a963d5040f7a90` |
| Immutable unit-test binary | `84b0efaa031647b2724d907614cf55192090d2045bafbcd2d2fb98b859eef111` |
| Immutable external API binary | `51dae293ffe247dc6da47eeeba6ed9dbdd6dd02ecdc03bcfee20e911734ff933` |
| Build log | `4855891a0405d5b8f7587c710e6e3482b37ffeb6c8fc2747c10a7897926194e7` |
| Focused native log | `5379fb2d9bc9c2641fb3ad511fd5c4b6e8d720284813ed9892a07074f18e86f1` |
| External API log | `9b89b2d8accee709788db733a8d53f7404756d0b68fb3d6b5cd988c52e5e1a6f` |
| Required-Metal hash parity log | `ac5255b32d02e2ce30b8788ff1267d05fce731f072e5df84c9a02013f55d42ff` |
| Existing transform/Poseidon parity log | `33a7a9b9a2fedb9974178c31e61d034c5161eabd494fba50991f82dd18099226` |

The focused selection passes 177 tests in 45.32s with no failures and 13 ignored
diagnostics. It includes the five actual-source ticket-budget controls, actual
abandoned Metal command draining, real-cell clearing on successful, failed and
unwinding paths, and terminal uncertain-completion mapping. The normal external
API passes 12 tests in 0.54s, with four ignored complete/captured-proof tests.
Existing Metal FFT/IFFT/LDE/Poseidon parity and failure controls pass 13 tests in
4.62s. Three separately required-Metal leaf/parent/continuation tests pass in
1.75s, with 31,899,648 bytes maximum RSS. These two short device selections
overlap each other; their timings are not isolated kernel measurements.

At 13:57:21 JST, two separate processes start the actual one-child ordinary and
AXT public producers from the immutable external binary. They require Metal,
use unchanged default limits and retain SHA-addressed canonical artifacts
before running the same-artifact verifier controls. Both overlap repository
compilation and later isolated diagnostics. Both complete successfully as recorded
below; the timing observations do not establish deployment latency.

### Completed one-child AXT public artifact

The immutable external API binary above completes the actual
`single::required_metal_axt_producer_and_reused_verifier_controls` test with one
pass and no failures. It retains the canonical artifact before running the
independent same-artifact controls. The producer requires Metal and uses the
unchanged default payload, work, child and carrier limits.

| AXT public facade measurement | Result |
| --- | --- |
| Canonical artifact | 484,750 bytes |
| Inner child proof | 481,729 bytes |
| Proving including self-verification | 2,045.209 s |
| Reused verifier controls | 3.506 s |
| Process wall time | 2,049.67 s |
| Maximum resident set size | 1,109,753,856 bytes |
| Peak memory footprint | 1,097,008,688 bytes |
| Swaps | 0 |

The accepted work reports one transcript, one AIR evaluation, one terminal
degree check, 64 row leaves, 64 oracle leaves, 304 FRI leaves and 4,278 parent
hashes. Tests accept exact inclusive limits and reject each tested one-unit
deficit, exhausted decode allowance, inherited strict decode scope, incorrect
independent statement fields, malformed/truncated bytes, ordinary/AXT schema
confusion and wrong AXT mirror/remote-spend expectations. Coherent changes to
both the advertised and independently expected public statement, corridor and
expiry context still reject at the DEEP mathematical binding check.

The retained artifact is
`target/fastpq-production-validation/quantity-public-producer-deep-axt-one-800d52cf07f9c4c7e72cc176e0ebd6ffdec0f4bccebbeeb1cb50bbd348e4f4a3.bin`.
Its SHA-256 is
`800d52cf07f9c4c7e72cc176e0ebd6ffdec0f4bccebbeeb1cb50bbd348e4f4a3`;
the complete process log SHA-256 is
`ab0bd08321393d480868a10267993d63736861380b75a24ba29c25234993e2e2`.
The captured binary/source scope is the final-lifetime receipt above. The other
proof process and builds run concurrently; this is a contended observation,
not deployment-latency qualification. Multiple-child chaining, authenticated
ledger/source admission and whole-transcript privacy/soundness remain separate.

Copies of the immutable final-lifetime and staged-FFT binaries, both public
artifacts, scoped source manifests, host observations and logs are retained under
ignored `dist/zk-remediation/2026-09-28/fastpq/`. Its separate `sha256-index.json`
records each original path, length and SHA-256; `SHA256SUMS` permits verification
after restart. These receipts contain no private witness dump. The original
paths remain in place; both completed producer logs and artifact replay logs
are indexed after completion.

### Completed one-child ordinary public artifact and saved-artifact replay

The same immutable external API binary completes
`single::required_metal_ordinary_producer_and_reused_verifier_controls` with one
pass and no failures under unchanged defaults and required Metal. The independent
same-artifact controls described above also pass for the ordinary schema.

| Ordinary public facade measurement | Result |
| --- | --- |
| Canonical artifact | 485,600 bytes |
| Inner child proof | 483,777 bytes |
| Proving including self-verification | 5,247.025 s |
| Reused verifier controls | 2.405 s |
| Process wall time | 5,250.63 s |
| Maximum resident set size | 1,110,228,992 bytes |
| Peak memory footprint | 1,097,729,584 bytes |
| Swaps | 0 |

Accepted work is one transcript, one AIR evaluation, one terminal degree check,
64 row leaves, 64 oracle leaves, 303 FRI leaves and 4,319 parent hashes. The
retained artifact is
`target/fastpq-production-validation/quantity-public-producer-deep-ordinary-one-99ba1411640b0c287bc6d4af4e14b8aa2b86509828a7d834cc96e0468b352c6c.bin`;
its SHA-256 is
`99ba1411640b0c287bc6d4af4e14b8aa2b86509828a7d834cc96e0468b352c6c`.
The complete log SHA-256 is
`73f6ae48d18715f6359d1a84c393a9e096bd43de2b29c9382a7f4ad153906105`.

The separately invoked
`single::captured_single_artifacts_verify_without_reproving` reads both
SHA-addressed bounded files, constructs expectations from the independent
one-child fixture and repeats the acceptance/mutation/resource/schema controls
without generating another proof. It passes on the immutable final-lifetime API
binary in 5.94s with 28,131,328 bytes maximum RSS, and on the later staged-FFT
normal API binary in 5.92s with 27,181,056 bytes maximum RSS. The replay logs have
SHA-256 `a38bde58656253a4565b7164256b67d6052db75c39a69c11f82fb1dc7956572f`
and `6ebe2fe19c7633dd88f2b0c69193e425828dd4af275417312d3fb710e94650e0`,
respectively. These results establish the captured one-child facade and verifier
paths; multiple-child chaining, authenticated ledger admission, whole-transcript
privacy/soundness and deployment latency remain separate requirements.

## Staged exact-root Metal FFT

The bounded exact-root arithmetic API now dispatches independent bit-reversal
pairs, one group per 256-word local tile, and one group per 2048 global
butterflies. Explicit resource barriers separate dependent kernels inside one
bounded command. The prior dispatcher assigned only one group per column and
iterated over that whole column. This change affects the exact-root API; the
captured ordinary/AXT proof processes above retain their original binary and CPU
FFT path.

The new dispatcher keeps one clearing shared copy and publishes caller columns
only after successful completion. It reuses the existing completion ticket,
permit, unknown-completion quarantine and private backing owner. The existing
additional-payload allowance remains conservative; no prover budget, proof cap,
root convention, masking rule or wire layout changes.

The fresh native selection passes 32 tests in 2.07s with zero failures and one
ignored full-shape diagnostic. It covers dense forward/inverse equality for
primitive root powers 1/3/5 across local-tile and global-group boundaries,
independent butterfly coverage/twiddle checks, actual clearing on success,
failure and unwind, unchanged original input after failure, the previous Metal
transform/Poseidon controls, ticket deadlines and quarantine. An initial cleanup
fixture took a weak reference before its required exclusive `Arc::get_mut`
transition; moving that observation after Metal publication repairs the test
without changing production code. The normal-library external API passes 12
tests in 0.79s. Required-Metal leaf/parent/continuation parity passes three tests
in 1.99s. The Python geometry/hiding/resource selection passes 37 tests in 2.06s.

The actual Rust API's separately selected full-shape test passes with eight
columns, native rows `2^19`, common rows `2^22`, and all 1816 mask coefficients.
Every output matches the CPU owner, with independent Horner checks and
non-default primitive-root controls. Timings include validation, staging,
dispatch, wait, publication and clearing; they exclude source construction,
coset packing, row hashing, constraints and the complete proof.

| Transform | CPU | Metal |
| --- | --- | --- |
| Eight-column native IFFT | 98.599 ms | 41.789 ms |
| Eight-column common FFT, one batch of eight | 690.993 ms | 119.840 ms |
| Same FFT, batches of four | same reference | 102.466 ms |
| Same FFT, batches of two | same reference | 130.368 ms |

The whole diagnostic takes 2.92s wall time, with 1,365,000,192 bytes maximum RSS
and zero swaps. Its retained reference, source and output arrays are diagnostic
storage, not a measurement of a complete prover's memory. Two full public proof
processes and Rust builds run concurrently; these are contended measurements.
The kernel/API result does not qualify complete X509 proving or authorize a
change to its resource admission.

| Staged FFT receipt | SHA-256 |
| --- | --- |
| Original 211-file source manifest | `f46529fac659dae1dae7b27d848ffb297f65e35f915b313ac8c069fa46b68db2` |
| Timed native binary | `0b3af1fae8e97f2ca4623af433759c167f9a64fb48d935bad7a15651f964832b` |
| Full-shape timing/parity log | `59c71a43838d1ccf1b72fc82e6bea2b054ffa220f52984c27bbf7dcad07ea4b0` |
| Source manifest after cleanup-fixture correction | `dd9804ca91b75e416b141f7331789c9a66ebb9f1bc81763657da5b9f2ba2b554` |
| Corrected test binary | `28f8886c0462dfe0c5ca9ff9bd939990d0c37cea88646301854bbffe73eb0720` |
| 32-test native log | `e9f76d26cd93b6fd558bf86817b17da8917552556448089ad9683578f1cada9f` |
| Required-Metal hash parity log | `3b87da87aefc7d7a2141564af9f9df8980b05b71a42321d5ea4cc8a41b63924c` |
| External API log | `bc2ac96f2aca0cfab37a9e24f5121a8fc37369b1beffd4f0e5aeb755624f551d` |

Only `metal_exact_root_tests.rs` differs between those two source manifests.
All production sources used by the timed binary match the corrected snapshot.
The same source-scope limitation stated above still applies.

## Isolated replay-cost prototypes

At the time of these prototype runs, production opening passes recomputed each
complete commitment to collect queried rows and canonical frontiers. An isolated harness compiles the actual `cyclotomic.rs` arithmetic
alongside two caller-owned public tables: 65,536 stripe powers and 32,768 root
powers, totaling 786,432 bytes. All 128 stripes of 301 columns and 65,536 rows,
with the current 136 mask coefficients, match the original arithmetic cell for
cell, including independent modular Horner controls. The original transforms
and twists total 14.098s; the candidate totals 7.989s plus 0.055s preparing stripe
powers. The whole parity harness takes 25.81s with 480,706,560 bytes maximum RSS
on the contended host. This saves arithmetic work, but does not measure hashing
or a complete proof; these tables have not been adopted.

A second isolated prototype retains internal Merkle nodes while regenerating
queried leaves and leaf-level siblings. It uses the current striped stream and
multiproof source, the native digest in a distinct diagnostic domain, and a
clearing storage shim. Across 52 small tree/stripe geometries and 259 frontiers,
all cached nodes and canonical sibling bytes match independent materialized
trees; serial and parallel reconstruction agree, and altered coordinates or
incomplete/duplicate cache writes fail. It takes 26.97s with 4,521,984 bytes maximum
RSS. This does not exercise the production DEEP context, codec, cache owner or
complete producer.

The seven fixed row/quotient/FRI trees would retain 832,272,048 bytes of internal
nodes. Conservatively retaining all construction coverage bitmaps adds 2,167,376
bytes. Adding both to the previously measured full-attempt payload plan gives
1,899,530,262 bytes, below the unchanged 2 GiB limit. This is a design screen,
not the final per-statement resource plan or a full-size allocation measurement.
An integrated implementation still needs bounded selected-leaf replay, immutable
context/oracle binding, exact lifetime accounting, clearing/error tests,
unchanged root/frontier/proof-byte parity and a new complete producer run.

The sources, binaries, logs and receipts for both prototypes are retained in the
FASTPQ evidence directory and its separate SHA-256 index. The complete-pass
arithmetic receipt has SHA-256
`c0b273d9787c1dd69699b894fbea98d4a91d7e06f97be2f2d2b24ff2e985383b`;
the internal-node prototype receipt has SHA-256
`7c4445fbf73fc90e442ca474cf804e2598dfbad3a79ede45d6a93f6fa734b7a6`.


## Internal-node cache integration awaiting native validation

The subsequent source integrates a per-attempt cache for all internal nodes of
the row, quotient/mask and five FRI trees. A cache retains the original immutable
context identity and first-pass root. After transcript queries are fixed, the
original masked coefficient owners regenerate queried leaves and leaf-level
siblings; complete FRI fibers keep their original ordering and framing. The
canonical multiproof must reconstruct the original root before any opening
leaves its clearing owner. No new entropy is sampled and no wire format changes.

The checked whole-attempt plan sums all seven node arrays and coverage bitmaps
(834,439,424 bytes), cache owner metadata and the largest selected-opening
workspace, in addition to the previous coefficient and active phase charges.
Previous full replay work remains charged conservatively. Duplicate, malformed
and extra writes poison caches/streams, including writes after complete coverage.
Both serial and parallel Merkle authentication frontiers now clear private-derived
hash copies on successful return, errors and unwinding. Source reviews found no
blocking algebra, ownership or accounting issue; they are not native validation.

The final 212-file FASTPQ source manifest is
`/tmp/fastpq-source-20260928-node-cache-final.json`, SHA-256
`0ec0680c6cc4096064000263e9a4afd800d470a3db8e22625d73731c372badb6`.
It covers FASTPQ/ISI and root build configuration, not the entire dependency
closure. The native feature-enabled library and external API build is queued.
New controls cover same-attempt binding, malformed/partial callbacks, observed
live-cell erasure, selected-stripe/fiber parity, full/reference frontier equality
and exact one-unit budget failures. The complete required-Metal seeded child
additionally pins the previously generated 482,978-byte proof hash; this long
proof-byte/entropy equivalence run has not yet executed against the cache code.
