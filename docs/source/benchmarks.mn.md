---
lang: mn
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2025-12-29T18:16:35.920013+00:00"
translation_last_reviewed: 2026-02-07
---

# Benchmarking Report

Detailed per-run snapshots and the FASTPQ WP5-B history live in
[`benchmarks/history.md`](benchmarks/history.md); use that index when attaching
artefacts to roadmap reviews or SRE audits. Regenerate it with
`python3 scripts/fastpq/update_benchmark_history.py` whenever new GPU captures
or Poseidon manifests land.

## Acceleration evidence bundle

Every GPU or mixed-mode benchmark must include the applied acceleration settings
so WP6-B/WP6-C can prove configuration parity alongside the timing artefacts.

- Capture the runtime snapshot before/after each run:
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  (use `--format table` for human-readable logs). This records `enable_{metal,cuda}`,
  Merkle thresholds, SHA-2 CPU bias limits, the detected backend health bits, and any
  sticky parity errors or disable reasons.
- Store the JSON next to the wrapped benchmark output
  (`artifacts/fastpq_benchmarks/*.json`, `benchmarks/poseidon/*.json`, Merkle sweep
  captures, etc.) so reviewers can diff timings and configuration together.
- Knob definitions and defaults live in `docs/source/config/acceleration.md`; when
  overrides are applied (e.g., `ACCEL_MERKLE_MIN_LEAVES_GPU`, `ACCEL_ENABLE_CUDA`),
  note them in the run metadata to keep reruns reproducible across hosts.

## Norito stage-1 benchmark (WP5-B/C)

- Command: `cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  emits JSON + Markdown under `benchmarks/norito_stage1/` with per-size timings
  for the scalar vs accelerated structural-index builder.
- Latest runs (macOS aarch64, dev profile) live at
  `benchmarks/norito_stage1/latest.{json,md}` and the fresh cutover CSV from
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) shows SIMD
  wins from ~6–8 KiB onwards. GPU/parallel Stage-1 now defaults to a **192 KiB**
  cutoff (`NORITO_STAGE1_GPU_MIN_BYTES=<n>` to override) to avoid launch thrash
  on small documents while enabling accelerators for larger payloads.

## Enum vs Trait Object Dispatch

- Compile time (debug build): 16.58s
- Runtime (Criterion, lower is better):
  - `enum`: 386 ps (average)
  - `trait_object`: 1.56 ns (average)

These measurements come from a microbenchmark comparing an enum-based dispatch against a boxed trait object implementation.

## Poseidon CUDA batching

The Poseidon benchmark (`crates/ivm/benches/bench_poseidon.rs`) now includes workloads that exercise both single-hash permutations and the new batched helpers. Run the suite with:

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

Criterion will record results under `target/criterion/poseidon*_many`. When a GPU worker is available, export the JSON summaries (e.g., copy `target/criterion/**/new/benchmark.json` into `benchmarks/poseidon/criterion_poseidon2_many_cuda.json`) (e.g., copy `target/criterion/**/new/benchmark.json` into `benchmarks/poseidon/`) so downstream teams can compare CPU vs CUDA throughput for each batch size. Until the dedicated GPU lane goes live, the benchmark falls back to the SIMD/CPU implementation and still provides useful regression data for batch performance.

For repeatable captures (and to keep parity evidence with timing data), run

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```

which seeds deterministic Poseidon2/6 batches, records CUDA health/disable reasons, checks
parity against the scalar path, and emits ops/sec + speedup summaries alongside the Metal
runtime status (feature flag, availability, last error). CPU-only hosts still write the scalar
reference and note the missing accelerator, so CI can publish artefacts even without a GPU
runner.

## FASTPQ Metal benchmark (Apple Silicon)

The GPU lane captured an updated end-to-end run of `fastpq_metal_bench` on macOS 14 (arm64) with the lane-balanced parameter set, 20,000 logical rows (padded to 32,768), and 16 column groups. The wrapped artefact lives at `artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json`, with the Metal trace stored alongside the previous captures under `traces/fastpq_metal_trace_*_rows20000_iter5.trace`. The averaged timings (from `benchmarks.operations[*]`) now read:

