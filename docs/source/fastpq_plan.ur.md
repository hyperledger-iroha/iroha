---
lang: ur
direction: rtl
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-18T05:31:56.951617+00:00"
translation_last_reviewed: 2026-01-30
---

# FASTPQ Prover Work Breakdown

This document captures the staged plan for delivering a production-ready FASTPQ-ISI prover and wiring it into the data-space scheduling pipeline. Every definition below is normative unless marked as a TODO. Estimated soundness uses Cairo-style DEEP-FRI bounds; automated rejection-sampling tests in CI fail if the measured bound drops below 128 bits.

## Stage 0 — Hash Placeholder (landed)
- Deterministic Norito encoding with BLAKE2b commitment.
- Placeholder backend returning `BackendUnavailable`.
- Canonical parameter table provided by `fastpq_isi`.

## Stage 1 — Trace Builder Prototype

> **Status (2025-11-09):** `fastpq_prover` now exposes canonical packing
> helpers (`pack_bytes`, `PackedBytes`) and the deterministic Poseidon2
> ordering commitment over Goldilocks. Constants are pinned to
> `ark-poseidon2` commit `3f2b7fe`, closing the follow-up about swapping out the interim BLAKE2
> placeholder is closed. Golden fixtures (`tests/fixtures/packing_roundtrip.json`,
> `tests/fixtures/ordering_hash.json`) now anchor the regression suite.

### Objectives
- Implement the FASTPQ trace builder for the KV-update AIR. Each row must encode:
  - `key_limbs[i]`: base-256 limbs (7 bytes, little-endian) of the canonical key path.
  - `value_old_limbs[i]`, `value_new_limbs[i]`: same packing for pre/post values.
  - Selector columns: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
  - Auxiliary columns: `delta = value_new - value_old`, `running_asset_delta`, `metadata_hash`, `supply_counter`.
  - Asset columns: `asset_id_limbs[i]` using 7-byte limbs.
  - SMT columns per level `ℓ`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, plus `neighbour_leaf` for non-membership.
  - Metadata columns: `dsid`, `slot`.
- **Deterministic ordering.** Sort rows lexicographically by `(key_bytes, op_rank, original_index)` using a stable sort. `op_rank` mapping: `transfer=0`, `mint=1`, `burn=2`, `role_grant=3`, `role_revoke=4`, `meta_set=5`. `original_index` is the 0-based index before sorting. Persist the resulting Poseidon2 ordering hash (domain tag `fastpq:v1:ordering`). Encode the hash preimage as `[domain_len, domain_limbs…, payload_len, payload_limbs…]` where lengths are u64 field elements so trailing zero bytes remain distinguishable.
- Lookup witness: produce `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)` when the stored column `s_perm` (logical OR of `s_role_grant` and `s_role_revoke`) is 1. Role/permission IDs are fixed-width 32-byte LE strings; epoch is 8-byte LE.
- Enforce invariants both before and inside the AIR: selectors mutually exclusive, per-asset conservation, dsid/slot constants.
- `N_trace = 2^k` (`pow2_ceiling` of row count); `N_eval = N_trace * 2^b` where `b` is the blowup exponent.
- Provide fixtures and property tests:
  - Packing round-trips (`fastpq_prover/tests/packing.rs`, `tests/fixtures/packing_roundtrip.json`).
  - Ordering stability hash (`tests/fixtures/ordering_hash.json`).
  - Batch fixtures (`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`).

### AIR Column Schema
| Column Group      | Names                                                                                  | Description                                                                                                           |
| ----------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Activity          | `s_active`                                                                               | 1 for real rows, 0 for padding.                                                                                       |
| Main              | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]`                               | Packed Goldilocks elements (little-endian, 7-byte limbs).                                                             |
| Asset             | `asset_id_limbs[i]`                                                                      | Packed canonical asset identifier (little-endian, 7-byte limbs).                                                      |
| Selectors         | `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm` | 0/1. Constraint: Σ selectors (including `s_perm`) = `s_active`; `s_perm` mirrors role grant/revoke rows.              |
| Auxiliary         | `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`                        | State used for constraints, conservation, and audit trails.                                                           |
| SMT               | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf`                   | Per-level Poseidon2 inputs/outputs plus neighbour witness for non-membership.                                         |
| Lookup            | `perm_hash`                                                                              | Poseidon2 hash for permission lookup (constrained only when `s_perm = 1`).                                            |
| Metadata          | `dsid`, `slot`                                                                           | Constant across rows.                                                                                                 |

### Math & Constraints
- **Field packing:** bytes are chunked into 7-byte limbs (little-endian). Each limb `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`; reject limbs ≥ Goldilocks modulus.
- **Balance/conservation:** let `δ = value_new - value_old`. Group rows by `asset_id`. Define `r_asset_start = 1` at the first row of each asset group (0 otherwise) and constrain
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  On the last row of each asset group assert
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  Transfers satisfy the constraint automatically because their δ values sum to zero across the group. Example: if `value_old = 100` and `value_new = 120` on a mint row, δ = 20, so the mint sum contributes +20 and the final check resolves to zero when no burns occur.
- **Padding:** introduce `s_active`. Multiply all row constraints by `s_active` and enforce a contiguous prefix: `s_active[i] ≥ s_active[i+1]`. Padding rows (`s_active=0`) must keep constant values but are otherwise unconstrained.
- **Ordering hash:** Poseidon2 hash (domain `fastpq:v1:ordering`) over row encodings; stored in Public IO for auditability.

## Stage 2 — STARK Prover Core

### Objectives
- Build Poseidon2 Merkle commitments over trace and lookup evaluation vectors. Parameters: rate=2, capacity=1, full rounds=8, partial rounds=57, constants pinned to `ark-poseidon2` commit `3f2b7fe` (v0.3.0).
- Low-degree extension: evaluate each column on domain `D = { g^i | i = 0 .. N_eval-1 }`, where `N_eval = 2^{k+b}` divides the 2-adic capacity of Goldilocks. Let `g = ω^{(p-1)/N_eval}` with `ω` the fixed primitive root of Goldilocks and `p` its modulus; use the base subgroup (no coset). Record `g` in the transcript (tag `fastpq:v1:lde`).
- Composition polynomials: for each constraint `C_j`, form `F_j(X) = C_j(X) / Z_N(X)` with degree margins listed below.
- Lookup argument (permissions): sample `γ` from transcript. Trace product `Z_0 = 1`, `Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`. Table product `T = ∏_j (table_perm_j - γ)`. Boundary constraint: `Z_final / T = 1`.
- DEEP-FRI with arity `r ∈ {8, 16}`: for each layer, absorb the root with tag `fastpq:v1:fri_layer_ℓ`, sample `β_ℓ` (tag `fastpq:v1:beta_ℓ`), and fold via `H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k`.
- Proof object (Norito-encoded):
  ```
  Proof {
      protocol_version: u16,
      params_version: u16,
      parameter_set: String,
      public_io: PublicIO,
      trace_root: [u8; 32],
      lookup_root: [u8; 32],
      fri_layers: Vec<[u8; 32]>,
      alphas: Vec<Field>,
      betas: Vec<Field>,
      queries: Vec<QueryOpening>,
  }
  ```
- Verifier mirrors prover; run regression suite on 1k/5k/20k-row traces with golden transcripts.

### Degree Accounting
| Constraint | Degree before division | Degree after selectors | Margin vs `deg(Z_N)` |
|------------|------------------------|------------------------|----------------------|
| Transfer/mint/burn conservation | ≤1 | ≤1 | `deg(Z_N) - 2` |
| Role grant/revoke lookup | ≤2 | ≤2 | `deg(Z_N) - 3` |
| Metadata set | ≤1 | ≤1 | `deg(Z_N) - 2` |
| SMT hash (per level) | ≤3 | ≤3 | `deg(Z_N) - 4` |
| Lookup grand product | product relation | N/A | Boundary constraint |
| Boundary roots / supply totals | 0 | 0 | exact |

Padding rows are handled through `s_active`; dummy rows extend the trace to `N_trace` without violating constraints.

## Encoding & Transcript (Global)
- **Byte packing:** base-256 (7-byte limbs, little-endian). Tests in `fastpq_prover/tests/packing.rs`.
- **Field encoding:** canonical Goldilocks (little-endian 64-bit limb, reject ≥ p); Poseidon2 outputs/SMT roots serialized as 32-byte little-endian arrays.
- **Transcript (Fiat–Shamir):**
  1. BLAKE2b absorb `protocol_version`, `params_version`, `parameter_set`, `public_io`, and Poseidon2 commit tag (`fastpq:v1:init`).
  2. Absorb `trace_root`, `lookup_root` (`fastpq:v1:roots`).
  3. Derive lookup challenge `γ` (`fastpq:v1:gamma`).
  4. Derive composition challenges `α_j` (`fastpq:v1:alpha_j`).
  5. For each FRI layer root, absorb with `fastpq:v1:fri_layer_ℓ`, derive `β_ℓ` (`fastpq:v1:beta_ℓ`).
  6. Derive query indices (`fastpq:v1:query_index`).

  Tags are lowercase ASCII; verifiers reject mismatches before sampling challenges. Golden transcript fixture: `tests/fixtures/transcript_v1.json`.
- **Versioning:** `protocol_version = 1`, `params_version` matches `fastpq_isi` parameter set.

## Lookup Argument (Permissions)
- Committed table sorted lexicographically by `(role_id_bytes, permission_id_bytes, epoch_le)` and committed via Poseidon2 Merkle tree (`perm_root` in `PublicIO`).
- Trace witness uses `perm_hash` and selector `s_perm` (OR of role grant/revoke). The tuple is encoded as `role_id_bytes || permission_id_bytes || epoch_u64_le` with fixed widths (32, 32, 8 bytes).
- Product relation:
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  Boundary assertion: `Z_final / T = 1`. See `examples/lookup_grand_product.md` for a concrete accumulator walkthrough.

## Sparse Merkle Tree Constraints
- Define `SMT_HEIGHT` (number of levels). Columns `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` appear for all `ℓ ∈ [0, SMT_HEIGHT)`.
- Poseidon2 parameters pinned to `ark-poseidon2` commit `3f2b7fe` (v0.3.0); domain tag `fastpq:v1:poseidon_node`. All nodes use little-endian field encoding.
- Update rules per level:
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- Inserts set `(node_in_0 = 0, node_out_0 = value_new)`; deletes set `(node_in_0 = value_old, node_out_0 = 0)`.
- Non-membership proofs supply `neighbour_leaf` to show the queried interval is empty. See `examples/smt_update.md` for a worked example and JSON layout.
- Boundary constraint: the final hash equals `old_root` for pre rows and `new_root` for post rows.

