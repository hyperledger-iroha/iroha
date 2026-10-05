---
title: FASTPQ Metal Kernel Suite
---

# FASTPQ Metal Kernel Suite

The Apple Silicon backend requires one privately admitted immutable compiled
`fastpq.metallib`. Discovery and the common loader check its independently reviewed
length/digest, eight ordered source inputs, target/language/producer identity and
all sixteen entry points before device setup or private staging. No genuine
approved bytes/pins are supplied in the current source: the bundle is `None`, so
Metal eligibility is absent. Ordinary builds and runtime never compile Metal
source or resolve a library path.

The canonical sources remain under `crates/fastpq_prover/metal/`. Their sole
explicit offline producer is `scripts/build_fastpq_metal_bundle.py`; its output is
an unqualified candidate, never its own admission authority or signed provenance.
The existing context owners retain queues, pipelines, clearing buffers and sticky
uncertain-completion refusal. Driver allocations, supported artifact packaging,
complete calibration, hardware parity and uncertain-input recovery remain open.

Goldilocks Poseidon kernels use an explicit three-scalar-word state structure.
On Apple Metal 32023.883, the previous `ulong3` arrays produced incorrect
non-leading sponge states when a lane processed multiple states. The scalar
structure passes direct native permutation, row-hash and column-hash parity
against an independent big-integer oracle on Apple M4 Max, including partial
chunks and near-modulus inputs. The Rust hardware regression
`poseidon_multi_state_chunks_match_cpu_edge_vectors` exercises 1/4/8 states per
lane and repeated dispatches. Production retains one state per lane until the
supported-device qualification matrix is complete; local kernel parity is not
an end-to-end prover or production performance qualification.

## Kernel inventory

| Entry point | Operation | Threadgroup cap | Tile stage cap | Notes |
| ----------- | --------- | --------------- | -------------- | ----- |
| `exact_root_bit_reverse_v1` | Exact-root FFT preparation | 256 threads | — | Each lower index swaps its disjoint reversed-index pair once. A device resource barrier precedes tile evaluation. |
| `exact_root_local_tiles_v1` | Exact-root FFT/IFFT local stages | 256 threads | 8 stages | Each group owns one 256-word tile. Small transforms apply inverse normalization in this final tile. |
| `exact_root_global_stage_v1` | One exact-root FFT/IFFT global stage | 256 threads | — | Each group owns at most 2048 independent butterflies. The host inserts a resource barrier between stages; the last stage applies inverse normalization. |
| `fastpq_fft_columns` | Forward FFT over trace columns | 256 threads | 8 stages | Uses shared-memory tiles for the first stages and applies inverse scaling when the planner requests an IFFT mode.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_fft_post_tiling` | Completes FFT/IFFT/LDE after the tile depth is reached | 256 threads | — | Runs the remaining butterflies directly out of device memory and handles the final coset/inverse factors before returning to the host.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | Low-degree extension across columns | 256 threads | 8 stages | Copies coefficients into the evaluation buffer, executes tiled stages with the configured coset, and leaves the final stages to `fastpq_fft_post_tiling` when needed.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_permute` | Dense-MDS Goldilocks `x^7` permutation (STATE_WIDTH = 3) | 256 threads | — | Threadgroups cache the round constants/MDS rows in threadgroup memory. Production Goldilocks dispatches assign one independent state per lane and size the grid from the actual state count; there is no artificial minimum-thread floor.【crates/fastpq_prover/metal/kernels/poseidon.metal:1】【crates/fastpq_prover/src/metal.rs:3115】
| `poseidon_hash_columns` | Hash flattened column payloads | 256 threads | — | Absorbs each domain-separated padded payload entirely on-device and returns one state per column.【crates/fastpq_prover/metal/kernels/poseidon.metal:353】
| `poseidon_hash_rows` | Hash independent trace rows | 256 threads | — | Reads column-major values and writes row digests in row order using one state per lane.【crates/fastpq_prover/metal/kernels/poseidon.metal:454】
| `bn254_fft_columns` | BN254 FFT over one canonical-limb column | Pipeline limit | — | A cooperative single threadgroup uses packed `n - 1` stage twiddles and deterministic Montgomery arithmetic.【crates/fastpq_prover/metal/kernels/bn254.metal:257】
| `bn254_lde_columns` | BN254 coset LDE over one canonical-limb column | Pipeline limit | — | A cooperative single threadgroup performs coset scaling and the packed-twiddle FFT; the host bounds retained command buffers while dispatching columns.【crates/fastpq_prover/metal/kernels/bn254.metal:313】
| `bn254_poseidon_hash_words` | BN254 Poseidon word-batch hashing | 128 threads | — | Converts canonical limbs to Montgomery form, hashes the requested word slices, and returns canonical BN254 digest bytes.【crates/fastpq_prover/metal/kernels/bn254.metal:532】

