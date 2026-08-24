---
title: FASTPQ Metal Kernel Suite
---

# FASTPQ Metal Kernel Suite

The Apple Silicon backend prefers a single build-generated `fastpq.metallib`
that contains every Metal Shading Language (MSL) kernel exercised by the
prover. The same sources are embedded as a runtime-compilation fallback, so a
missing build artifact does not make visible Metal hardware unavailable. This
note explains the available entry points, their threadgroup limits, and the
determinism guarantees that make the GPU path interchangeable with the scalar
fallback.

The canonical implementation lives under
`crates/fastpq_prover/metal/kernels/` and is compiled by
`crates/fastpq_prover/build.rs` whenever `fastpq-gpu` is enabled on macOS.
Runtime metadata (`metal_kernel_descriptors`) mirrors the information below so
benchmarks and diagnostics can surface the same facts programmatically.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## Kernel inventory

| Entry point | Operation | Threadgroup cap | Tile stage cap | Notes |
| ----------- | --------- | --------------- | -------------- | ----- |
| `fastpq_fft_columns` | Forward FFT over trace columns | 256 threads | 8 stages | Uses shared-memory tiles for the first stages and applies inverse scaling when the planner requests an IFFT mode.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_fft_post_tiling` | Completes FFT/IFFT/LDE after the tile depth is reached | 256 threads | — | Runs the remaining butterflies directly out of device memory and handles the final coset/inverse factors before returning to the host.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | Low-degree extension across columns | 256 threads | 8 stages | Copies coefficients into the evaluation buffer, executes tiled stages with the configured coset, and leaves the final stages to `fastpq_fft_post_tiling` when needed.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_permute` | Dense-MDS Goldilocks `x^7` permutation (STATE_WIDTH = 3) | 256 threads | — | Threadgroups cache the round constants/MDS rows in threadgroup memory. Production Goldilocks dispatches assign one independent state per lane and size the grid from the actual state count; there is no artificial minimum-thread floor. The source filename remains `poseidon2.metal` for build compatibility; the construction is not Poseidon2.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal.rs:3115】
| `poseidon_hash_columns` | Hash flattened column payloads | 256 threads | — | Absorbs each domain-separated padded payload entirely on-device and returns one state per column.【crates/fastpq_prover/metal/kernels/poseidon2.metal:353】
| `poseidon_hash_rows` | Hash independent trace rows | 256 threads | — | Reads column-major values and writes row digests in row order using one state per lane.【crates/fastpq_prover/metal/kernels/poseidon2.metal:454】
| `poseidon_trace_fused` | Hash trace columns into a combined leaf/parent buffer | 256 threads | — | Writes the leaf slice into the combined buffer. The host waits for global visibility before launching `poseidon_trace_parents` for the depth-1 layer.【crates/fastpq_prover/metal/kernels/poseidon2.metal:524】
| `poseidon_trace_parents` | Compute depth-1 trace Merkle parents | 256 threads | — | Hashes adjacent leaves after the leaf pass completes; odd leaf counts duplicate the final leaf exactly like the CPU builder.【crates/fastpq_prover/metal/kernels/poseidon2.metal:596】
| `bn254_fft_columns` | BN254 FFT over one canonical-limb column | Pipeline limit | — | A cooperative single threadgroup uses packed `n - 1` stage twiddles and deterministic Montgomery arithmetic.【crates/fastpq_prover/metal/kernels/bn254.metal:257】
| `bn254_lde_columns` | BN254 coset LDE over one canonical-limb column | Pipeline limit | — | A cooperative single threadgroup performs coset scaling and the packed-twiddle FFT; the host bounds retained command buffers while dispatching columns.【crates/fastpq_prover/metal/kernels/bn254.metal:313】
| `bn254_poseidon_hash_words` | BN254 Poseidon word-batch hashing | 128 threads | — | Converts canonical limbs to Montgomery form, hashes the requested word slices, and returns canonical BN254 digest bytes.【crates/fastpq_prover/metal/kernels/bn254.metal:532】

