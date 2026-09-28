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
measurements or release-hardware feasibility results. Bounded 32/256/1024-job
samples separating frame preparation, device-executor wall time and returned
owner cleanup are the next comparison. No live batch size or protocol changed.
