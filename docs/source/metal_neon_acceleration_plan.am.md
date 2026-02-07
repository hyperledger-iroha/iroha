---
lang: am
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2025-12-29T18:16:35.976997+00:00"
translation_last_reviewed: 2026-02-07
---

# Metal & NEON Acceleration Plan (Swift & Rust)

This document captures the shared plan for enabling deterministic hardware
acceleration (Metal GPU + NEON/Accelerate SIMD + StrongBox integration) across
the Rust workspace and the Swift SDK. It addresses the roadmap items tracked
under **Hardware Acceleration Workstream (macOS/iOS)** and provides a hand-off
artifact for the Rust IVM team, Swift bridge owners, and telemetry tooling.

> Last updated: 2026-01-12  
> Owners: IVM Performance TL, Swift SDK Lead

## Goals

1. Reuse the Rust GPU kernels (Poseidon/BN254/CRC64) on Apple hardware via Metal
   compute shaders with deterministic parity against CPU paths.
2. Expose acceleration toggles (`AccelerationConfig`) end-to-end so Swift apps
   can opt into Metal/NEON/StrongBox while preserving ABI/parity guarantees.
3. Instrument CI + dashboards to surface parity/benchmark data and flag
   regressions across CPU vs GPU/SIMD paths.
4. Share StrongBox/secure-enclave lessons between Android (AND2) and Swift
   (IOS4) to keep signing flows deterministically aligned.

**Update (CRC64 + Stage‑1 refresh):** CRC64 GPU helpers are now wired into `norito::core::hardware_crc64` with a 192 KiB default cutoff (override via `NORITO_GPU_CRC64_MIN_BYTES` or explicit helper path `NORITO_CRC64_GPU_LIB`) while retaining SIMD and scalar fallbacks. JSON Stage‑1 cutovers were re-benchmarked (`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`), keeping the scalar cutover at 4 KiB and aligning the Stage‑1 GPU default to 192 KiB (`NORITO_STAGE1_GPU_MIN_BYTES`) so small documents stay on CPU and large payloads amortize GPU launch costs.

## Deliverables & Owners

| Milestone | Deliverable | Owner(s) | Target |
|-----------|-------------|----------|--------|
| Rust WP2-A/B | Metal shader interfaces mirroring CUDA kernels | IVM Perf TL | Feb 2026 |
| Rust WP2-C | Metal BN254 parity tests & CI lane | IVM Perf TL | Q2 2026 |
| Swift IOS6 | Bridge toggles wired (`connect_norito_set_acceleration_config`) + SDK API + samples | Swift Bridge Owners | Done (Jan 2026) |
| Swift IOS5 | Sample apps/docs demonstrating config usage | Swift DX TL | Q2 2026 |
| Telemetry | Dashboard feeds w/ acceleration parity + benchmark metrics | Swift Program PM / Telemetry | Pilot data Q2 2026 |
| CI | XCFramework smoke harness exercising CPU vs Metal/NEON on device pool | Swift QA Lead | Q2 2026 |
| StrongBox | Hardware-backed signing parity tests (shared vectors) | Android Crypto TL / Swift Security | Q3 2026 |

## Interfaces & API Contracts