| Operation | CPU mean (ms) | Metal mean (ms) | Speedup (x) |
|-----------|---------------|-----------------|-------------|
| FFT (32,768 inputs) | 83.29 | 79.95 | 1.04 |
| IFFT (32,768 inputs) | 93.90 | 78.61 | 1.20 |
| LDE (262,144 inputs) | 669.54 | 657.67 | 1.02 |
| Poseidon hash columns (524,288 inputs) | 29,087.53 | 30,004.90 | 0.97 |

Observations:

- FFT/ IFFT both benefit from the refreshed BN254 kernels (IFFT clears the previous regression by ~20%).
- LDE remains near parity; zero-fill now records 33,554,432 padded bytes with an 18.66 ms average so the JSON bundle captures the queue impact.
- Poseidon hashing is still CPU-bound on this hardware; keep comparing against the Poseidon microbench manifests until the Metal path adopts the latest queue controls.
- Each capture now records `AccelerationSettings.runtimeState().metal.lastError`, letting
  engineers annotate CPU fallbacks with the specific disable reason (policy toggle,
  parity failure, no device) directly in the benchmark artefact.

To reproduce the run, build the Metal kernels and execute:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

Commit the resulting JSON under `artifacts/fastpq_benchmarks/` together with the Metal trace so the determinism evidence stays reproducible.

## FASTPQ CUDA automation

CUDA hosts can run and wrap the SM80 benchmark in one step with:

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

The helper invokes `fastpq_cuda_bench`, threads through labels/device/notes, honours
`--require-gpu`, and (by default) wraps/signs via `scripts/fastpq/wrap_benchmark.py`.
Outputs include the raw JSON, the wrapped bundle under `artifacts/fastpq_benchmarks/`,
and a `<name>_plan.json` next to the output that records the exact commands/env so
Stage 7 captures stay reproducible across GPU runners. Add `--sign-output` and
`--gpg-key <id>` when signatures are required; use `--dry-run` to emit only the
plan/paths without executing the bench.

### GA release capture (macOS 14 arm64, lane-balanced)

To satisfy WP2-D we also recorded a release build on the same host with GA-ready
queue heuristics and published it as
`fastpq_metal_bench_20k_release_macos14_arm64.json`. The artefact captures two
column batches (lane-balanced, padded to 32,768 rows) and includes Poseidon
microbench samples for dashboard consumption.

| Operation | CPU mean (ms) | Metal mean (ms) | Speedup | Notes |
|-----------|---------------|-----------------|---------|-------|
| FFT (32,768 inputs) | 12.741 | 10.963 | 1.16× | GPU kernels track the refreshed queue thresholds. |
| IFFT (32,768 inputs) | 17.499 | 25.688 | 0.68× | Regression traced to conservative queue fan-out; keep tuning the heuristics. |
| LDE (262,144 inputs) | 68.389 | 65.701 | 1.04× | Zero-fill logs 33,554,432 bytes in 9.651 ms for both batches. |
| Poseidon hash columns (524,288 inputs) | 1,728.835 | 1,447.076 | 1.19× | GPU finally beats CPU after the Poseidon queue tweaks. |

Poseidon microbench values embedded in the JSON show a 1.10× speedup (default lane
596.229 ms vs scalar 656.251 ms across five iterations), so dashboards can now chart
per-lane improvements alongside the main bench. Reproduce the run with:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

Keep the wrapped JSON and `FASTPQ_METAL_TRACE_CHILD=1` traces checked in under
`artifacts/fastpq_benchmarks/` so subsequent WP2-D/WP2-E reviews can diff the GA
capture against earlier refresh runs without rerunning the workload.

Each fresh `fastpq_metal_bench` capture now also writes a `bn254_metrics` block,
which exposes `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` entries for the CPU
baseline and whichever GPU backend (Metal/CUDA) was active, **and** a
`bn254_dispatch` block that records the observed threadgroup widths, logical thread
counts, and pipeline limits for the single-column BN254 FFT/LDE dispatches. The
benchmark wrapper copies both maps into `benchmarks.bn254_*`, so dashboards and
Prometheus exporters can scrape labelled latencies and geometry without re-parsing
the raw operations array. The `FASTPQ_METAL_THREADGROUP` override now applies to
BN254 kernels as well, making threadgroup sweeps reproducible from one knob.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_benchmark.py:1037】