## Soundness Parameters & SLOs
| N_trace | blowup | FRI arity | layers | queries | est bits | Proof size (≤) | RAM (≤) | P95 latency (≤) |
| ------- | ------ | --------- | ------ | ------- | -------- | --------------- | ------- | ---------------- |
| 2^15    | 8      | 8         | 5      | 52      | ~190     | 300 KB          | 1.5 GB  | 0.40 s (A100)    |
| 2^16    | 8      | 8         | 6      | 58      | ~132     | 420 KB          | 2.5 GB  | 0.75 s (A100)    |
| 2^17    | 16     | 16        | 5      | 64      | ~142     | 550 KB          | 3.5 GB  | 1.20 s (A100)    |

Derivations follow Appendix A. CI harness produces malformed proofs and fails if estimated bits <128.

## Public IO Schema
| Field            | Bytes | Encoding                              | Notes                               |
|-----------------|-------|---------------------------------------|-------------------------------------|
| `dsid`           | 16    | little-endian UUID                    | Dataspace ID for the entry's lane (global for default lane), hashed with tag `fastpq:v1:dsid`. |
| `slot`           | 8     | little-endian u64                     | Nanoseconds since epoch.            |
| `old_root`       | 32    | little-endian Poseidon2 field bytes   | SMT root before batch.              |
| `new_root`       | 32    | little-endian Poseidon2 field bytes   | SMT root after batch.               |
| `perm_root`      | 32    | little-endian Poseidon2 field bytes   | Permission table root for the slot. |
| `tx_set_hash`    | 32    | BLAKE2b                               | Sorted instruction identifiers.     |
| `parameter`      | var   | UTF-8 (e.g., `fastpq-lane-balanced`)  | Parameter set name.                 |
| `protocol_version`, `params_version` | 2 each | little-endian u16 | Version values.                      |
| `ordering_hash`  | 32    | Poseidon2 (little-endian)             | Stable hash of sorted rows.         |

Deletion is encoded by zero value limbs; absent keys use zero leaf + neighbour witness.

`FastpqTransitionBatch.public_inputs` is the canonical carrier for `dsid`, `slot`, and root commitments;
batch metadata is reserved for entry hash/transcript count bookkeeping.

## Encoding Hashes
- Ordering hash: Poseidon2 (tag `fastpq:v1:ordering`).
- Batch artifact hash: BLAKE2b over `PublicIO || proof.commitments` (tag `fastpq:v1:artifact`).

## Stage Definitions of Done (DoD)
- **Stage 1 DoD**
  - Packing round-trip tests and fixtures merged.
  - AIR spec (`docs/source/fastpq_air.md`) includes `s_active`, asset/SMT columns, selector definitions (including `s_perm`), and symbolic constraints.
  - Ordering hash recorded in PublicIO and verified via fixtures.
  - SMT/lookup witness generation implemented with membership & non-membership vectors.
  - Conservation tests cover transfer, mint, burn, and mixed batches.
- **Stage 2 DoD**
  - Transcript spec implemented; golden transcript (`tests/fixtures/transcript_v1.json`) and domain tags verified.
  - Poseidon2 parameter commit `3f2b7fe` pinned in prover and verifier with endianness tests across architectures.
  - Soundness CI guard active; proof size/RAM/latency SLOs recorded.
- **Stage 3 DoD**
  - Scheduler API (`SubmitProofRequest`, `ProofResult`) documented with idempotency keys.
  - Proof artifacts stored content-addressably with retry/backoff.
  - Telemetry exported for queue depth, queue wait time, prover execution latency, retry counts, backend failure counts, and GPU/CPU utilisation, with dashboards and alert thresholds for each metric.

## Stage 5 — GPU Acceleration & Optimisation
- Target kernels: LDE (NTT), Poseidon2 hashing, Merkle tree construction, FRI folding.
- Determinism: disable fast-math, ensure bit-identical outputs across CPU, CUDA, Metal. CI must compare proof roots across devices.
- Benchmark suite comparing CPU vs GPU on reference hardware (e.g., Nvidia A100, AMD MI210).
- Metal backend (Apple Silicon):
  - Build script compiles the kernel suite (`metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon2.metal`) into `fastpq.metallib` via `xcrun metal`/`xcrun metallib`; ensure the macOS developer tools include the Metal toolchain (`xcode-select --install`, then `xcodebuild -downloadComponent MetalToolchain` if required).【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - Manual rebuild (mirrors `build.rs`) for CI warm-ups or deterministic packaging:
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    Successful builds emit `FASTPQ_METAL_LIB=<path>` so the runtime can load the metallib deterministically.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - The LDE kernel now assumes the evaluation buffer is zero-initialised on the host. Keep the existing `vec![0; ..]` allocation path or explicitly zero buffers when reusing them.【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141】
  - Coset multiplication is fused into the final FFT stage to avoid an extra pass; any changes to LDE staging must preserve that invariant.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - The shared-memory FFT/LDE kernel now stops at the tile depth and hands the remaining butterflies plus any inverse scaling to a dedicated `fastpq_fft_post_tiling` pass. The Rust host threads the same column batches through both kernels and only launches the post-tile dispatch when `log_len` exceeds the tile limit, so queue-depth telemetry, kernel stats, and fallback behaviour stay deterministic while the GPU handles the wide-stage work entirely on-device.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - To experiment with launch shapes, set `FASTPQ_METAL_THREADGROUP=<width>`; the dispatch path clamps the value to the device limit and logs the override so profiling runs can sweep threadgroup sizes without recompiling.【crates/fastpq_prover/src/metal.rs:321】
  - Tune the FFT tile directly: the host now derives threadgroup lanes (16 for short traces, 32 once `log_len ≥ 6`, 64 once `log_len ≥ 10`, 128 once `log_len ≥ 14`, and 256 at `log_len ≥ 18`) and tile depth (5 stages for small traces, 4 when `log_len ≥ 12`, and once the domain reaches `log_len ≥ 18/20/22` the shared-memory pass now runs 12/14/16 stages before handing control to the post-tile kernel) from the requested domain plus the device’s execution width/max threads. Override with `FASTPQ_METAL_FFT_LANES` (power of two between 8 and 256) and `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) to pin specific launch shapes; both values flow through `FftArgs`, get clamped to the supported window, and are logged for profiling sweeps.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:120】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:244】
- FFT/IFFT and LDE column batching now derive from the resolved threadgroup width: the host targets roughly 4 096 logical threads per command buffer, fuses up to 64 columns at a time with the circular-buffer tile staging, and only ratchets down through 64 → 32 → 16 → 8 → 4 → 2 → 1 columns as the evaluation domain crosses the 2¹⁶/2¹⁸/2²⁰/2²² thresholds. This keeps the 20 k-row capture at ≥64 columns per dispatch while ensuring long cosets still finish deterministically. The adaptive scheduler still doubles column width until dispatches approach the ≈2 ms target and now halves the batch automatically whenever a sampled dispatch lands ≥30 % over that target, so lane/tile transitions that inflate per-column cost fall back without manual overrides. Poseidon permutations share the same adaptive scheduler and the `metal_heuristics.batch_columns.poseidon` block in `fastpq_metal_bench` now records the resolved state count, cap, last duration, and override flag so queue-depth telemetry can be tied directly to Poseidon tuning. Override with `FASTPQ_METAL_FFT_COLUMNS` (1–64) to pin a deterministic FFT batch size, and use `FASTPQ_METAL_LDE_COLUMNS` (1–64) when you need the LDE dispatcher to honour a fixed column count; the Metal bench surfaces the resolved `kernel_profiles.*.columns` entries in every capture so tuning experiments stay reproducible.【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1284】
- Multi-queue dispatch is now automatic on discrete Macs: the host inspects `is_low_power`, `is_headless`, and the device location to decide whether to spin up two Metal command queues, only fans out when the workload carries at least 16 columns (scaled by the resolved fan-out), and round-robins the column batches so long traces keep both GPU lanes busy without sacrificing determinism. The command-buffer semaphore now enforces a “two in flight per queue” floor, and queue telemetry records the aggregate measurement window (`window_ms`) plus normalized busy ratios (`busy_ratio`) for the global semaphore and every queue entry so release artefacts can prove both queues stayed ≥50 % busy over the same time span. Override the defaults with `FASTPQ_METAL_QUEUE_FANOUT` (1–4 lanes) and `FASTPQ_METAL_COLUMN_THRESHOLD` (minimum total columns before fan-out); the Metal parity tests force the overrides so multi-GPU Macs stay covered, and the resolved policy is logged alongside the queue-depth telemetry and the new `metal_dispatch_queue.queues[*]` block.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- Metal detection now probes `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` directly (warming up CoreGraphics on headless shells) before falling back to `system_profiler`, and `FASTPQ_DEBUG_METAL_ENUM` prints the enumerated devices when set so headless CI runs can explain why `FASTPQ_GPU=gpu` still downgraded to the CPU path. When the override is set to `gpu` but no accelerator is detected, `fastpq_metal_bench` now errors immediately with a pointer to the debug knob instead of silently continuing on the CPU. This narrows the “silent CPU fallback” class called out in WP2‑E and gives operators a knob to capture enumeration logs inside wrapped benchmarks.【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/backend.rs:705】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
  - Poseidon GPU timings now refuse to treat CPU fallbacks as “GPU” data. `hash_columns_gpu` reports whether the accelerator actually ran, `measure_poseidon_gpu` drops samples (and logs a warning) whenever the pipeline falls back, and the Poseidon microbench child exits with an error if GPU hashing is unavailable. As a result, `gpu_recorded=false` whenever Metal execution falls back, the queue summary still records the failed dispatch window, and dashboard summaries immediately flag the regression. The wrapper (`scripts/fastpq/wrap_benchmark.py`) now fails when `metal_dispatch_queue.poseidon.dispatch_count == 0` so Stage 7 bundles can’t be signed without real GPU Poseidon dispatch evidence.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_benchmark.py:912】
- Poseidon hashing now mirrors that staging contract. `PoseidonColumnBatch` produces flattened payload buffers plus offset/length descriptors, the host rebases those descriptors per batch and runs a `COLUMN_STAGING_PIPE_DEPTH` double buffer so payload + descriptor uploads overlap with GPU work, and both Metal/CUDA kernels consume the descriptors directly so each dispatch absorbs all padded rate blocks on-device before emitting the column digests. `hash_columns_from_coefficients` now streams those batches through a GPU worker thread, keeping 64+ columns in flight by default on discrete GPUs (tunable via `FASTPQ_POSEIDON_PIPE_COLUMNS` / `FASTPQ_POSEIDON_PIPE_DEPTH`). The Metal bench records the resolved pipeline settings + batch counts under `metal_dispatch_queue.poseidon_pipeline`, and `kernel_profiles.poseidon.bytes` includes the descriptor traffic so Stage 7 captures prove the new ABI end-to-end.【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1963】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【crates/fastpq_prover/src/metal.rs:2290】【crates/fastpq_prover/cuda/fastpq_cuda.cu:351】
- Stage7-P2 fused Poseidon hashing now lands in both GPU backends. The streaming worker feeds contiguous `PoseidonColumnBatch::column_window()` slices into `hash_columns_gpu_fused`, which pipes them to `poseidon_hash_columns_fused` so each dispatch writes `leaf_digests || parent_digests` with the canonical `(⌈columns / 2⌉)` parent mapping. `ColumnDigests` keeps both slices, and `merkle_root_with_first_level` consumes the parent layer immediately, so the CPU never recomputes depth‑1 nodes and Stage7 telemetry can assert that GPU captures report zero “fallback” parents whenever the fused kernel succeeds.【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】【crates/fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq_cuda.cu:631】
- `fastpq_metal_bench` now emits a `device_profile` block with the Metal device name, registry id, `low_power`/`headless` flags, location (built-in, slot, external), discrete indicator, `hw.model`, and the derived Apple SoC label (for example, “M3 Max”). Stage 7 dashboards consume this field to bucket captures by M4/M3 vs discrete GPUs without parsing hostnames, and the JSON ships next to the queue/heuristic evidence so every release artefact proves which fleet class produced the run.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2536】
  - FFT host/device overlap now uses a double-buffered staging window: while batch *n* finishes inside `fastpq_fft_post_tiling`, the host flattens batch *n + 1* into the second staging buffer and only pauses when a buffer must be recycled. The backend records how many batches were flattened plus the time spent flattening versus waiting for GPU completion, and `fastpq_metal_bench` surfaces the aggregated `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` block so release artefacts can prove the overlap instead of silent host stalls. The JSON report now also breaks the totals down per phase under `column_staging.phases.{fft,lde,poseidon}`, letting Stage 7 captures prove whether FFT/LDE/Poseidon staging is host-bound or waiting on GPU completion. Poseidon permutations reuse the same pooled staging buffers, so `--operation poseidon_hash_columns` captures now emit the Poseidon-specific `column_staging` deltas alongside the queue-depth evidence without bespoke instrumentation. The new `column_staging.samples.{fft,lde,poseidon}` arrays record the per-batch `batch/flatten_ms/wait_ms/wait_ratio` tuples, making it trivial to prove that the `COLUMN_STAGING_PIPE_DEPTH` overlap is holding (or to spot when the host starts waiting for GPU completions).【crates/fastpq_prover/src/metal.rs:319】【crates/fastpq_prover/src/metal.rs:330】【crates/fastpq_prover/src/metal.rs:1813】【crates/fastpq_prover/src/metal.rs:2488】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1216】
- Poseidon2 acceleration now runs as a high-occupancy Metal kernel: each threadgroup copies the round constants and MDS rows into threadgroup memory, unrolls the full/partial rounds, and walks multiple states per lane so every dispatch launches at least 4 096 logical threads. Override the launch shape via `FASTPQ_METAL_POSEIDON_LANES` (powers of two between 32 and 256, clamped to the device limit) and `FASTPQ_METAL_POSEIDON_BATCH` (1–32 states per lane) to reproduce profiling experiments without rebuilding `fastpq.metallib`; the Rust host threads the resolved tuning through `PoseidonArgs` before dispatching. The host now snapshots `MTLDevice::{is_low_power,is_headless,location}` once per boot and automatically biases discrete GPUs toward VRAM-tiered launches (`256×24` on ≥48 GiB parts, `256×20` at 32 GiB, `256×16` otherwise) while low-power SoCs stick to `256×8` (fallbacks for 128/64 lane hardware continue to use 8/6 states per lane), so operators get the >16-state pipeline depth without touching env vars. `fastpq_metal_bench` re-executes itself under `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` to capture a dedicated `poseidon_microbench` block comparing the scalar lane against the multi-state kernel so release artefacts can quote a concrete speedup. The same captures surface `poseidon_pipeline` telemetry (`chunk_columns`, `pipe_depth`, `batches`, `fallbacks`) so Stage 7 evidence proves the overlap window on every GPU trace.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  - LDE tile staging now mirrors the FFT heuristics: heavy traces only execute 12 stages in the shared-memory pass once `log₂(len) ≥ 18`, drop to 10 stages at log₂ 20, and clamp to eight stages at log₂ 22 so the wide butterflies move into the post-tiling kernel. Override with `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) whenever you need a deterministic depth; the host only launches the post-tiling dispatch when the heuristic stops early so queue-depth and kernel telemetry stay deterministic.【crates/fastpq_prover/src/metal.rs:827】
  - Kernel micro-optimisation: the shared-memory FFT/LDE tiles now reuse per-lane twiddle and coset strides instead of re-evaluating `pow_mod*` for every butterfly. Each lane precomputes `w_seed`, `w_stride`, and (when required) the coset stride once per block, then streams through the offsets, slashing the scalar multiplications inside `apply_stage_tile`/`apply_stage_global` and bringing the 20 k-row LDE mean down to ~1.55 s with the latest heuristics (still above the 950 ms goal, but a further ~50 ms improvement over the batching-only tweak).【crates/fastpq_prover/metal/kernels/ntt_stage.metal:164】【fastpq_metal_bench_run11.json:1】
  - The kernel suite now has a dedicated reference (`docs/source/fastpq_metal_kernels.md`) that documents each entry point, the threadgroup/tile limits enforced in `fastpq.metallib`, and the reproduction steps for compiling the metallib manually.【docs/source/fastpq_metal_kernels.md:1】
  - The benchmark report now emits a `post_tile_dispatches` object that records how many FFT/IFFT/LDE batches ran in the dedicated post-tiling kernel (per-kind dispatch counts plus the stage/log₂ boundaries). `scripts/fastpq/wrap_benchmark.py` copies the block into `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary`, and the manifest gate refuses GPU captures that omit the evidence so every 20 k-row artefact proves the multi-pass kernel ran on-device.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - Set `FASTPQ_METAL_TRACE=1` to emit per-dispatch debug logs (pipeline label, threadgroup width, launch groups, elapsed time) for Instruments/Metal trace correlation.【crates/fastpq_prover/src/metal.rs:346】