### Rust (`ivm::AccelerationConfig`)
- Keep existing fields (`enable_simd`, `enable_metal`, `enable_cuda`, `max_gpus`, thresholds).
- Add explicit Metal warm-up to avoid first-use latency (Rust #15875).
- Provide parity APIs returning status/diagnostics for dashboards:
  - e.g. `ivm::vector::metal_status()` -> {enabled, parity, last_error}.
- Output benchmarking metrics (Merkle tree timings, CRC throughput) via
  telemetry hooks for `ci/xcode-swift-parity`.
- Metal host now loads the compiled `fastpq.metallib`, dispatches FFT/IFFT/LDE
  and Poseidon kernels, and falls back to the CPU implementation whenever the
  metallib or device queue is unavailable.

### C FFI (`connect_norito_bridge`)
- New struct `connect_norito_acceleration_config` (completed).
- Getter coverage now includes `connect_norito_get_acceleration_config` (config only) and `connect_norito_get_acceleration_state` (config + parity) to mirror the setter.
- Document struct layout in header comments for SPM/CocoaPods consumers.

### Swift (`AccelerationSettings`)
- Defaults: Metal enabled, CUDA disabled, thresholds nil (inherit).
- Negative values ignored; `apply()` invoked automatically by `IrohaSDK`.
- `AccelerationSettings.runtimeState()` now surfaces the `connect_norito_get_acceleration_state`
  payload (config + Metal/CUDA parity status) so Swift dashboards emit the same telemetry
  as Rust (`supported/configured/available/parity`). The helper returns `nil` when the
  bridge is absent to keep tests portable.
- `AccelerationBackendStatus.lastError` copies the disable/error reason from
  `connect_norito_get_acceleration_state` and frees the native buffer once the string is
  materialised so mobile parity dashboards can annotate why Metal/CUDA were disabled on
  each host.
- `AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`,
  tests under `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift`) now
  resolves operator manifests in the same priority order as the Norito demo: honour
  `NORITO_ACCEL_CONFIG_PATH`, search bundled `acceleration.{json,toml}` / `client.{json,toml}`,
  log the chosen source, and fall back to defaults. Apps no longer need bespoke loaders to
  mirror the Rust `iroha_config` surface.
- Update sample apps & README to show toggles and telemetry integration.

### Telemetry (Dashboards + Exporters)
- Parity feed (mobile_parity.json):
  - `acceleration.metal/neon/strongbox` -> {enabled, parity, perf_delta_pct}.
  - Accept `perf_delta_pct` baseline CPU vs GPU comparison.
  - `acceleration.metal.disable_reason` mirrors `AccelerationBackendStatus.lastError`
    so Swift automation can flag disabled GPUs with the same fidelity as the Rust
    dashboards.
- CI feed (mobile_ci.json):
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {cpu, metal}
  - `acceleration_bench.neon_crc64_throughput_mb_s` -> Double.
- Exporters must source metrics from Rust benchmarks or CI runs (e.g., run
  Metal/CPU microbench as part of `ci/xcode-swift-parity`).

### Configuration knobs & defaults (WP6-C)
- `AccelerationConfig` defaults: `enable_metal = true` on macOS builds, `enable_cuda = true` when the CUDA feature is compiled, `max_gpus = None` (no cap). The Swift `AccelerationSettings` wrapper inherits the same defaults through `connect_norito_set_acceleration_config`.
- Norito Merkle heuristics (GPU vs CPU): `merkle_min_leaves_gpu = 8192` enables GPU hashing for trees with ≥8192 leaves; backend overrides (`merkle_min_leaves_metal`, `merkle_min_leaves_cuda`) default to the same threshold unless explicitly set.
- CPU preference heuristics (SHA2 ISA present): on both AArch64 (ARMv8 SHA2) and x86/x86_64 (SHA-NI) the CPU path remains preferred up to `prefer_cpu_sha2_max_leaves_* = 32_768` leaves; above that the GPU threshold applies. These values are configurable via `AccelerationConfig` and should be adjusted only with benchmark evidence.

## Testing Strategy

1. **Unit parity tests (Rust)**: ensure Metal kernels match CPU outputs for
   deterministic vectors; run under `cargo test -p ivm --features metal`.
   `crates/fastpq_prover/src/metal.rs` now ships macOS-only parity tests that
   exercise FFT/IFFT/LDE and Poseidon against the scalar reference.
2. **Swift smoke harness**: extend IOS6 test runner to execute CPU vs Metal
   encoding (Merkle/CRC64) on both emulators and StrongBox devices; compare
   results and log parity status.
3. **CI**: update `norito_bridge_ios.yml` (already calls `make swift-ci`) to push
   acceleration metrics to artifacts; ensure the run confirms Buildkite
   `ci/xcframework-smoke:<lane>:device_tag` metadata before publishing harness changes,
   and fail the lane on parity/benchmark drift.
4. **Dashboards**: new fields now render in CLI output. Ensure exporters produce
   data before flipping dashboards live.

## WP2-A Metal Shader Plan (Poseidon Pipelines)

The first WP2 milestone covers the planning work for the Poseidon Metal kernels
that mirror the CUDA implementation. The plan splits the effort into kernels,
host scheduling, and shared constant staging so later work can focus purely on
implementation and testing.

### Kernel Scope

