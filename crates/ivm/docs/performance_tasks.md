# Performance Improvement Tasks

This document lists potential optimizations that could speed up the IVM
implementation. Each bullet can be turned into an issue or a future pull
request.

## SIMD and Vector Optimizations

- **Complete vectorized Poseidon** – **done.** `src/poseidon.rs` now uses the SIMD field backends so `POSEIDON2` and `POSEIDON6` are accelerated automatically.

- **SSE/AVX intrinsics for vector helpers** – **done.** The helper
  functions in `src/vector.rs` now include SSE2 and AVX2 implementations
  selected at runtime, speeding up `vadd32`, `vadd64` and the bitwise
  operations on x86 CPUs without GPU acceleration.
- **Broader SIMD detection** – **done.** A global `SimdChoice` is set on first
  use in `vector::simd_choice()` which checks for AVX‑512, AVX2, SSE2 or NEON at
  runtime and caches the result so subsequent vector operations dispatch without
  further feature checks.

- **AES‑NI/CPU AES round acceleration** – **done.** `aesenc`/`aesdec` now use
  AES‑NI on x86/x86_64 and ARMv8 AESE/AESD on AArch64 when available
  (runtime‑detected), with scalar and CUDA fallbacks. Results are bit‑exact with
  the reference round implementation.

- **AArch64 SHA‑256 (ARMv8 SHA2)** – **done.** `sha256_compress` uses ARMv8
  SHA2 intrinsics on AArch64 when available (runtime‑detected with a golden
  self‑test), complementing the Metal and CUDA GPU paths. Falls back to the
  scalar reference if the feature is unavailable or self‑test fails.

- **x86 SHA‑256 (SHA‑NI) Merkle heuristic** – **done.** Merkle helpers prefer
  CPU hashing for medium‑sized trees on x86/x86_64 when Intel SHA‑NI is
  available, avoiding GPU launch overhead and improving latency; large trees
  still offload to GPU when beneficial.

## Memory and Merkle Tree

- **Parallel root calculation** – `Memory::commit()` hashes dirty chunks in
  parallel with `rayon` by updating Merkle leaves concurrently, significantly
  reducing commit time on large memories.
- **Incremental hashing** – *done.* `commit()` re-hashes only subtrees for
  chunks marked dirty since the last commit.
- **CPU-accelerated leaf hashing** – *done.* Merkle leaf digests for 32‑byte
  chunks use the VM’s accelerated `sha256_compress` (Metal/CUDA/ARMv8 SHA2/x86
  SHA‑NI/scalar fallback) via a single padded block, speeding up CPU paths and
  reducing the need to offload medium trees to GPU.

## Scheduling and Parallel Execution

- **Dynamic thread pool sizing** – the scheduler now tracks the average number
  of tasks over the last few blocks. If the workload exceeds four times the
  current pool size it doubles the number of threads up to a configurable
  maximum. When the workload drops below half the pool size it scales back down
  but never below the configured minimum.
- **Profile HTM paths** – the instruction-level scheduler has optional hardware
  transactional memory support. Gathering benchmarks on real hardware will help
  decide whether maintaining this code is worthwhile.

## Benchmarking

- Extend the Criterion benchmarks under `benches/` to cover vector helpers,
  memory commits and scheduler throughput.
- Automate benchmark execution in CI and track results over time.

## Field Arithmetic Backends

BN254 field helpers dispatch through the `FieldArithmetic` trait which
defines `add`, `sub` and `mul` operations【F:src/field_dispatch.rs†L5-L21】.
The active implementation is selected by [`field_impl`] at runtime. On the
first call a global `OnceLock` stores the chosen backend based on
[`vector::simd_choice`] and all threads reuse the cached reference
afterwards, ensuring thread safe initialization【F:src/field_dispatch.rs†L34-L53】.

### Adding a new SIMD implementation

1. Extend `SimdChoice` and update `vector::simd_choice` to detect the new
   CPU feature.
2. Implement a new backend struct (e.g. `FooField`) and provide the
   `FieldArithmetic` trait methods using the new intrinsics.
3. Match the new variant inside `field_impl` so the backend becomes
   selectable at runtime.
4. Add unit tests verifying correctness against the scalar reference
   implementation and include Criterion benchmarks if possible.


For a detailed breakdown of the tasks involved in integrating the SIMD dispatch layer, see [dynamic_dispatch_tasks.md](dynamic_dispatch_tasks.md).

### Benchmark results

The `bench_field` benchmark suite exercises the BN254 field backends along with
the Poseidon2 and Poseidon6 permutations. On an AVX2-capable CPU the vector path
is roughly twice as fast for basic arithmetic compared with the scalar backend.
Poseidon permutations run about 1.5× faster. SSE2 and NEON show smaller but
consistent improvements, and no backend is slower than the scalar reference.