- The dispatch queue is now instrumented: `FASTPQ_METAL_MAX_IN_FLIGHT` caps concurrent Metal command buffers (auto default derived from the detected GPU core count via `system_profiler`, clamped to at least the queue fan-out floor with a host-parallelism fallback when macOS refuses to report the device). The bench enables queue-depth sampling so the exported JSON carries a `metal_dispatch_queue` object with `limit`, `dispatch_count`, `max_in_flight`, `busy_ms`, and `overlap_ms` fields for release evidence, adds a nested `metal_dispatch_queue.poseidon` block whenever a Poseidon-only capture (`--operation poseidon_hash_columns`) runs, and emits a `metal_heuristics` block describing the resolved command-buffer limit plus the FFT/LDE batch columns (including whether overrides forced the values) so reviewers can audit the scheduling decisions alongside the telemetry. Poseidon kernels also feed a dedicated `poseidon_profiles` block distilled from the kernel samples so bytes/thread, occupancy, and dispatch geometry are tracked across artefacts. If the primary run can’t collect queue depth or the LDE zero-fill stats (for example, when a GPU dispatch silently falls back to the CPU), the harness automatically fires a single probe dispatch to gather the missing telemetry and now synthesizes host zero-fill timings when the GPU refuses to report them, so published evidence always includes the `zero_fill` block.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - Set `FASTPQ_SKIP_GPU_BUILD=1` when cross-compiling without the Metal toolchain; the warning records the skip and the planner continues on the CPU path.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
  - Runtime detection uses `system_profiler` to confirm Metal support; if the framework, device, or metallib is missing the build script clears `FASTPQ_METAL_LIB` and the planner stays on the deterministic CPU path.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src/metal.rs:43】
  - Operator checklist (Metal hosts):
    1. Confirm the toolchain is present and that `FASTPQ_METAL_LIB` points at a compiled `.metallib` (`echo $FASTPQ_METAL_LIB` should be non-empty after `cargo build --features fastpq-gpu`).【crates/fastpq_prover/build.rs:188】
    2. Run parity tests with GPU lanes enabled: `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`. This exercises the Metal kernels and falls back automatically if detection fails.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. Capture a benchmark sample for dashboards: locate the compiled Metal library
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`), export it via
       `FASTPQ_METAL_LIB`, and run
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
       The canonical `fastpq-lane-balanced` set now pads every capture to 32,768 rows, so the
       JSON reflects both the requested 20 k rows and the padded domain that drives the GPU
       kernels. Upload the JSON/log to your evidence store; the nightly macOS workflow mirrors
      this run and archives the artefacts for reference. The report records
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` alongside each operation’s `speedup`, the
     LDE section adds `zero_fill.{bytes,ms,queue_delta}` so release artefacts prove determinism,
     host zero-fill overhead, and the incremental GPU queue usage (limit, dispatch count,
     peak in-flight, busy/overlap time), and the new `kernel_profiles` block captures per-kernel
     occupancy ratios, estimated bandwidth, and duration ranges so dashboards can flag GPU
       regressions without reprocessing raw samples.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       Expect the Metal LDE path to stay under 950 ms (`<1 s` target on Apple M-series hardware);
