## Acceleration & Norito Heuristics Reference

The `[accel]` block in `iroha_config` threads through
`crates/irohad/src/main.rs:1895` into `ivm::set_acceleration_config`. Every host
applies the same knobs before instantiating the VM, so operators can deterministically
pick which GPU backends are allowed while keeping scalar/SIMD fallbacks available.
Swift, Android, and Python bindings load the same manifest via the bridge layer, so
documenting these defaults unblocks WP6-C in the hardware-acceleration backlog.

### `accel` (hardware acceleration)

The table below mirrors `docs/source/references/peer.template.toml` and the
`iroha_config::parameters::user::Acceleration` definition, exposing the environment
variable that overrides each key.

| Key | Env var | Default | Description |
|-----|---------|---------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | Enables SIMD/NEON/AVX execution. When `false`, the VM forces scalar backends for vector ops and Merkle hashing to ease deterministic parity captures. |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | Enables the CUDA backend when it is compiled in and the runtime passes all golden-vector checks. |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | Enables the Metal backend on macOS builds. Even when true, Metal self-tests can still disable the backend at runtime if parity mismatches occur. |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (auto) | Caps how many physical GPUs the runtime initialises. `0` means “match hardware fan-out” and is clamped by `GpuManager`. |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Minimum leaves required before Merkle leaf hashing offloads to GPU. Values below this threshold keep hashing on the CPU to avoid PCIe overhead (`crates/ivm/src/byte_merkle_tree.rs:49`). |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (inherit global) | Metal-specific override for the GPU threshold. When `0`, Metal inherits `merkle_min_leaves_gpu`. |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (inherit global) | CUDA-specific override for the GPU threshold. When `0`, CUDA inherits `merkle_min_leaves_gpu`. |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (32 768 internally) | Caps the tree size where ARMv8 SHA-2 instructions should win over GPU hashing. `0` keeps the compiled-in default of `32_768` leaves (`crates/ivm/src/byte_merkle_tree.rs:59`). |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0` (32 768 internally) | Same as above but for x86/x86_64 hosts using SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`). |

Example configuration:

```toml
[accel]
enable_simd = true
enable_cuda = true
enable_metal = true
max_gpus = 2
merkle_min_leaves_gpu = 12288
merkle_min_leaves_metal = 8192
merkle_min_leaves_cuda = 16384
prefer_cpu_sha2_max_leaves_aarch64 = 65536
```

Zero values for the last five keys mean “keep the compiled default”. Hosts must not
set conflicting overrides (e.g., disabling CUDA while forcing CUDA-only thresholds),
otherwise the request is ignored and the backend continues to follow the global policy.

### Inspecting runtime state

Run `cargo xtask acceleration-state [--format table|json]` to snapshot the applied
configuration alongside the Metal/CUDA runtime health bits. The command pulls the
current `ivm::acceleration_config`, parity status, and sticky error strings (if a
backend was disabled) so operations can feed the result directly into parity
dashboards or incident reviews.

```
$ cargo xtask acceleration-state
Acceleration Configuration
--------------------------
enable_simd: yes
enable_metal: yes
enable_cuda: no
max_gpus: 1
merkle_min_leaves_gpu: 8192
merkle_min_leaves_metal: 8192
merkle_min_leaves_cuda: auto
prefer_cpu_sha2_max_leaves_aarch64: auto
prefer_cpu_sha2_max_leaves_x86: auto

Backend Status
--------------
Backend Supported  Configured  Available  ParityOK  Last error
SIMD    yes        yes         yes        yes       -
Metal   yes        yes         yes        yes       -
CUDA    no         no          no         no        policy disabled (no CUDA libraries present)
```

Use `--format json` when the snapshot needs to be ingested by automation (the JSON
contains the same fields shown in the table).

`acceleration_runtime_errors()` now calls out why SIMD fell back to scalar:
`disabled by config`, `forced scalar override`, `simd unsupported on hardware`, or
`simd unavailable at runtime` when detection succeeds but execution still runs
without vectors. Clearing the override or re-enabling the policy drops the message
on hosts that support SIMD.

### Parity checks

Flip `AccelerationConfig` between CPU-only and accel-on to prove deterministic results.
The `poseidon_instructions_match_across_acceleration_configs` regression runs the
Poseidon2/6 opcodes twice—first with `enable_cuda`/`enable_metal` set to `false`, then
with both enabled—and asserts identical outputs plus CUDA parity when GPUs are present.【crates/ivm/tests/crypto.rs:100】
Capture `acceleration_runtime_status()` alongside the run to record whether backends
were configured/available in lab logs.

```rust
let baseline = ivm::acceleration_config();
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: false,
    enable_metal: false,
    ..baseline
});
// run CPU-only parity workload
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: true,
    enable_metal: true,
    ..baseline
});

When isolating SIMD/NEON differences, set `enable_simd = false` to force scalar
execution. The `disabling_simd_forces_scalar_and_preserves_outputs` regression
forces the scalar backend and asserts vector ops stay bit-identical to the
SIMD-enabled baseline on the same host while surfacing the `simd` status/error
fields via `acceleration_runtime_status`/`acceleration_runtime_errors`.【crates/ivm/tests/acceleration_simd.rs:9】
```

