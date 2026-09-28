# FASTPQ masked construction: September 28 native validation

This record validates the merged normal-library construction and focused kernels.
It does not qualify complete proof generation, peak RSS, latency, source-state
admission, or the complete hiding and soundness reduction.

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
Maximum RSS for this scoped parity run is 34,422,784 bytes. The complete producer
diagnostic remains outstanding. Whole-attempt preflight still reports
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