4. Capture row-usage telemetry from a real ExecWitness so dashboards can chart transfer gadget
   adoption. Fetch a witness from Torii
  (`iroha_cli audit witness --binary --out exec.witness`) and decode it with
  `iroha_cli audit witness --decode exec.witness` (optionally add
  `--fastpq-parameter fastpq-lane-balanced` to assert the expected parameter set; FASTPQ batches
  emit by default; pass `--no-fastpq-batches` only if you need to trim the output).
   Every batch entry now emits a `row_usage` object (`total_rows`, `transfer_rows`,
   `non_transfer_rows`, per-selector counts, and `transfer_ratio`). Archive that JSON snippet
   reprocessing raw transcripts.【crates/iroha_cli/src/audit.rs:209】 Compare the new capture against
   the previous baseline with `scripts/fastpq/check_row_usage.py` so CI fails if transfer ratios or
   total rows regress:

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```

   Sample JSON blobs for smoke tests live in `scripts/fastpq/examples/`. Locally you can run `make check-fastpq-row-usage`
   (wraps `ci/check_fastpq_row_usage.sh`), and CI runs the same script via `.github/workflows/fastpq-row-usage.yml` to compare the committed
   `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` snapshots so the evidence bundle fails fast whenever
   transfer rows creep back up. Pass `--summary-out <path>` if you want a machine-readable diff (the CI job uploads `fastpq_row_usage_summary.json`).
   When an ExecWitness isn’t handy, synthesize a regression sample with `fastpq_row_bench`
   (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), which emits the exact same `row_usage`
   object for configurable selector counts (e.g., a 65 536 row stress test):

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```

   Stage 7-3 rollout bundles must also pass `scripts/fastpq/validate_row_usage_snapshot.py`, which
   enforces that every `row_usage` entry contains the selector counts and that
   `transfer_ratio = transfer_rows / total_rows`; `ci/check_fastpq_rollout.sh` calls the helper
   automatically so bundles missing those invariants fail before GPU lanes are mandated.【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       the bench manifest gate enforces this via `--max-operation-ms lde=950`, so refresh the
       capture whenever your evidence exceeds that bound.
      When you also need Instruments evidence, pass `--trace-dir <dir>` so the harness
      relaunches itself via `xcrun xctrace record` (default “Metal System Trace” template) and
      stores a timestamped `.trace` file alongside the JSON; you can still override the location /
      template manually with `--trace-output <path>` plus optional `--trace-template` /
      `--trace-seconds`. The resulting JSON advertises `metal_trace_{template,seconds,output}` so
      artefact bundles always identify the captured trace.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      Wrap each capture with
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       (add `--gpg-key <fingerprint>` if you need to pin a signing identity) so the bundle fails
       fast whenever the GPU LDE mean breaches the 950 ms target, Poseidon exceeds 1 s, or the
       Poseidon telemetry blocks are missing, embeds a `row_usage_snapshot`
      next to the JSON, surfaces the Poseidon microbench summary under `benchmarks.poseidon_microbench`,
      and still carries metadata for runbooks and the Grafana dashboard
    (`dashboards/grafana/fastpq_acceleration.json`). The JSON now emits `speedup.ratio` /
     `speedup.delta_ms` per operation so release evidence can prove GPU vs
     CPU gains without reprocessing the raw samples, and the wrapper copies both the
     zero-fill statistics (plus `queue_delta`) into `zero_fill_hotspots` (bytes, latency, derived
     GB/s), records the Instruments metadata under `metadata.metal_trace`, threads the optional
     `metadata.row_usage_snapshot` block when `--row-usage <decoded witness>` is supplied, and flattens the
     per-kernel counters into `benchmarks.kernel_summary` so padding bottlenecks, Metal queue
     utilisation, kernel occupancy, and bandwidth regressions are visible at a glance without
     spelunking the raw report.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【scripts/fastpq/wrap_benchmark.py:1】【artifacts/fastpq_benchmarks/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】
     Because the row-usage snapshot now travels with the wrapped artefact, rollout tickets simply
     reference the bundle instead of attaching a second JSON snippet, and CI can diff the embedded
    counts directly when validating Stage 7 submissions. To archive the microbench data on its own,
    run `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json`
    and store the resulting file under `benchmarks/poseidon/`. Keep the aggregated manifest fresh with
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    so dashboards/CI can diff the full history without walking each file manually.
    4. Validate telemetry by curling `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` (Prometheus endpoint) or looking for `telemetry::fastpq.execution_mode` logs; unexpected `resolved="cpu"` entries indicate the host fell back despite GPU intent.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. Use `FASTPQ_GPU=cpu` (or the config knob) to force CPU execution during maintenance and confirm fallback logs still appear; this keeps SRE runbooks aligned with the deterministic path.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- Telemetry & fallback:
  - Execution-mode logs (`telemetry::fastpq.execution_mode`) and counters (`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`) expose the requested vs resolved mode so silent fallbacks are visible in dashboards.【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - The `FASTPQ Acceleration Overview` Grafana board (`dashboards/grafana/fastpq_acceleration.json`) visualises the Metal adoption rate and links back to the benchmark artefacts, while the paired alert rules (`dashboards/alerts/fastpq_acceleration_rules.yml`) gate rollouts on sustained downgrades.
  - `FASTPQ_GPU={auto,cpu,gpu}` overrides remain supported; unknown values raise warnings but still propagate to telemetry for auditing.【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - GPU parity tests (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`) must pass for CUDA and Metal; CI skips gracefully when the metallib is absent or detection fails.【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
  - Metal readiness evidence (archive the artefacts below with every rollout so the roadmap audit can prove determinism, telemetry coverage, and fallback behaviour):

    | Step | Goal | Command / Evidence |
    | ---- | ---- | ------------------ |
    | Build metallib | Ensure `xcrun metal`/`xcrun metallib` are available and emit the deterministic `.metallib` for this commit | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`; `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`; `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`; export `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | Verify env var | Confirm Metal stays enabled by checking the env var recorded by the build script | `echo $FASTPQ_METAL_LIB` (must return an absolute path; empty means the backend was disabled).【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | GPU parity suite | Prove kernels execute (or emit deterministic downgrade logs) before shipping | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` and store the resulting log snippet that shows either `backend="metal"` or the fallback warning.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
    | Benchmark sample | Capture the JSON/log pair that records `speedup.*` and FFT tuning so dashboards can ingest accelerator evidence | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; archive the JSON, the timestamped `.trace`, and stdout alongside release notes so the Grafana board picks up the Metal run (the report records the requested 20 k rows plus the padded 32,768-row domain so reviewers can confirm the `<1 s` LDE target).【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | Wrap & sign report | Fail the release if the GPU LDE mean breaches 950 ms, Poseidon exceeds 1 s, or Poseidon telemetry blocks are missing, and produce a signed artefact bundle | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`; ship both the wrapped JSON and the generated `.json.asc` signature so auditors can verify the sub-second metrics without rerunning the workload.【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
    | Signed bench manifest | Enforce `<1 s` LDE evidence across Metal/CUDA bundles and capture signed digests for release approval | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`; attach the manifest + signature to the release ticket so downstream automation can validate the sub-second proof metrics.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
| CUDA bundle | Keep the SM80 CUDA capture in lock-step with the Metal evidence so manifests cover both GPU classes. | `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` on the Xeon + RTX host → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output`; append the wrapped path to `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`, keep the `.json`/`.asc` pair next to the Metal bundle, and cite the seeded `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` when auditors need a reference layout.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
| Telemetry check | Validate Prometheus/OTEL surfaces reflect `device_class="<matrix>", backend="metal"` (or log the downgrade) | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` and copy the `telemetry::fastpq.execution_mode` log emitted at startup.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    | Forced fallback drill | Document the deterministic CPU path for SRE playbooks | Run a short workload with `FASTPQ_GPU=cpu` or `zk.fastpq.execution_mode = "cpu"` and capture the downgrade log so operators can rehearse the rollback procedure.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
    | Trace capture (optional) | When profiling, capture dispatch traces so kernel lane/tile overrides are reviewable later | Rerun one parity test with `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` and attach the produced trace log to your release artefacts.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    Archive the evidence with the release ticket and mirror the same checklist in `docs/source/fastpq_migration_guide.md` so staging/prod rollouts follow an identical playbook.【docs/source/fastpq_migration_guide.md:1】

### Release checklist enforcement

Add the following gates to every FASTPQ release ticket. Releases are blocked until all items are
complete and attached as signed artefacts.

1. **Sub-second proof metrics** — The canonical Metal benchmark capture
   (`fastpq_metal_bench_*.json`) must prove the 20 000-row workload (32 768 padded rows) finishes in
   <1 s. Concretely, the `benchmarks.operations` entry where `operation = "lde"` and the matching
   `report.operations` sample must show `gpu_mean_ms ≤ 950`. Runs that exceed the ceiling require
   investigation and a recapture before the checklist can be signed.
2. **Signed benchmark manifest** — After recording fresh Metal + CUDA bundles, run
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` to emit
   `artifacts/fastpq_bench_manifest.json` and the detached signature
   (`artifacts/fastpq_bench_manifest.sig`). Attach both files plus the public key fingerprint to the
   release ticket so reviewers can verify the digest and signature independently.【xtask/src/fastpq.rs:1】
3. **Evidence attachments** — Store the raw benchmark JSON, stdout log (or Instruments trace, when
   captured), and the manifest/signature pair with the release ticket. The checklist is only
   considered green when the ticket links to those artefacts and the on-call reviewer confirms the
   digest recorded in `fastpq_bench_manifest.json` matches the uploaded files.【artifacts/fastpq_benchmarks/README.md:1】

## Stage 6 — Hardening & Documentation
- Placeholder backend retired; production pipeline ships by default with no feature toggles.
- Reproducible builds (pin toolchains, container images).
- Fuzzers for trace, SMT, lookup structures.
- Prover-level smoke tests cover governance ballot grants and remittance transfers to keep Stage 6 fixtures stable ahead of full IVM rollouts.【crates/fastpq_prover/tests/realistic_flows.rs:1】
- Runbooks with alert thresholds, remediation procedures, capacity planning guidelines.
- Cross-architecture proof replay (x86_64, ARM64) in CI.

### Bench manifest & release gate

Release evidence now includes a deterministic manifest covering both Metal and
CUDA benchmark bundles. Run:

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```

The command validates the wrapped bundles, enforces latency/speedup thresholds,
emits BLAKE3 + SHA-256 digests, and (optionally) signs the manifest with an
Ed25519 key so release tooling can verify provenance. See
`xtask/src/fastpq.rs`/`xtask/src/main.rs` for the implementation and
`artifacts/fastpq_benchmarks/README.md` for operational guidance.

> **Note:** Metal bundles that omit `benchmarks.poseidon_microbench` now cause
> the manifest generation to fail. Re-run `scripts/fastpq/wrap_benchmark.py`
> (and `scripts/fastpq/export_poseidon_microbench.py` if you need a standalone
> summary) whenever the Poseidon evidence is missing so release manifests
> always capture the scalar-vs-default comparison.【xtask/src/fastpq.rs:409】

The `--matrix` flag (defaulting to `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
when present) loads the cross-device medians captured by
`scripts/fastpq/capture_matrix.sh`. The manifest encodes the 20 000-row floor and
per-operation latency/speedup limits for every device class, so bespoke
`--require-rows`/`--max-operation-ms`/`--min-operation-speedup` overrides are no
longer required unless you are debugging a specific regression.

Refresh the matrix by appending wrapped benchmark paths to the
`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` lists and running
`scripts/fastpq/capture_matrix.sh`. The script snapshots the per-device medians,
emits the consolidated `matrix_manifest.json`, and prints the relative path that
`cargo xtask fastpq-bench-manifest` will consume. The Apple M4, Xeon + RTX, and
Neoverse + MI300 capture lists (`devices/apple-m4-metal.txt`,
`devices/xeon-rtx-sm80.txt`, `devices/neoverse-mi300.txt`) plus their wrapped
benchmark bundles
(`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`,
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`,
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) are now checked
in, so every release enforces the same cross-device medians before the manifest
is signed.【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【artifacts/fastpq_benchmarks/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】【artifacts/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json:1】【artifacts/fastpq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## Critique Summary & Open Actions

## Stage 7 — Fleet Adoption & Rollout Evidence

Stage 7 takes the prover from “documented & benchmarked” (Stage 6) to
“default-ready for production fleets”. The focus is on telemetry ingestion,
cross-device capture parity, and operator evidence bundles so GPU acceleration
can be mandated deterministically.

- **Stage7-1 — Fleet telemetry ingestion & SLOs.** Production dashboards
  (`dashboards/grafana/fastpq_acceleration.json`) must be wired to live
  Prometheus/OTel feeds with Alertmanager coverage for queue-depth stalls,
  zero-fill regressions, and silent CPU fallbacks. The alert pack stays under
  `dashboards/alerts/fastpq_acceleration_rules.yml` and feeds the same evidence
  bundle required in Stage 6.【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  The dashboard now exposes template variables for `device_class`, `chip_family`,
  and `gpu_kind`, letting operators pivot Metal adoption by the exact matrix
  label (e.g., `apple-m4-max`), by Apple chip family, or by discrete vs.
  integrated GPU classes without editing the queries.
  macOS nodes built with `irohad --features fastpq-gpu` now emit
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`,
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  (busy/overlap ratios), and
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  (limit, max_in_flight, dispatch_count, window_seconds) so the dashboards and
  Alertmanager rules can read Metal semaphore duty-cycle/headroom directly from
  Prometheus without waiting for a benchmark bundle. Hosts now export
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` and
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` whenever
  the LDE helper zeros GPU evaluation buffers, and Alertmanager gained the
  `FastpqQueueHeadroomLow` (headroom < 1 for 10 m) and
  `FastpqZeroFillRegression` (>0.40 ms over 15 m) rules so queue headroom and
  zero-fill regressions page operators immediately instead of waiting for the
  next wrapped benchmark. A new `FastpqCpuFallbackBurst` page-level alert tracks
  GPU requests that land on the CPU backend for more than 5 % of the workload,
  forcing operators to capture evidence and root-cause transient GPU failures
  before retrying the rollout.【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
  The SLO set now also enforces the ≥50 % Metal duty-cycle target via the
  `FastpqQueueDutyCycleDrop` rule, which averages
  `fastpq_metal_queue_ratio{metric="busy"}` over a rolling 15-minute window and
  warns whenever GPU work is still being scheduled but a queue fails to keep the
  required occupancy. This keeps the live telemetry contract aligned with the
  benchmark evidence before GPU lanes are mandated.【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
- **Stage7-2 — Cross-device capture matrix.** The new
  `scripts/fastpq/capture_matrix.sh` builds
  `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json` from the per-device
  capture lists under `artifacts/fastpq_benchmarks/matrix/devices/`. Apple M4,
  Xeon + RTX, and Neoverse + MI300 medians now live in-repo alongside their
  wrapped bundles, so `cargo xtask fastpq-bench-manifest` loads the manifest
  automatically, enforces the 20 000-row floor, and applies per-device
  latency/speedup limits without bespoke CLI flags before a release bundle is
  approved.【scripts/fastpq/capture_matrix.sh:1】【artifacts/fastpq_benchmarks/matrix/matrix_manifest.json:1】【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【xtask/src/fastpq.rs:1】
Aggregated instability reasons now ship alongside the matrix: pass
`--reason-summary-out` to `scripts/fastpq/geometry_matrix.py` to emit a
JSON histogram of failure/warning causes keyed by host label and source
summary, so Stage7-2 reviewers can see CPU fallbacks or missing telemetry at
a glance without scanning the full Markdown table. The same helper now
accepts `--host-label chip_family:Chip` (repeat for multiple keys) so the
Markdown/JSON outputs include curated host label columns instead of burying
that metadata in the raw summary, making it trivial to filter OS builds or
Metal driver versions when compiling the Stage7-2 evidence bundle.【scripts/fastpq/geometry_matrix.py:1】
Geometry sweeps also stamp ISO8601 `started_at` / `completed_at` fields into the
summary, CSV, and Markdown outputs so capture bundles can prove the window for
each host when Stage7-2 matrices merge multiple lab runs.【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` now stitches the geometry matrix together with
`row_usage/*.json` snapshots into a single Stage7 bundle (`stage7_bundle.json`
+ `stage7_geometry.md`), validating transfer ratios via
`validate_row_usage_snapshot.py` and persisting host/env/reason/source summaries
so rollout tickets can attach one deterministic artefact instead of juggling
per-host tables.【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **Stage7-3 — Operator adoption evidence & rollback drills.** The new
  `docs/source/fastpq_rollout_playbook.md` describes the artefact bundle
  (`fastpq_bench_manifest.json`, wrapped Metal/CUDA captures, Grafana export,
  Alertmanager snapshot, rollback logs) that must accompany every rollout ticket
  plus the staged (pilot → ramp → default) timeline and forced fallback drills.
  `ci/check_fastpq_rollout.sh` validates these bundles so CI enforces the Stage 7
  gate before releases move forward. The release pipeline can now pull the same
  bundles into `artifacts/releases/<version>/fastpq_rollouts/…` via
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`, ensuring the
  signed manifests and rollout evidence stay together. A reference bundle lives
  under `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/` to keep
  the GitHub workflow (`.github/workflows/fastpq-rollout.yml`) green while real
  rollout submissions are reviewed.

### Stage7 FFT queue fan-out

`crates/fastpq_prover/src/metal.rs` now instantiates a `QueuePolicy` that
automatically spawns multiple Metal command queues whenever the host reports a
discrete GPU. Integrated GPUs keep the single-queue path
(`MIN_QUEUE_FANOUT = 1`), while discrete devices default to two queues and only
fan out when a workload covers at least 16 columns. Both heuristics can be tuned
via the new `FASTPQ_METAL_QUEUE_FANOUT` and `FASTPQ_METAL_COLUMN_THRESHOLD`
environment variables, and the scheduler round-robins FFT/LDE batches across the
active queues before issuing the paired post-tiling dispatch on the same queue
to preserve ordering guarantees.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】
Node operators no longer need to export those env vars manually: the
`iroha_config` profile exposes `fastpq.metal_queue_fanout` and
`fastpq.metal_queue_column_threshold`, and `irohad` applies them via
`fastpq_prover::set_metal_queue_policy` before the Metal backend initialises so
fleet profiles stay reproducible without bespoke launch wrappers.【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
Inverse FFT batches now stick to a single queue whenever the workload only just
hits the fan-out threshold (e.g., the 16-column lane-balanced capture), which
restores ≥1.0× parity for WP2-D while leaving large-column FFT/LDE/Poseidon
dispatches on the multi-queue path.【crates/fastpq_prover/src/metal.rs:2018】

Helper tests exercise the queue-policy clamps and parser validation so CI can
prove the Stage 7 heuristics without requiring GPU hardware on every builder,
and the GPU-specific tests force fan-out overrides to keep replay coverage in
sync with the new defaults.【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Stage7-1 Device Labels & Alert Contract

`scripts/fastpq/wrap_benchmark.py` now probes `system_profiler` on macOS capture
hosts and records hardware labels in every wrapped benchmark so Fleet telemetry
and the capture matrix can pivot by device without bespoke spreadsheets. A
20 000-row Metal capture now carries entries such as:

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

These labels are ingested along with `benchmarks.zero_fill_hotspots` and
`benchmarks.metal_dispatch_queue` so the Grafana snapshot, capture matrix
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`), and Alertmanager
evidence all agree on the hardware class that produced the metrics. The
`--label` flag still allows manual overrides when a lab host lacks
`system_profiler`, but the auto-probed identifiers now cover Apple M1–M4 and
discrete PCIe GPUs out of the box.【scripts/fastpq/wrap_benchmark.py:1】