### GPU defaults & heuristics

`MerkleTree` GPU offload kicks in at `8192` leaves by default, and the CPU SHA-2
preference thresholds stay at `32_768` leaves per architecture. When neither CUDA nor
Metal is available or has been disabled by health checks, the VM automatically falls
back to SIMD/scalar hashing and the above numbers do not affect determinism.

`max_gpus` clamps the pool size fed into `GpuManager`. Setting `max_gpus = 1` on
multi-GPU hosts keeps telemetry simple while still allowing acceleration. Operators can
use this switch to reserve the remaining devices for FASTPQ or CUDA Poseidon jobs.

### Next acceleration targets & budgets

The latest FastPQ Metal trace (`fastpq_metal_bench_20k_latest.json`, 32 K rows × 16
columns, 5 iters) shows Poseidon column hashing dominating ZK workloads:

- `poseidon_hash_columns`: CPU mean **3.64 s** vs. GPU mean **3.55 s** (1.03×).
- `lde`: CPU mean **1.75 s** vs. GPU mean **1.57 s** (1.12×).

IVM/Crypto will target these two kernels in the next accel sweep. Baseline budgets:

- Keep scalar/SIMD parity at or below the CPU means above, and capture
  `acceleration_runtime_status()` alongside each run so Metal/CUDA availability is
  logged with the budget numbers.
- Target ≥1.3× speedup for `poseidon_hash_columns` and ≥1.2× for `lde` once tuned Metal
  and CUDA kernels land, without changing outputs or telemetry labels.

Attach the JSON trace and `cargo xtask acceleration-state --format json` snapshot to
future lab runs so CI/regressions can assert both the budgets and backend health while
comparing CPU-only vs. accel-on runs.

### Norito heuristics (compile-time defaults)

Norito’s layout and compression heuristics live in `crates/norito/src/core/heuristics.rs`
and are compiled into every binary. They are not configurable at runtime, but exposing
the inputs helps SDK and operator teams predict how Norito will behave once GPU
compression kernels are enabled.
The workspace now builds Norito with the `gpu-compression` feature enabled by default,
so GPU zstd backends are compiled in; runtime availability still depends on hardware,
the helper library (`libgpuzstd_*`/`gpuzstd_cuda.dll`), and the `allow_gpu_compression`
config flag.

| Field | Default | Purpose |
|-------|---------|---------|
| `min_compress_bytes_cpu` | `256` bytes | Below this, payloads skip zstd entirely to avoid overhead. |
| `min_compress_bytes_gpu` | `1_048_576` bytes (1 MiB) | Payloads at or above this limit switch to GPU zstd when `norito::core::hw::has_gpu_compression()` is true. |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` | CPU compression levels for <32 KiB and ≥32 KiB payloads respectively. |
| `zstd_level_gpu` | `1` | Conservative GPU level to keep latency consistent while filling command queues. |
| `large_threshold` | `32_768` bytes | Size boundary between the “small” and “large” CPU zstd levels. |
| `aos_ncb_small_n` | `64` rows | Below this row count adaptive encoders probe both AoS and NCB layouts to pick the smallest payload. |
| `combo_no_delta_small_n_if_empty` | `2` rows | Prevents enabling u32/id delta encodings when 1–2 rows contain empty cells. |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | Deltas kick in only once there are at least two rows. |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | All delta transforms are enabled by default for well-behaved inputs. |
| `combo_enable_name_dict` | `true` | Allows per-column dictionaries when hit ratios justify the memory overhead. |
| `combo_dict_ratio_max` | `0.40` | Disable dictionaries when more than 40 % of rows are distinct. |
| `combo_dict_avg_len_min` | `8.0` | Require average string length ≥8 before building dictionaries (short aliases stay inline). |
| `combo_dict_max_entries` | `1024` | Hard cap on dictionary entries to guarantee bounded memory usage. |

These heuristics keep GPU-enabled hosts aligned with CPU-only peers: the selector
never makes a decision that would change the wire format, and the thresholds are fixed
per release. When profiling uncovers better break-even points, Norito updates the
canonical `Heuristics::canonical` implementation and `docs/source/benchmarks.md` plus
`status.md` record the change alongside the versioned evidence.

### Troubleshooting and parity checklist

- Snapshot runtime state with `cargo xtask acceleration-state --format json` and keep
  the output alongside any failing logs; the report shows configured/available backends
  plus parity/last-error strings.
- Re-run the accel parity regression locally to rule out drift:
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (runs CPU-only then accel-on). Record `acceleration_runtime_status()` for the run.
- If a backend fails self-tests, keep the node online in CPU-only mode (`enable_metal =
  false`, `enable_cuda = false`) and open an incident with the captured parity output
  instead of forcing the backend on. Results must remain deterministic across modes.
- TODO: add a CUDA parity smoke on lab NV hardware once sm_8x cycles reopen; use the
  same poseidon parity harness with `ACCEL_ENABLE_CUDA=1` and attach the status snapshot
  to the benchmark artefacts.
