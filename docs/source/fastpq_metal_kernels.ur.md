---
lang: ur
direction: rtl
source: docs/source/fastpq_metal_kernels.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0022f5f9c53445d26876f0097635092b5c685d332bfa25b13243c584d358dfe
source_last_modified: "2026-01-04T10:50:53.612910+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: FASTPQ Metal Kernel Suite
---

# FASTPQ Metal Kernel Suite

The Apple Silicon backend ships a single `fastpq.metallib` that contains every
Metal Shading Language (MSL) kernel exercised by the prover. This note explains
the available entry points, their threadgroup limits, and the determinism
guarantees that make the GPU path interchangeable with the scalar fallback.

The canonical implementation lives under
`crates/fastpq_prover/metal/kernels/` and is compiled by
`crates/fastpq_prover/build.rs` whenever `fastpq-gpu` is enabled on macOS.
Runtime metadata (`metal_kernel_descriptors`) mirrors the information below so
benchmarks and diagnostics can surface the same facts programmatically.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## Kernel inventory

| Entry point | Operation | Threadgroup cap | Tile stage cap | Notes |
| ----------- | --------- | --------------- | -------------- | ----- |
| `fastpq_fft_columns` | Forward FFT over trace columns | 256 threads | 32 stages | Uses shared-memory tiles for the first stages and applies inverse scaling when the planner requests an IFFT mode.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_fft_post_tiling` | Completes FFT/IFFT/LDE after the tile depth is reached | 256 threads | — | Runs the remaining butterflies directly out of device memory and handles the final coset/inverse factors before returning to the host.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | Low-degree extension across columns | 256 threads | 32 stages | Copies coefficients into the evaluation buffer, executes tiled stages with the configured coset, and leaves the final stages to `fastpq_fft_post_tiling` when needed.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_trace_fused` | Hash columns and compute depth‑1 parents in one pass | 256 threads | — | Runs the same absorption/permutation as `poseidon_hash_columns`, stores the leaf digests directly into the output buffer, and immediately folds each `(left,right)` pair under the `fastpq:v1:trace:node` domain so `(⌈columns / 2⌉)` parents land after the leaf slice. Odd column counts duplicate the final leaf on-device, eliminating the follow-up kernel and the CPU fallback for the first Merkle layer.【crates/fastpq_prover/metal/kernels/poseidon2.metal:384】【crates/fastpq_prover/src/metal.rs:2407】
| `poseidon_permute` | Poseidon2 permutation (STATE_WIDTH = 3) | 256 threads | — | Threadgroups cache the round constants/MDS rows in threadgroup memory, copy the MDS rows into per-thread registers, and process states in 4-state chunks so each round constant fetch is reused across multiple states before advancing. The rounds stay fully unrolled and every lane still walks multiple states, guaranteeing ≥4 096 logical threads per dispatch. `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` pin the launch width and per-lane batch without rebuilding the metallib.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】

The descriptors are available at runtime via
`fastpq_prover::metal_kernel_descriptors()` for tooling that wants to display
the same metadata.

## Deterministic Goldilocks arithmetic

- All kernels work over the Goldilocks field with helpers defined in
  `field.metal` (modular add/mul/sub, inverses, `pow5`).【crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE stages reuse the same twiddle tables that the CPU planner produces.
  `compute_stage_twiddles` precomputes one twiddle per stage and the host
  uploads the array through buffer slot 1 before each dispatch, guaranteeing the
  GPU path uses identical roots of unity.【crates/fastpq_prover/src/metal.rs:1527】
- Coset multiplication for LDE is fused into the final stage so the GPU never
  diverges from the CPU trace layout; the host zero-fills the evaluation buffer
  before dispatch, keeping padding behaviour deterministic.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Metallib generation

`build.rs` compiles the individual `.metal` sources into `.air` objects and then
links them into `fastpq.metallib`, exporting every entry point listed above.
Setting `FASTPQ_METAL_LIB` to that path (the build script does this
automatically) allows the runtime to load the library deterministically regardless
of where `cargo` placed the build artifacts.【crates/fastpq_prover/build.rs:45】

For parity with CI runs you can regenerate the library manually:

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## Threadgroup sizing heuristics

`metal_config::fft_tuning` threads the device execution width and max threads per
threadgroup into the planner so runtime dispatches respect the hardware limits.
The defaults clamp to 32/64/128/256 lanes as the log-size increases, and the
tile depth now walks from five stages to four at `log_len ≥ 12`, then keeps the
shared-memory pass active for 12/14/16 stages once the trace crosses
`log_len ≥ 18/20/22` before handing work to the post-tiling kernel. Operator
overrides (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) flow through
`FftArgs::threadgroup_lanes`/`local_stage_limit` and are applied by the kernels
above without rebuilding the metallib.【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

Use `fastpq_metal_bench` to capture the resolved tuning values and verify that
the multi-pass kernels were exercised (`post_tile_dispatches` in the JSON) before
shipping a benchmark bundle.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】