Linux captures receive the same treatment: `wrap_benchmark.py` now inspects
`/proc/cpuinfo`, `nvidia-smi`/`rocm-smi`, and `lspci` so CUDA and OpenCL runs
derive `cpu_model`, `gpu_model`, and a canonical `device_class` (`xeon-rtx-sm80`
for the Stage 7 CUDA host, `neoverse-mi300` for the MI300A lab). Operators can
still override the auto-detected values, but Stage 7 evidence bundles no longer
require manual edits to tag Xeon/Neoverse captures with the correct device
metadata.

At runtime, each host sets `fastpq.device_class`, `fastpq.chip_family`, and
`fastpq.gpu_kind` (or the corresponding `FASTPQ_*` environment variables) to the
same matrix labels that appear in the capture bundle so Prometheus export
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` and
the FASTPQ Acceleration dashboard can filter by any of the three axes. The
Alertmanager rules aggregate over the same label set, letting operators chart
adoption, downgrades, and fallbacks per hardware profile instead of a single
fleet-wide ratio.【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

The telemetry SLO/alert contract now ties the captured metrics back to the Stage 7
gates. The table below summarises the signals and enforcement points:

| Signal | Source | Target / Trigger | Enforcement |
| ------ | ------ | ---------------- | ----------- |
| GPU adoption ratio | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | ≥95 % of per-(device_class, chip_family, gpu_kind) resolutions must land on `resolved="gpu", backend="metal"`; page when any triplet drops below 50 % over 15 m | `FastpqMetalDowngrade` alert (page)【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
| Backend gap | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | Must remain at 0 for every triplet; warn after any sustained (>10 m) bursts | `FastpqBackendNoneBurst` alert (warning)【dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| CPU fallback ratio | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` | ≤5 % of GPU-requested proofs may land on the CPU backend for any triplet; page when a triplet exceeds 5 % for ≥10 m | `FastpqCpuFallbackBurst` alert (page)【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
| Metal queue duty cycle | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | Rolling 15 m average must stay ≥50 % whenever GPU jobs are queued; warn when utilisation drops below target while GPU requests persist | `FastpqQueueDutyCycleDrop` alert (warning)【dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
| Queue depth & zero-fill budget | Wrapped benchmark `metal_dispatch_queue` and `zero_fill_hotspots` blocks | `max_in_flight` must stay at least one slot below `limit` and LDE zero-fill mean must stay ≤0.4 ms (≈80 GB/s) for the canonical 20 000-row trace; any regression blocks the rollout bundle | Reviewed via `scripts/fastpq/wrap_benchmark.py` output and attached to the Stage 7 evidence bundle (`docs/source/fastpq_rollout_playbook.md`). |
| Runtime queue headroom | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | `limit - max_in_flight ≥ 1` for every triplet; warn after 10 m without headroom | `FastpqQueueHeadroomLow` alert (warning)【dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
| Runtime zero-fill latency | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | Latest zero-fill sample must remain ≤0.40 ms (Stage 7 limit) | `FastpqZeroFillRegression` alert (page)【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |

The wrapper enforces the zero-fill row directly. Pass
`--require-zero-fill-max-ms 0.40` to `scripts/fastpq/wrap_benchmark.py` and it
will fail when the bench JSON lacks zero-fill telemetry or when the hottest
zero-fill sample exceeds the Stage 7 budget, preventing rollout bundles from
shipping without the mandated evidence.【scripts/fastpq/wrap_benchmark.py:1008】

#### Stage 7-1 alert-handling checklist

