# Vendored MSM/FFT dispatch checks

`tests/halo2_native_dispatch.rs` exercises the vendored `halo2curves` dependency
already used as this crate's differential oracle. It checks the direct native
iteration and join operations that replace `plonky2_maybe_rayon`, without adding
an oracle dependency to the workspace graph.

Run the serial and Rayon variants separately from the workspace root:

```sh
cargo iroha-fast -- test --locked -p iroha_pasta --test halo2_native_dispatch
cargo iroha-fast -- test --locked -p iroha_pasta --features halo2curves/std --test halo2_native_dispatch
```

The tests compare MSM against an algebraic reference at both sides of the
scheduled-window threshold, check finite and identity bases, compare FFT against
direct polynomial evaluation, and check inverse transforms and deterministic
results with one, two and four Rayon workers. The large mixed-identity case
reproduces the former absent-coordinate unwrap panic. Identity inputs now use
the existing complete group-arithmetic kernels.

The opt-in timing harness reports seven-batch medians and individual samples;
MSM batches contain three operations. Input construction and FFT input cloning
are outside measurement. It uses a four-worker pool; without `halo2curves/std`,
the vendored kernels still execute sequentially.

```sh
cargo iroha-fast -- test --locked -p iroha_pasta --test halo2_native_dispatch benchmark_native_dispatch -- --ignored --nocapture
cargo iroha-fast -- test --locked -p iroha_pasta --features halo2curves/std --test halo2_native_dispatch benchmark_native_dispatch -- --ignored --nocapture
```

For a before/after comparison, compile the original and current `msm.rs` and
`fft.rs` as separate modules in one external optimized harness. Reuse exactly
the same cached field, curve, `ff`, `group`, `libm` and `num_traits` artifacts for
both modules. Supply the removed adapter only to that external harness via
`rustc --extern`; do not restore it to any workspace manifest. For a parallel
comparison, enable the original adapter's `parallel` feature and the modules'
`std` feature against the same Rayon artifact. Assert identical results before
timing, alternate execution order, and report all samples.

A local comparison on 2026-11-04 used nine alternating batches, optimized
dispatch modules and common cached arithmetic artifacts. Process CPU time
reduced interference from other builds; it is not wall latency or throughput.
The serial medians, in microseconds per operation, were:

| Operation | Original adapter | Direct native operations |
| --- | ---: | ---: |
| MSM, 64 bases | 613 | 603 |
| MSM, 8,192 bases | 32,269 | 31,205 |
| FFT, 4,096 values | 616 | 626 |
| FFT, 65,536 values | 14,376 | 14,367 |

Parallel comparisons also checked equal outputs with one and four workers.
Worker scheduling and concurrent machine load produced wide sample spreads
(including slower medians on some small parallel cases), so these measurements
do not establish an arithmetic speedup. The change removes one registry package
without changing the arithmetic implementation; the identity guard also fixes
the reproduced panic. Use a quiet host and matching build profiles for release
performance qualification.