| `fastpq_digest384_last_fields` | Six-lane last-fields continuation | Pipeline limit | — | Preserves canonical field-word and state order. |
| `digest384_hash_frames_v1` | Canonical framed six-lane Digest384 | Pipeline limit | — | Processes admitted complete byte frames. |
| `digest384_indexed_first_coordinate_v1` | Indexed first-coordinate Digest384 | Pipeline limit | — | Preserves indexed ordered digest geometry. |
| `fastpq_sha3_256_continuations` | SHA3-256 byte continuation | Pipeline limit | — | Uses the original continuation kernel and exact byte length. |

The descriptors are available at runtime via
`fastpq_prover::metal_kernel_descriptors()` for tooling that wants to display
the same metadata.

The bounded `goldilocks_transform` API selects the three `exact_root_*` kernels
on Metal. Its supplied root is validated for exact order and its input words
for canonicality before dispatch. One command contains bit reversal, local
tiles, and the remaining stages; explicit resource barriers separate every
dependent dispatch. Threadgroup barriers are used only within a tile. The
implementation is `src/metal_exact_root.rs` and `metal/kernels/exact_root.metal`.

One clearing shared buffer holds all accepted columns (at most eight). Caller
columns are copied back only after successful completion of the entire command,
so a drained failure leaves every original input intact. The existing bounded
ticket owner drains abandoned commands; uncertain completion retains device
ownership and makes subsequent bounded-prover admission fail. The public Metal
payload allowance remains conservative: it reserves both a rollback-sized
payload and staging, although this path only needs unpublished staging. This
arithmetic implementation does not activate a proof profile or establish whole
prover performance. The existing profile FFT/LDE callers still use their own
dispatchers; their migration needs separate shape, coset and lifetime controls.

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

Ordinary macOS Cargo builds do not run `xcrun`, `metal` or `metallib`. The
explicit producer captures all eight exact source owners and compiles all six
translation units in a fresh retained generation directory. It checks actual
compiler/linker versions and file digests, successful natural terminal exits,
fresh non-empty outputs and source/tool currentness before create-only publication.
It verifies that the real compiler advertises `-fno-fast-math` before using that
option with the original `-std=macos-metal2.4 -O3` flags. No timeout, signal,
automatic cleanup, installation or download is performed.

Select an installed full Xcode toolchain before explicitly producing a candidate:

```bash
python3 scripts/build_fastpq_metal_bundle.py \
  --target aarch64-apple-darwin --output target/fastpq-metal-candidate
```

The output parent must already exist; the candidate path must be new. `--skip`
is explicit non-generation without source, output or tool access. Generation
metadata is published last and records an unqualified candidate. Retained partial
outputs never become an admitted library. Independent target/pin review, repeated
genuine generation, all sixteen real pipeline loads and complete output parity
are still required. A producer receipt alone does not enable Metal.

The common loader uses only admitted compiled bytes through
`MTLDevice::new_library_with_data`; it does not read runtime files or compile MSL.
This does not qualify driver-internal compilation or allocation behavior. Required
GPU policies keep their original operational refusal; optional policies keep
existing deterministic CPU behavior. Sticky `CompletionUncertain` remains a safety
boundary, not a claim that eventual original-input recovery is complete.

## Threadgroup sizing heuristics

`metal_config::fft_tuning` threads the device execution width and max threads per
threadgroup into the planner so runtime dispatches respect the hardware limits.
The defaults clamp to 32/64/128/256 lanes as the log-size increases. The
256-word threadgroup tile can hold butterflies for at most eight radix-2 stages;
smaller domains retain the five-/four-stage heuristics, and wider stages are
handed to the post-tiling kernel. Debug-build developer
overrides (`FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`) flow through
`FftArgs::threadgroup_lanes`/`local_stage_limit` and are applied by the kernels
above without rebuilding the metallib. Release builds ignore them, and they stop
applying once configured Metal overrides freeze the environment shim.【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

Use `fastpq_metal_bench` to capture the resolved tuning values and verify that
the multi-pass kernels were exercised (`post_tile_dispatches` in the JSON) before
shipping a benchmark bundle.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】