Every alert listed above feeds a specific on-call drill so operators gather the
same artefacts that the release bundle requires:

1. **`FastpqQueueHeadroomLow` (warning).** Run an instantaneous Prometheus query
   for `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` and
   capture the Grafana “Queue headroom” panel from the `fastpq-acceleration`
   board. Record the query result in
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   together with the alert ID so the release bundle proves the warning was
   acknowledged before the queue starved.【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression` (page).** Inspect
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` and, if the metric is
   noisy, rerun `scripts/fastpq/wrap_benchmark.py` on the most recent bench JSON
   to refresh the `zero_fill_hotspots` block. Attach the promQL output,
   screenshots, and refreshed bench file to the rollout directory; this creates
   the same evidence that `ci/check_fastpq_rollout.sh` expects during release
   validation.【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst` (page).** Confirm that
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` exceeds the 5 %
   floor, then sample `irohad` logs for the corresponding downgrade messages
   (`telemetry::fastpq.execution_mode resolved="cpu"`). Store the promQL dump
   plus log excerpts in `metrics_cpu_fallback.prom`/`rollback_drill.log` so the
   bundle demonstrates both the impact and the operator acknowledgement.
4. **Evidence packaging.** After any alert clears, rerun the Stage 7-3 steps in
   the rollout playbook (Grafana export, alert snapshot, rollback drill) and
   revalidate the bundle via `ci/check_fastpq_rollout.sh` before reattaching it
   to the release ticket.【docs/source/fastpq_rollout_playbook.md:114】

Operators who prefer automation can run
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
to query the Prometheus API for the queue headroom, zero-fill, and CPU fallback
metrics listed above; the helper writes the captured JSON (prefixed with the
original promQL) into `metrics_headroom.prom`, `metrics_zero_fill.prom`, and
`metrics_cpu_fallback.prom` under the chosen rollout directory so those files
can be attached to the bundle without manual curl invocations.

`ci/check_fastpq_rollout.sh` now enforces the queue headroom and zero-fill
budget directly. It parses each `metal` bench referenced by
`fastpq_bench_manifest.json`, inspects
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` and
`benchmarks.zero_fill_hotspots[]`, and fails the bundle when headroom drops
below one slot or when any LDE hotspot reports `mean_ms > 0.40`. This keeps the
Stage 7 telemetry guard in CI, matching the manual review performed on the
Grafana snapshot and release evidence.【ci/check_fastpq_rollout.sh#L1】
As part of the same validation pass the script now insists that every wrapped
benchmark carries the auto-detected hardware labels (`metadata.labels.device_class`
and `metadata.labels.gpu_kind`). Bundles missing those labels fail immediately,
guaranteeing that release artefacts, Stage7-2 matrix manifests, and runtime
dashboards all refer to the exact same device-class names.

The Grafana “Latest Benchmark” panel and associated rollout bundle now quote the
`device_class`, zero-fill budget, and queue-depth snapshot so on-call engineers
can correlate production telemetry with the exact capture class used during sign
off. Future matrix entries inherit the same labels, meaning the Stage7-2 device
lists and the Prometheus dashboards share a single naming scheme for Apple M4,
M3 Max, and upcoming MI300/RTX captures.

### Stage7-1 Fleet telemetry runbook

Follow this checklist before enabling GPU lanes by default so fleet telemetry
and Alertmanager rules mirror the same evidence captured during release prep:

1. **Label capture and runtime hosts.** `python3 scripts/fastpq/wrap_benchmark.py`
   already emits `metadata.labels.device_class`, `chip_family`, and `gpu_kind`
   for every wrapped JSON. Keep those labels in sync with
   `fastpq.{device_class,chip_family,gpu_kind}` (or the
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` env vars) inside `iroha_config`
   so runtime metrics publish
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   and the `fastpq_metal_queue_*` gauges with the same identifiers that show up
   in `artifacts/fastpq_benchmarks/matrix/devices/*.txt`. When staging a new
   class, regenerate the matrix manifest via
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   so CI and dashboards understand the additional label.
2. **Verify queue gauges and adoption metrics.** Run `irohad --features fastpq-gpu`
   on the Metal hosts and scrape the telemetry endpoint to confirm live queue
   gauges are exporting:

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```

   The first command proves the semaphore sampler is emitting the `busy`,
   `overlap`, `limit`, and `max_in_flight` series and the second shows whether
   each device class is resolving to `backend="metal"` or falling back to
   `backend="cpu"`. Wire the scrape target through Prometheus/OTel before
   importing the dashboard so Grafana can plot the fleet view immediately.
3. **Install the dashboard + alert pack.** Import
   `dashboards/grafana/fastpq_acceleration.json` into Grafana (retain the
   built-in Device Class, Chip Family, and GPU Kind template variables) and load
   `dashboards/alerts/fastpq_acceleration_rules.yml` into Alertmanager together
   with its unit test fixture. The rule pack ships a `promtool` harness; run
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   whenever the rules change to prove `FastpqMetalDowngrade` and
   `FastpqBackendNoneBurst` still fire at the documented thresholds.
4. **Gate releases with the evidence bundle.** Keep
   `docs/source/fastpq_rollout_playbook.md` handy while generating a rollout
   submission so every bundle carries the wrapped benchmarks, Grafana export,
   alert pack, queue telemetry proof, and rollback logs. CI already enforces the
   contract: `make check-fastpq-rollout` (or invoking
   `ci/check_fastpq_rollout.sh --bundle <path>`) validates the bundle, re-runs
   the alert tests, and refuses to sign off when queue headroom or zero-fill
   budgets regress.
5. **Tie alerts back to remediation.** When Alertmanager pages, use the Grafana
   board and the raw Prometheus counters from step 2 to confirm whether
   downgrades stem from queue starvation, CPU fallbacks, or backend=none bursts.
The runbook lives in
this document plus `docs/source/fastpq_rollout_playbook.md`; update the
release ticket with the relevant `fastpq_execution_mode_total`,
`fastpq_metal_queue_ratio`, and `fastpq_metal_queue_depth` excerpts together
with links to the Grafana panel and the alert snapshot so reviewers can see
exactly which SLO triggered.

### WP2-E — Stage-by-stage Metal profiling snapshot

`scripts/fastpq/src/bin/metal_profile.rs` summarizes the wrapped Metal captures
so the sub-900 ms target can be tracked over time (run
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`).
The new Markdown helper
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
generates the stage tables below (it prints the Markdown along with a textual
summary so WP2-E tickets can embed the evidence verbatim). Two captures are tracked
right now:

> **New WP2-E instrumentation:** `fastpq_metal_bench --gpu-probe ...` now emits a
> detection snapshot (requested/resolved execution mode, `FASTPQ_GPU`
> overrides, detected backend, and the enumerated Metal devices/registry ids)
> before any kernels run. Capture this log whenever a forced GPU run still
> falls back to the CPU path so the evidence bundle records which hosts see
> `MTLCopyAllDevices` return zero and which overrides were in effect during the
> benchmark.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】