1. `poseidon_permute`: permutes `state_count` independent states. Each thread
   owns a `STATE_CHUNK` (4 states) and runs all `TOTAL_ROUNDS` iterations using
   threadgroup-shared round constants staged at dispatch time.
2. `poseidon_hash_columns`: reads the sparse `PoseidonColumnSlice` catalogue and
   performs Merkle-friendly hashing of every column (matching the CPU’s
   `PoseidonColumnBatch` layout). It uses the same threadgroup constant buffer
   as the permute kernel but loops over `(states_per_lane * block_count)`
   outputs so the kernel can amortize queue submissions.
3. `poseidon_trace_fused`: computes the parent/leaf digests for the trace table
   in a single pass. The fused kernel consumes `PoseidonFusedArgs` so the host
   can describe non-contiguous regions and a `leaf_offset`/`parent_offset`, and
   it shares all round/MDS tables with the other kernels.

### Command Scheduling & Host Contracts

- Every kernel dispatch runs through `MetalPipelines::command_queue`, which
  enforces the adaptive scheduler (target ~2 ms) and the queue fan-out controls
  exposed via `FASTPQ_METAL_QUEUE_FANOUT` and
  `FASTPQ_METAL_COLUMN_THRESHOLD`. The warm-up path in `with_metal_state`
  compiles all three Poseidon kernels up-front so the first dispatch does not
  pay a pipeline creation penalty.
- Threadgroup sizing mirrors the existing Metal FFT/LDE defaults: the target is
  8,192 threads per submission with a hard cap of 256 threads per group. The
  host may downshift the `states_per_lane` multiplier for low-power devices by
  dialing the environment overrides (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  to be added in WP2-B) without modifying the shader logic.
- Column staging follows the same double-buffered pool already used by the FFT
  pipelines. The Poseidon kernels accept raw pointers into that staging buffer
  and never touch global heap allocations, which keeps memory-determinism
  aligned with the CUDA host.

### Shared Constants

- The `PoseidonSnapshot` manifest described in
  `docs/source/fastpq/poseidon_metal_shared_constants.md` is now the canonical
  source for the round constants and MDS matrix. Both Metal (`poseidon2.metal`)
  and CUDA (`fastpq_cuda.cu`) kernels must be regenerated whenever the manifest
  changes.
- WP2-B will add a tiny host loader that reads the manifest at runtime and
  emits the SHA-256 into telemetry (`acceleration.poseidon_constants_sha`) so
  parity dashboards can assert that the shader constants match the published
  snapshot.
- During warm-up we will copy the `TOTAL_ROUNDS x STATE_WIDTH` constants into a
  `MTLBuffer` and upload it once per device. Each kernel then copies the data
  into threadgroup memory before processing its chunk, ensuring deterministic
  ordering even when multiple command buffers run in flight.

### Validation Hooks

- Unit tests (`cargo test -p fastpq_prover --features fastpq-gpu`) will grow an
  assertion that hashes the embedded shader constants and compares them with
  the manifest’s SHA before executing the GPU fixture suite.
- The existing kernel statistics toggles (`FASTPQ_METAL_TRACE_DISPATCH`,
  `FASTPQ_METAL_QUEUE_FANOUT`, queue depth telemetry) become required evidence
  for WP2 exit: every test run must prove that the scheduler never violates the
  configured fan-out and that the fused trace kernel keeps the queue below the
  adaptive window.
- The Swift XCFramework smoke harness and the Rust benchmark runners will start
  exporting `acceleration.poseidon.permute_p90_ms{cpu,metal}` so WP2-D can chart
  Metal-vs-CPU deltas without reinventing new telemetry feeds.

## WP2-B Poseidon Manifest Loader & Self-Test Parity

- `fastpq_prover::poseidon_manifest()` now embeds and parses
  `artifacts/offline_poseidon/constants.ron`, computes its SHA-256
  (`poseidon_manifest_sha256()`), and validates the snapshot against the CPU
  poseidon tables before any GPU work runs. `build_metal_context()` logs the
  digest during warm-up so telemetry exporters can publish
  `acceleration.poseidon_constants_sha`.
- The manifest parser rejects mismatched width/rate/round-count tuples and
  ensures the manifest MDS matrix equals the scalar implementation, preventing
  silent drift when the canonical tables are regenerated.