The descriptors are available at runtime via
`fastpq_prover::metal_kernel_descriptors()` for tooling that wants to display
the same metadata.

## Deterministic Goldilocks arithmetic

- All kernels work over the Goldilocks field with helpers defined in
  `field.metal` (modular add/mul/sub, inverses, and `pow7`). FASTPQ's shared
  Goldilocks path exposes only the bijective `pow7` S-box; BN254 kernels keep
  their separately specified `x^5` helper in `bn254.metal`.
  【crates/fastpq_prover/metal/kernels/field.metal:1】【crates/fastpq_prover/metal/kernels/bn254.metal:408】
- FFT/LDE stages reuse the same twiddle tables that the CPU planner produces.
  `compute_stage_twiddles` precomputes one twiddle per stage and the host
  uploads the array through buffer slot 1 before each dispatch, guaranteeing the
  GPU path uses identical roots of unity.【crates/fastpq_prover/src/metal.rs:1527】
- Coset multiplication for LDE is fused into the final stage so the GPU never
  diverges from the CPU trace layout; the host zero-fills the evaluation buffer
  before dispatch, keeping padding behaviour deterministic.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Metallib generation

`build.rs` first resolves and executes both `metal -v` and `metallib -v`. Unless
`FASTPQ_SKIP_GPU_BUILD` opts out, it runs exactly
`xcodebuild -downloadComponent MetalToolchain` only when either probe fails,
clears the `xcrun` cache, and probes both tools again. It then compiles the
individual `.metal` sources into non-empty `.air` objects and links them into a
non-empty `fastpq.metallib`, exporting every entry point listed above. Bootstrap
diagnostics identify initial lookup/probe failures, cache-clear or redetection
failures, and Xcode selection/license remediation. If toolchain bootstrap or
offline compilation fails, the runtime concatenates the prelude, parameters,
field helpers, and all kernel translation units into self-contained MSL 2.4
source and creates the same pipelines through
`MTLDevice::new_library_with_source`.
The build script passes the generated Cargo `OUT_DIR` path to the crate at compile
time, so a release loads that library while it remains present. A packaged or
relocated release with a stale path selects the embedded fallback instead;
`FASTPQ_METAL_LIB` is only a debug/dev override, not production configuration.【crates/fastpq_prover/build.rs:210】【crates/fastpq_prover/src/metal.rs:2475】

For parity with CI runs you can regenerate the library manually:

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=macos-metal2.4 -O3 -c -I crates/fastpq_prover/metal/include -I crates/fastpq_prover/metal/kernels crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=macos-metal2.4 -O3 -c -I crates/fastpq_prover/metal/include -I crates/fastpq_prover/metal/kernels crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metal -std=macos-metal2.4 -O3 -c -I crates/fastpq_prover/metal/include -I crates/fastpq_prover/metal/kernels crates/fastpq_prover/metal/kernels/bn254.metal -o "$OUT_DIR/bn254.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" "$OUT_DIR/bn254.air" -o "$OUT_DIR/fastpq.metallib"
```

## Threadgroup sizing heuristics

`metal_config::fft_tuning` threads the device execution width and max threads per
threadgroup into the planner so runtime dispatches respect the hardware limits.
The defaults clamp to 32/64/128/256 lanes as the log-size increases. The
256-word threadgroup tile can hold butterflies for at most eight radix-2 stages;
smaller domains retain the five-/four-stage heuristics, and wider stages are
handed to the post-tiling kernel. Operator
overrides (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) flow through
`FftArgs::threadgroup_lanes`/`local_stage_limit` and are applied by the kernels
above without rebuilding the metallib.【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

Use `fastpq_metal_bench` to capture the resolved tuning values and verify that
the multi-pass kernels were exercised (`post_tile_dispatches` in the JSON) before
shipping a benchmark bundle.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】