> **Stage capture helper:** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> now drives `fastpq_metal_bench` for FFT, LDE, and Poseidon individually,
> stores the raw JSON outputs under per-stage directories, and emits a single
> `stage_profile_summary.json` bundle that records CPU/GPU timings, queue depth
> telemetry, column-staging stats, kernel profiles, and the associated trace
> artefacts. Pass `--stage fft --stage lde --stage poseidon` to target a subset,
> `--trace-template "Metal System Trace"` to pick a specific xctrace template,
> and `--trace-dir` to route `.trace` bundles to a shared location. Attach the
> summary JSON plus the generated trace files to every WP2-E issue so reviewers
> can diff queue occupancy (`metal_dispatch_queue.*`), overlap ratios, and the
> captured launch geometry across runs without manually spelunking multiple
> `fastpq_metal_bench` invocations.【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **Queue/staging evidence helper (2026-05-09):** `scripts/fastpq/profile_queue.py` now
> ingests one or more `fastpq_metal_bench` JSON captures and emits both a Markdown table and
> a machine-readable summary (`--markdown-out/--json-out`) so queue depth, overlap ratios, and
> host-side staging telemetry can ride alongside every WP2-E artefact. Running
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` produced the table below and flagged that the archived Metal captures still report
> `dispatch_count = 0` and `column_staging.batches = 0`—WP2-E.1 remains open until the Metal
> instrumentation is rebuilt with telemetry enabled. The generated JSON/Markdown artefacts live
> under `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` for auditing.
> The helper now (2026-05-19) also surfaces the Poseidon pipeline telemetry (`pipe_depth`,
> `batches`, `chunk_columns`, and `fallbacks`) inside both the Markdown table and the JSON summary,
> so WP2-E.4/6 reviewers can prove whether the GPU stayed on the pipelined path and whether any
> fallbacks occurred without opening the raw capture.【scripts/fastpq/profile_queue.py:1】

> **Stage profile summariser (2026-05-30):** `scripts/fastpq/stage_profile_report.py` consumes
> the `stage_profile_summary.json` bundle emitted by `cargo xtask fastpq-stage-profile` and
> renders both Markdown and JSON summaries so WP2-E reviewers can copy evidence into tickets
> without manually transcribing timings. Invoke
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> to produce deterministic tables listing GPU/CPU means, speedup deltas, trace coverage, and
> telemetry gaps per stage. The JSON output mirrors the table and records per-stage issue tags
> (`trace missing`, `queue telemetry missing`, etc.) so governance automation can diff the host
> runs referenced in WP2-E.1 through WP2-E.6.
> **Host/device overlap guard (2026-06-04):** `scripts/fastpq/profile_queue.py` now annotates
> FFT/LDE/Poseidon wait ratios alongside the per-stage flatten/wait millisecond totals and emits an
> issue whenever `--max-wait-ratio <threshold>` detects poor overlap. Use
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> to capture both the Markdown table and the JSON bundle with explicit wait ratios so WP2-E.5 tickets
> can show whether the double-buffering window kept the GPU fed. The plain-text console output also
> lists the per-phase ratios to make on-call investigations easier.
> **Telemetry guard + run status (2026-06-09):** `fastpq_metal_bench` now emits a `run_status` block
> (backend label, dispatch count, reasons) and the new `--require-telemetry` flag fails the run
> whenever GPU timings or queue/staging telemetry are missing. `profile_queue.py` renders the run
> status as a dedicated column and surfaces non-`ok` states in the issue list, and
> `launch_geometry_sweep.py` threads the same state into warnings/classification so matrices can no
> longer admit captures that silently fell back to CPU or skipped queue instrumentation.
> **Poseidon/LDE auto-tuning (2026-06-12):** `metal_config::poseidon_batch_multiplier()` now scales
> with the Metal working-set hints and `lde_tile_stage_target()` boosts tile depth on discrete GPUs.
> The applied multiplier and tile limit are included in the `metal_heuristics` block of
> `fastpq_metal_bench` outputs and rendered by `scripts/fastpq/metal_capture_summary.py`, so WP2-E
> bundles record the exact pipeline knobs used in each capture without digging through raw JSON.【crates/fastpq_prover/src/metal_config.rs:1】【crates/fastpq_prover/src/metal.rs:2833】【scripts/fastpq/metal_capture_summary.py:1】

| Label | Dispatch | Busy | Overlap | Max Depth | FFT flatten | FFT wait | FFT wait % | LDE flatten | LDE wait | LDE wait % | Poseidon flatten | Poseidon wait | Poseidon wait % | Pipe depth | Pipe batches | Pipe fallbacks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fastpq_metal_bench_poseidon | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### 20 k snapshot (pre-override)

`fastpq_metal_bench_20k_latest.json`

| Stage | Columns | Input len | GPU mean (ms) | CPU mean (ms) | GPU share | Speedup | Δ CPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32 768 | 130.986 ms (115.761–167.755) | 112.616 ms (95.335–132.929) | 2.4 % | 0.860× | −18.370 |
| IFFT | 16 | 32 768 | 129.296 ms (111.127–142.955) | 158.144 ms (126.847–237.887) | 2.4 % | 1.223× | +28.848 |
| LDE | 16 | 262 144 | 1 570.656 ms (1 544.397–1 584.502) | 1 752.523 ms (1 548.807–2 191.930) | 29.2 % | 1.116× | +181.867 |
| Poseidon | 16 | 524 288 | 3 548.329 ms (3 519.881–3 576.041) | 3 642.706 ms (3 539.055–3 758.279) | 66.0 % | 1.027× | +94.377 |

Key observations:

1. The GPU total is 5.379 s, which is **4.48 s over** the 900 ms goal. Poseidon
   hashing still dominates the runtime (≈66 %) with the LDE kernel in second
   place (≈29 %), so WP2-E needs to attack both the Poseidon pipeline depth and
   the LDE memory residency/tiling plan before CPU fallbacks disappear.
2. FFT remains a regression (0.86×) even though IFFT is >1.22× over the scalar
   path. We need a launch-geometry sweep
   (`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`) to understand
   whether the FFT occupancy can be salvaged without hurting the already-better
   IFFT timings. The `scripts/fastpq/launch_geometry_sweep.py` helper now drives
   these experiments end-to-end: pass comma-separated overrides (for example,
   `--fft-columns 16,32 --queue-fanout 1,2` and
   `--poseidon-lanes auto,256`) and it will invoke
   `fastpq_metal_bench` for every combination, store the JSON payloads under
   `artifacts/fastpq_geometry/<timestamp>/`, and persist a `summary.json` bundle
   describing each run’s queue ratios, FFT/LDE launch picks, GPU vs CPU timings,
   and the host metadata (hostname/label, platform triple, detected device
   class, GPU vendor/model) so cross-device comparisons have deterministic
   provenance. The helper now also writes `reason_summary.json` next to the
   summary by default, using the same classifier as the geometry matrix to roll
   up CPU fallbacks and missing telemetry. Use `--host-label staging-m3` to tag
   captures from shared labs.
   The companion `scripts/fastpq/geometry_matrix.py` tool now ingests one or
   more summary bundles (`--summary hostA/summary.json --summary hostB/summary.json`)
   and emits Markdown/JSON tables that label every launch shape as *stable*
   (FFT/LDE/Poseidon GPU timings captured) or *unstable* (timeout, CPU fallback,
   non-Metal backend, or missing telemetry) alongside the host columns. The
   tables now include the resolved `execution_mode`/`gpu_backend` plus a
   `Reason` column so CPU fallbacks and missing GPU timings are obvious in
   Stage 7 matrices even when timing blocks are present; a summary line counts
   the stable vs total runs. Pass `--operation fft|lde|poseidon_hash_columns`
   when the sweep needs to isolate a single stage (for example, to profile
   Poseidon separately) and keep `--extra-args` free for bench-specific flags.
   The helper accepts any
   command prefix (defaulting to `cargo run … fastpq_metal_bench`) plus optional
   `--halt-on-error` / `--timeout-seconds` guards so performance engineers can
   reproduce the sweep on different machines while collecting comparable,
   multi-device evidence bundles for Stage 7.
3. `metal_dispatch_queue` reported `dispatch_count = 0`, so queue occupancy
   telemetry was missing even though GPU kernels ran. The Metal runtime now uses
   acquire/release fences for the queue/column-staging toggles so worker threads
   observe the instrumentation flags, and the geometry matrix report calls out
   unstable launch shapes whenever FFT/LDE/Poseidon GPU timings are absent. Keep
   attaching the Markdown/JSON matrix to WP2-E tickets so reviewers can see
   which combinations are still failing once queue telemetry becomes available.
   The `run_status` guard and `--require-telemetry` flag now fail the capture
   whenever GPU timings are missing or queue/staging telemetry is absent, so
   dispatch_count=0 runs can no longer slip into WP2-E bundles unnoticed.
   `fastpq_metal_bench` now exposes `--require-gpu`, and
   `launch_geometry_sweep.py` enables it by default (opt out with
   `--allow-cpu-fallback`) so CPU fallbacks and Metal detection failures abort
   immediately instead of polluting Stage 7 matrices with non-GPU telemetry.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. Zero-fill metrics previously vanished for the same reason; the fencing fix
   keeps host instrumentation live, so the next capture should include the
   `zero_fill` block without synthetic timings.

#### 20 k snapshot with `FASTPQ_GPU=gpu`

`fastpq_metal_bench_20k_refresh.json`

| Stage | Columns | Input len | GPU mean (ms) | CPU mean (ms) | GPU share | Speedup | Δ CPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32 768 | 79.951 ms (65.645–93.193) | 83.289 ms (59.956–107.585) | 0.3 % | 1.042× | +3.338 |
| IFFT | 16 | 32 768 | 78.605 ms (69.986–83.726) | 93.898 ms (80.656–119.625) | 0.3 % | 1.195× | +15.293 |
| LDE | 16 | 262 144 | 657.673 ms (619.219–712.367) | 669.537 ms (619.716–723.285) | 2.1 % | 1.018× | +11.864 |
| Poseidon | 16 | 524 288 | 30 004.898 ms (27 284.117–32 945.253) | 29 087.532 ms (24 969.810–33 020.517) | 97.4 % | 0.969× | −917.366 |

Observations:

1. Even with `FASTPQ_GPU=gpu`, this capture still reflects CPU fallback:
   ~30 s per iteration with `metal_dispatch_queue` stuck at zero. When the
   override is set but the host can’t discover a Metal device, the CLI now exits
   before running any kernels and prints the requested/resolved mode plus the
   backend label so engineers can tell whether detection, entitlements, or the
   metallib lookup caused the downgrade. Run `fastpq_metal_bench --gpu-probe
   --rows …` with `FASTPQ_DEBUG_METAL_ENUM=1` to capture the enumeration log and
   fix the underlying detection issue before re-running the profiler.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. Zero-fill telemetry now records a real sample (18.66 ms over 32 MiB), proving
   the fencing fix works, but queue deltas remain absent until GPU dispatches
   succeed.
3. Because the backend keeps downgrading, the Stage 7 telemetry gate is still
   blocked: queue headroom evidence and poseidon overlap require a genuine GPU
   run.

These captures now anchor the WP2-E backlog. Next actions: gather profiler
flamecharts and queue logs (once the backend executes on the GPU), target the
Poseidon/LDE bottlenecks before revisiting FFT, and unblock the backend fallback
so Stage 7 telemetry has real GPU data.

### Strengths
- Incremental staging, trace-first design, transparent STARK stack.

### High-Priority Action Items
1. Implement packing/order fixtures and update AIR spec.
2. Finalise Poseidon2 commit `3f2b7fe` and publish example SMT/lookup vectors.
3. Maintain worked examples (`lookup_grand_product.md`, `smt_update.md`) alongside fixtures.
4. Add Appendix A documenting soundness derivation and CI rejection methodology.

### Resolved Design Decisions
- ZK disabled (correctness-only) in P1; revisit in future stage.
- Permission table root derived from governance state; batches treat table as read-only and prove membership via lookup.
- Absent key proofs use zero leaf plus neighbour witness with canonical encoding.
- Delete semantics = leaf value set to zero within canonical keyspace.

Use this document as the canonical reference; update it alongside source code, fixtures, and appendices to avoid drift.

## Appendix A — Soundness Derivation

This appendix explains how the “Soundness & SLOs” table is produced and how CI enforces the ≥128-bit floor mentioned earlier.

### Notation
- `N_trace = 2^k` — trace length after sorting and padding to a power of two.
- `b` — blowup factor (`N_eval = N_trace × b`).
- `r` — FRI arity (8 or 16 for the canonical sets).
- `ℓ` — number of FRI reductions (`layers` column).
- `q` — verifier queries per proof (`queries` column).
- `ρ` — effective code rate reported by the column planner: `ρ = max_i(degree_i / domain_i)` over the constraints that survive the first FRI round.

The Goldilocks base field has `|F| = 2^64 - 2^32 + 1`, so Fiat–Shamir collisions are bounded by `q / 2^64`. Grinding adds an orthogonal `2^{-g}` factor, with `g = 23` for `fastpq-lane-balanced` and `g = 21` for the latency profile.【crates/fastpq_isi/src/params.rs:65】

### Analytic bound

With constant-rate DEEP-FRI the statistical failure probability satisfies

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

because each layer reduces polynomial degree and domain width by the same factor `r`, keeping `ρ` constant. The table’s `est bits` column reports `⌊-log₂ p_fri⌋`; Fiat–Shamir and grinding serve as extra safety margin.

### Planner output and worked computation

Running the Stage 1 column planner on representative batches yields:

| Parameter set | `N_trace` | `b` | `N_eval` | `ρ` (planner) | Effective degree (`ρ × N_eval`) | `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------- | --------- | --- | -------- | ------------- | -------------------------------- | --- | --- | ------------------ |
| Balanced 20 k batch | `2^15` | 8  | 262 144  | 0.077026 | 20 192  | 5 | 52 | 190 bits |
| Throughput 65 k batch | `2^16` | 8 | 524 288 | 0.200208 | 104 967 | 6 | 58 | 132 bits |
| Latency 131 k batch | `2^17` | 16 | 2 097 152 | 0.209492 | 439 337 | 5 | 64 | 142 bits |

Example (balanced 20 k batch):
1. `N_trace = 2^15`, so `N_eval = 2^15 × 8 = 2^18`.
2. Planner instrumentation reports `ρ = 0.077026`, so `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`.
3. `-log₂ p_fri = 190 bits`, matching the table entry.
4. Fiat–Shamir collisions add at most `2^{-58.3}`, and grinding (`g = 23`) subtracts another `2^{-23}`, keeping the total soundness comfortably above 160 bits.