- Added `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs`, which
  parses the Poseidon tables embedded in `poseidon2.metal` and
  `fastpq_cuda.cu` and asserts that both kernels serialize exactly the same
  constants as the manifest. CI now fails if someone edits the shader/CUDA
  files without regenerating the canonical manifest.
- Future parity hooks (WP2-C/D) can reuse `poseidon_manifest()` to stage the
  round constants into GPU buffers and to expose the digest via Norito
  telemetry feeds.

## WP2-C BN254 Metal Pipelines & Parity Tests

- **Scope & gap:** Host dispatchers, parity harnesses, and `bn254_status()` are live, and `crates/fastpq_prover/metal/kernels/bn254.metal` now implements the Montgomery primitives plus threadgroup-synchronized FFT/LDE loops. Each dispatch runs an entire column inside a single threadgroup with per-stage barriers, so the kernels exercise the staged manifests in parallel. Telemetry is now wired and scheduler overrides are honored so we can gate the default-on rollout with the same evidence we use for the Goldilocks kernels.
- **Kernel requirements:** ✅ reuse the staged twiddle/coset manifests, convert inputs/outputs once, and execute all radix-2 stages inside the per-column threadgroup so we don’t need multi-dispatch synchronisation. Montgomery helpers remain shared between FFT/LDE so only the loop geometry changed.
- **Host wiring:** ✅ `crates/fastpq_prover/src/metal.rs` stages canonical limbs, zero-fills the LDE buffer, selects a single threadgroup per column, and exposes `bn254_status()` for gating. No extra host changes are required for telemetry.
- **Build guards:** the `fastpq.metallib` ships the tiled kernels, so CI still fails fast if the shader drifts. Any future optimisations stay behind telemetry/feature gates rather than compile-time switches.
- **Parity fixtures:** ✅ `bn254_parity` tests continue to compare GPU FFT/LDE outputs against CPU fixtures and now run live on Metal hardware; keep tampered-manifest tests in mind if new kernel code paths appear.
- **Telemetry & benchmarks:** `fastpq_metal_bench` now emits:
  - a `bn254_dispatch` block summarising per-dispatch threadgroup widths, logical thread counts, and pipeline limits for FFT/LDE single-column batches; and
  - a `bn254_metrics` block that records `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` for the CPU baseline plus whichever GPU backend ran.
  The benchmark wrapper copies both maps into every wrapped artefact so WP2-D dashboards ingest labelled latencies/geometry without reverse-engineering the raw operations array. `FASTPQ_METAL_THREADGROUP` now applies to BN254 FFT/LDE dispatches as well, making the knob usable for perf sweeps.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_benchmark.py:1037】

## Open Questions (Resolved May 2027)

1. **Metal resource cleanup:** `warm_up_metal()` reuses the thread-local
   `OnceCell` and now has idempotence/regression tests
   (`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state` /
   `warm_up_metal_is_noop_on_non_metal_targets`), so app lifecycle transitions
   can safely call the warm-up path without leaking or double-initialising.
2. **Benchmark baselines:** Metal lanes must remain within 20 % of the CPU
   baseline for FFT/IFFT/LDE and within 15 % for Poseidon CRC/Merkle helpers;
   alerting should fire when `acceleration.*_perf_delta_pct > 0.20` (or missing)
   in the mobile parity feed. IFFT regressions observed in the 20 k trace bundle
   are now gated by the queue override fix noted in WP2-D.
3. **StrongBox fallback:** Swift follows the Android fallback playbook by
   recording attestation failures in the support runbook
   (`docs/source/sdk/swift/support_playbook.md`) and automatically switching to
   the documented HKDF-backed software path with audit logging; parity vectors
   stay shared via the existing OA fixtures.
4. **Telemetry storage:** Acceleration captures and device pool proofs are
   archived under `configs/swift/` (e.g.,
   `configs/swift/xcframework_device_pool_snapshot.json`), and exporters
   should mirror the same layout (`artifacts/swift/telemetry/acceleration/*.json`
   or `.prom`) so Buildkite annotations and portal dashboards can ingest the
   feeds without ad-hoc scraping.

## Next Steps (Feb 2026)