To keep downstream dashboards simple, run `python3 scripts/benchmarks/export_csv.py`
after capturing a bundle. The helper flattens `poseidon_microbench_*.json` into
matching `.csv` files so automation jobs can diff default and scalar lanes without
custom parsers.

## Poseidon microbench (Metal)

`fastpq_metal_bench` now re-executes itself under `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` and promotes the timings into `benchmarks.poseidon_microbench`. We exported the latest Metal captures with `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` and aggregated them via `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`. The summaries below live under `benchmarks/poseidon/`:

| Summary | Wrapped bundle | Default mean (ms) | Scalar mean (ms) | Speedup vs scalar | Columns x states | Iterations |
|---------|----------------|-------------------|------------------|-------------------|------------------|------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1,990.49 | 1,994.53 | 1.002 | 64 x 262,144 | 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2,167.66 | 2,152.18 | 0.993 | 64 x 262,144 | 5 |

Both captures hashed 262,144 states per run (trace log2 = 12) with a single warm-up iteration. The "default" lane corresponds to the tuned multi-state kernel whereas "scalar" locks the kernel to one state per lane for comparison.

## Merkle threshold sweeps

The `merkle_threshold` example (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) stresses the Metal-vs-CPU Merkle hashing paths. The latest Apple Silicon capture (Darwin 25.0.0 arm64, `ivm::metal_available()=true`) lives in `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` with a matching CSV export. CPU-only macOS 14 baselines remain under `benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json` for hosts without Metal.

| Leaves | CPU best (ms) | Metal best (ms) | Speedup |
|--------|---------------|-----------------|---------|
| 1,024  | 23.01 | 19.69 | 1.17× |
| 4,096  | 50.87 | 62.12 | 0.82× |
| 8,192  | 95.77 | 96.57 | 0.99× |
| 16,384 | 64.48 | 58.98 | 1.09× |
| 32,768 | 109.49 | 87.68 | 1.25× |
| 65,536 | 177.72 | 137.93 | 1.29× |

Larger leaf counts benefit from Metal (1.09–1.29×); smaller buckets still run faster on CPU, so the CSV keeps both columns for analysis. The CSV helper preserves the `metal_available` flag beside each profile to keep GPU vs CPU regression dashboards aligned.

Reproduction steps:

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

Set `FASTPQ_METAL_LIB`/`FASTPQ_GPU` if the host requires explicit Metal enabling, and keep both CPU + GPU captures checked in so WP1-F can chart the policy thresholds.

When running from a headless shell, set `IVM_DEBUG_METAL_ENUM=1` to log device enumeration and `IVM_FORCE_METAL_ENUM=1` to bypass `MTLCreateSystemDefaultDevice()`. The CLI warms up the CoreGraphics session **before** asking for the default Metal device and falls back to `MTLCreateSystemDefaultDevice()` when `MTLCopyAllDevices()` returns zero; if the host still reports no devices the capture will retain `metal_available=false` (useful CPU baselines live under `macos14_arm64_*`), while GPU hosts should keep `FASTPQ_GPU=metal` enabled so the bundle logs the chosen backend.

`fastpq_metal_bench` exposes a similar knob via `FASTPQ_DEBUG_METAL_ENUM=1`, which prints the `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` results before the backend decides whether to stay on the GPU path. Enable it whenever `FASTPQ_GPU=gpu` still reports `backend="none"` in the wrapped JSON so the capture bundle records exactly how the host enumerated Metal hardware; the harness aborts immediately when `FASTPQ_GPU=gpu` is set but no accelerator is detected, pointing at the debug knob so the release bundle never hides a CPU fallback behind a forced GPU run.【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】

The CSV helper emits per-profile tables (for example `macos14_arm64_*.csv` and `takemiyacStudio.lan_25.0.0_arm64.csv`), preserving the `metal_available` flag so regression dashboards can ingest the CPU and GPU measurements without bespoke parsers.