### CI rejection-sampling harness

Every CI run executes a Monte Carlo harness to ensure empirical measurements stay within ±0.6 bits of the analytic bound:
1. Draw a canonical parameter set and synthesise a `TransitionBatch` with the matching row count.
2. Build the trace, flip a randomly chosen constraint (e.g., perturb the lookup grand product or an SMT sibling), and attempt to produce a proof.
3. Rerun the verifier, resampling Fiat–Shamir challenges (grinding included), and record whether the tampered proof is rejected.
4. Repeat for 16 384 seeds per parameter set and convert the 99 % Clopper–Pearson lower bound of the observed rejection rate into bits.

The job fails immediately if the measured lower bound slips under 128 bits, so regressions in the planner, folding loop, or transcript wiring are caught before merge.

## Appendix B — Domain-root derivation

Stage 0 pins the trace and evaluation generators to Poseidon-derived constants so all implementations share the same subgroups.

### Procedure
1. **Seed selection.** Absorb the UTF‑8 tag `fastpq:v1:domain_roots` into the Poseidon2 sponge used elsewhere in FASTPQ (state width = 3, rate = 2, four full + 57 partial rounds). Inputs reuse the `[len, limbs…]` encoding from `pack_bytes`, yielding the base generator `g_base = 7`.【crates/fastpq_prover/src/packing.rs:44】【scripts/fastpq/src/bin/poseidon_gen.rs:1】
2. **Trace generator.** Compute `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` and verify `trace_root^{2^{trace_log_size}} = 1` while the half-power is not 1.
3. **LDE generator.** Repeat the same exponentiation with `lde_log_size` to derive `lde_root`.
4. **Coset selection.** Stage 0 uses the base subgroup (`omega_coset = 1`). Future cosets can absorb an additional tag such as `fastpq:v1:domain_roots:coset`.
5. **Permutation size.** Persist `permutation_size` explicitly so schedulers never infer padding rules from implicit powers of two.

### Reproduction and validation
- Tooling: `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` emits either Rust snippets or a Markdown table (see `--format table`, `--seed`, `--filter`).【scripts/fastpq/src/bin/poseidon_gen.rs:1】
- Tests: `canonical_sets_meet_security_target` keeps the canonical parameter sets aligned with the published constants (non-zero roots, blowup/arity pairing, permutation sizing), so `cargo test -p fastpq_isi` catches drift immediately.【crates/fastpq_isi/src/params.rs:138】
- Source of truth: update the Stage 0 table and `fastpq_isi/src/params.rs` together whenever new parameter packs are introduced.

## Appendix C — Commitment pipeline details

### Streaming Poseidon commitment flow
Stage 2 defines the deterministic trace commitment shared by the prover and verifier:
1. **Normalise transitions.** `trace::build_trace` sorts each batch, pads it to `N_trace = 2^{⌈log₂ rows⌉}`, and emits column vectors in the order below.【crates/fastpq_prover/src/trace.rs:123】
2. **Hash columns.** `trace::column_hashes` streams the columns through dedicated Poseidon2 sponges tagged `fastpq:v1:trace:column:<name>`. When the `fastpq-prover-preview` feature is active the same traversal recycles the IFFT/LDE coefficients required by the backend, so no extra matrix copies are allocated.【crates/fastpq_prover/src/trace.rs:474】
3. **Lift into a Merkle tree.** `trace::merkle_root` folds the column digests with Poseidon nodes tagged `fastpq:v1:trace:node`, duplicating the last leaf whenever a level has odd fan-out to avoid special cases.【crates/fastpq_prover/src/trace.rs:656】
4. **Finalize the digest.** `digest::trace_commitment` prefixes the domain tag (`fastpq:v1:trace_commitment`), parameter name, padded dimensions, column digests, and Merkle root using the same `[len, limbs…]` encoding, then hashes the payload with SHA3-256 before embedding it in `Proof::trace_commitment`.【crates/fastpq_prover/src/digest.rs:25】

The verifier recomputes the same digest before sampling Fiat–Shamir challenges, so mismatches abort proofs before any openings.

### Poseidon fallback controls

- The prover now exposes a dedicated Poseidon pipeline override (`zk.fastpq.poseidon_mode`, env `FASTPQ_POSEIDON_MODE`, CLI `--fastpq-poseidon-mode`) so operators can mix GPU FFT/LDE with CPU Poseidon hashing on devices that fail to reach the Stage 7 <900 ms target. Supported values mirror the execution-mode knob (`auto`, `cpu`, `gpu`), defaulting to the global mode when unspecified. The runtime threads this value through the lane config (`FastpqPoseidonMode`) and propagates it into the prover (`Prover::canonical_with_modes`) so overrides are deterministic and auditable in config dumps.【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- Telemetry exports the resolved pipeline mode via the new `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` counter (and OTLP twin `fastpq.poseidon_pipeline_resolutions_total`). `sorafs`/operator dashboards can therefore confirm when a rollout is running GPU fused/pipelined hashing versus the forced CPU fallback (`path="cpu_forced"`) or runtime downgrades (`path="cpu_fallback"`). The CLI probe installs automatically in `irohad`, so release bundles and live telemetry share the same evidence stream.【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- Mixed-mode evidence is also stamped into every scoreboard via the existing adoption gate: the prover emits the resolved mode + path label for each batch, and the `fastpq_poseidon_pipeline_total` counter increments alongside the execution-mode counter whenever a proof lands. This satisfies WP2-E.6 by making brownouts visible and by providing a clean switch for deterministic downgrades while optimisation continues.【crates/fastpq_prover/src/trace.rs:1684】【docs/source/sorafs_orchestrator_rollout.md:139】
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` now parses Prometheus scrapes (Metal or CUDA) and embeds a `poseidon_metrics` summary inside every wrapped bundle. The helper filters the counter rows by `metadata.labels.device_class`, captures the matching `fastpq_execution_mode_total` samples, and fails the wrap when `fastpq_poseidon_pipeline_total` entries are missing so WP2-E.6 bundles always ship reproducible CUDA/Metal evidence instead of ad-hoc notes.【scripts/fastpq/wrap_benchmark.py:1】【scripts/fastpq/tests/test_wrap_benchmark.py:1】

#### Deterministic mixed-mode policy (WP2-E.6)

1. **Detect GPU shortfall.** Flag any device-class whose Stage 7 capture or live Grafana snapshot shows Poseidon latency keeping the total proof time >900 ms while FFT/LDE stay below target. Operators annotate the capture matrix (`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) and page the on-call when `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` stagnates while `fastpq_execution_mode_total{backend="metal"}` still records GPU FFT/LDE dispatches.【scripts/fastpq/wrap_benchmark.py:1】【dashboards/grafana/fastpq_acceleration.json:1】
2. **Flip to CPU Poseidon only for the affected hosts.** Set `zk.fastpq.poseidon_mode = "cpu"` (or `FASTPQ_POSEIDON_MODE=cpu`) in the host-local config alongside the fleet labels, keeping `zk.fastpq.execution_mode = "gpu"` so FFT/LDE continue to use the accelerator. Record the config diff in the rollout ticket and add the per-host override to the bundle as `poseidon_fallback.patch` so reviewers can replay the change deterministically.
3. **Prove the downgrade.** Scrape the Poseidon counter immediately after restarting the node:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   The dump must show `path="cpu_forced"` growing in lock-step with the GPU execution counter. Store the scrape as `metrics_poseidon.prom` next to the existing `metrics_cpu_fallback.prom` snapshot and capture the matching `telemetry::fastpq.poseidon` log lines in `poseidon_fallback.log`.
4. **Monitor & exit.** Keep alerting on `fastpq_poseidon_pipeline_total{path="cpu_forced"}` while optimisation work continues. Once a patch brings the per-proof runtime back under 900 ms on the test host, roll the config back to `auto`, re-run the scrape (showing `path="gpu"` again), and attach the before/after metrics to the bundle to close the mixed-mode drill.

**Telemetry contract.**

| Signal | PromQL / Source | Purpose |
|--------|-----------------|---------|
| Poseidon mode counter | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` | Confirms CPU hashing is intentional and scoped to the flagged device-class. |
| Execution mode counter | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` | Proves FFT/LDE still run on GPU even while Poseidon downgrades. |
| Log evidence | `telemetry::fastpq.poseidon` entries captured in `poseidon_fallback.log` | Provides per-proof proof that the host resolved to CPU hashing with Reason `cpu_forced`. |

The rollout bundle must now include `metrics_poseidon.prom`, the config diff, and the log excerpt whenever mixed-mode is active so governance can audit the deterministic fallback policy alongside the FFT/LDE telemetry. `ci/check_fastpq_rollout.sh` already enforces the queue/zero-fill limits; the follow-up gate will sanity-check the Poseidon counter once mixed-mode lands in release automation.

The Stage 7 capture tooling already handles CUDA: wrap every `fastpq_cuda_bench` bundle with `--poseidon-metrics` (pointing at the scraped `metrics_poseidon.prom`) and the output now carries the same pipeline counters/resolution summary used on Metal so governance can verify CUDA fallbacks without bespoke tooling.【scripts/fastpq/wrap_benchmark.py:1】

### Column order
The hashing pipeline consumes columns in this deterministic order:
1. Selector flags: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
2. Packed limb columns (each zero-padded to the trace length): `key_limb_{i}`, `value_old_limb_{i}`, `value_new_limb_{i}`, `asset_id_limb_{i}`.
3. Auxiliary scalars: `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`, `perm_hash`, `neighbour_leaf`, `dsid`, `slot`.
4. Sparse Merkle witnesses for every level `ℓ ∈ [0, SMT_HEIGHT)`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`.

`trace::column_hashes` walks the columns in exactly this order, so the placeholder backend and Stage 2 STARK implementation remain trace-stable across releases.【crates/fastpq_prover/src/trace.rs:474】

### Transcript domain tags
Stage 2 fixes the Fiat–Shamir catalog below to keep challenge generation deterministic:

| Tag | Purpose |
| --- | ------- |
| `fastpq:v1:init` | Absorb protocol version, parameter set, and `PublicIO`. |
| `fastpq:v1:roots` | Commit the trace and lookup Merkle roots. |
| `fastpq:v1:gamma` | Sample the lookup grand-product challenge. |
| `fastpq:v1:alpha:<i>` | Sample composition-polynomial challenges (`i = 0, 1`). |
| `fastpq:v1:lookup:product` | Absorb the evaluated lookup grand product. |
| `fastpq:v1:beta:<round>` | Sample the folding challenge for each FRI round. |
| `fastpq:v1:fri_layer:<round>` | Commit the Merkle root for each FRI layer. |
| `fastpq:v1:fri:final` | Record the final FRI layer before opening queries. |
| `fastpq:v1:query_index:0` | Deterministically derive verifier query indices. |