- [x] Rust: land Metal host integration (`crates/fastpq_prover/src/metal.rs`) and
      expose the kernel interface for Swift; doc hand-off tracked alongside the
      Swift bridge notes.
- [x] Swift: expose SDK-level acceleration settings (done Jan 2026).
- [x] Telemetry: `scripts/acceleration/export_prometheus.py` now converts
      `cargo xtask acceleration-state --format json` output into a Prometheus
      textfile (with optional `--instance` label) so CI runs can attach GPU/CPU
      enablement, thresholds, and parity/disable reasons directly to textfile
      collectors without bespoke scraping.
- [x] Swift QA: `scripts/acceleration/acceleration_matrix.py` aggregates multiple
      acceleration-state captures into JSON or Markdown tables keyed by device
      label, giving the smoke harness a deterministic “CPU vs Metal/CUDA” matrix
      to upload alongside the sample-app smokes. The Markdown output mirrors the
      Buildkite evidence format so dashboards can ingest the same artefact.
- [x] Update status.md now that `irohad` ships the queue/zero-fill exporters and
      the env/config validation tests cover the Metal queue overrides, so WP2-D
      telemetry + bindings have live evidence attached.【crates/irohad/src/main.rs:2664】【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【status.md:1546】

Telemetry/export helper commands:

```bash
# Prometheus textfile from a single capture
cargo xtask acceleration-state --format json > artifacts/acceleration_state_macos_m4.json
python3 scripts/acceleration/export_prometheus.py \
  --input artifacts/acceleration_state_macos_m4.json \
  --output artifacts/acceleration_state_macos_m4.prom \
  --instance macos-m4

# Aggregate multiple captures into a Markdown matrix
python3 scripts/acceleration/acceleration_matrix.py \
  --state macos-m4=artifacts/acceleration_state_macos_m4.json \
  --state sim-m3=artifacts/acceleration_state_sim_m3.json \
  --format markdown \
  --output artifacts/acceleration_matrix.md
```

## WP2-D Release Benchmark & Binding Notes

- **20 k-row release capture:** Recorded a fresh Metal vs CPU benchmark on macOS 14
  (arm64, lane-balanced parameters, padded 32,768-row trace, two column batches) and
  checked the JSON bundle into `fastpq_metal_bench_20k_release_macos14_arm64.json`.
  The benchmark exports per-operation timings plus Poseidon microbench evidence so
  WP2-D has a GA-quality artefact tied to the new Metal queue heuristics. Headline
  deltas (full table lives in `docs/source/benchmarks.md`):

  | Operation | CPU mean (ms) | Metal mean (ms) | Speedup |
  |-----------|---------------|-----------------|---------|
  | FFT (32,768 inputs) | 12.741 | 10.963 | 1.16× |
  | IFFT (32,768 inputs) | 17.499 | 25.688 | 0.68× *(regression: queue fan-out throttled to keep determinism; needs follow-up tuning)* |
  | LDE (262,144 inputs) | 68.389 | 65.701 | 1.04× |
  | Poseidon hash columns (524,288 inputs) | 1,728.835 | 1,447.076 | 1.19× |

  Each capture logs `zero_fill` timings (9.651 ms for 33,554,432 bytes) and
  `poseidon_microbench` entries (default lane 596.229 ms vs scalar 656.251 ms,
  1.10× speedup) so dashboard consumers can diff queue pressure alongside the
  main operations.
- **Bindings/docs cross-link:** `docs/source/benchmarks.md` now references the
  release JSON and reproducer command, the Metal queue overrides are validated
  via `iroha_config` env/manifest tests, and `irohad` publishes live
  `fastpq_metal_queue_*` gauges so dashboards flag IFFT regressions without
  ad-hoc log scraping. Swift’s `AccelerationSettings.runtimeState` exposes the
  same telemetry payload shipped in the JSON bundle, closing the WP2-D
  binding/doc gap with a reproducible acceptance baseline.【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **IFFT queue fix:** Inverse FFT batches now skip multi-queue dispatch whenever
  the workload barely meets the fan-out threshold (16 columns on the lane-balanced
  profile), removing the Metal-vs-CPU regression called out above while keeping
  large-column workloads on the multi-queue path for FFT/LDE/Poseidon.
