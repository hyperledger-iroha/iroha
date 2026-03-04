# GPU Offloading Design

Status
- Metal: SHA‑256 compression and batched leaf hashing/reduction used for Merkle
  roots are accelerated on macOS; bitwise vector kernels (add/and/xor/or) are
  wired through `vector::*` helpers with deterministic fallbacks.
- CUDA: A `build.rs` script compiles kernels in `cuda/` to PTX at build time and copies bundled `*.ptx` files if `nvcc` is unavailable; vector helpers, SHA‑256, Merkle leaf hashing/reduction, Keccak, Poseidon2/6 permutations, and BN254 add/sub/mul all have GPU paths with automatic parity self‑tests.
- Python (`iroha_python.gpu`) and Java (`CudaAccelerators`) bindings surface CUDA availability
  probes together with optional Poseidon/BN254 wrappers, returning fallbacks when hardware support
  is missing so applications can branch deterministically.
- Developers can compare CPU vs CUDA throughput locally via Criterion benches such as `crates/ivm/benches/bench_bn254_cuda.rs` (run with `cargo bench -p ivm --bench bench_bn254_cuda --features cuda`).

## PTX Build Process

CUDA kernels live under [`cuda/`](../cuda/). During compilation `build.rs` invokes `nvcc` (overridable via the `NVCC` environment variable) to convert any `.cu` sources such as `sha256.cu` into PTX files placed in the crate’s output directory. When the CUDA toolkit is missing or `nvcc` fails, the script falls back to shipping the precompiled `.ptx` artifacts stored alongside the sources. The build prints `cargo:warning` lines when `nvcc` is unavailable or when neither compilation nor bundled PTX is present, in which case it aborts with an error. This keeps builds deterministic and allows the crate to work without a local CUDA installation.

You can explicitly select the target SM/architecture via environment variables (dev/operator convenience; production gating should use config):

- `IVM_CUDA_GENCODE` — passes a full `-gencode` tuple to `nvcc`.
  - Example: `IVM_CUDA_GENCODE="arch=compute_80,code=sm_80"`
- `IVM_CUDA_SM` — selects a specific SM with `-arch=sm_<NN>`.
  - Example: `IVM_CUDA_SM=80` → `-arch=sm_80`
- `IVM_CUDA_ARCH` — passes the raw `-arch=<value>` (e.g., `sm_75`, `compute_80`).
- `IVM_CUDA_NVCC_EXTRA` — additional flags appended verbatim to `nvcc` (testing only).

Determinism note: These knobs do not change observable outputs; kernels implement identical integer logic across SMs. They only control the PTX/SASS targeting and may influence performance.

This document outlines a plan for accelerating selected IVM operations on CUDA GPUs while preserving bit-for-bit determinism across nodes. The VM already provides CPU SIMD paths and Metal compute kernels for some vector helpers. Extending this to CUDA allows heavier workloads to run on up to eight GPUs without altering execution results.

## Candidate Operations

The following hotspots contain tight loops or field arithmetic that map well to massive parallelism:

- **SHA‑256 compression** – [`sha256_compress`](../src/vector.rs) iterates over 64 rounds of 32‑bit arithmetic【F:src/vector.rs†L503-L561】.
- **Poseidon permutations** – [`poseidon2`](../src/poseidon.rs) and [`poseidon6`](../src/poseidon.rs) use repeated S‑box and MDS matrix steps with field multiplications【F:src/poseidon.rs†L60-L118】【F:src/poseidon.rs†L180-L248】.
- **Keccak‑f1600** – the `keccak_f1600` function processes a 25‑lane state with many rotations and XORs【F:src/sha3.rs†L1-L63】.
- **AES round helpers** – `aesenc` and `aesdec` apply S‑boxes and MixColumns over 16‑byte states【F:src/aes.rs†L132-L175】.
- **Vector helpers** – functions like `vadd32_slice` and `vadd64_slice` perform lane-wise arithmetic on large arrays【F:src/vector.rs†L575-L613】.
- **BN254 field arithmetic** – accelerated on CUDA via `bn254_add_kernel`, `bn254_sub_kernel`, and `bn254_mul_kernel` (see `crates/ivm/cuda/bn254.cu`) with scalar/SIMD fallbacks for hosts without GPUs; the host wrappers in `crates/ivm/src/cuda.rs` flatten operands, submit through `GpuManager`, and disable the backend on first mismatch. Tests in `crates/ivm/tests/cuda_extra.rs` cover GPU vs CPU parity.
- **Merkle hashing** – `ByteMerkleTree::from_bytes_parallel` hashes leaves and inner nodes in parallel using Rayon threads【F:src/byte_merkle_tree.rs†L72-L98】.
- **Signature verification** – wrappers in [`signature.rs`] dispatch to Ed25519 or Dilithium libraries to check signatures【F:src/signature.rs†L27-L63】.

## Integration Approach

1. **CUDA Runtime Module** – Introduce an optional `gpu` module that initializes CUDA contexts on start-up. When enabled, the VM loads precompiled PTX kernels for each operation. The existing Rust implementations remain as fallbacks when no GPU is present.
2. **Kernel Design** – Each kernel mirrors the pure Rust logic so results remain identical. For example, a `sha256_compress_cuda` kernel consumes one 64‑byte block per thread block and writes the updated state. Poseidon kernels operate on small fixed arrays of 64‑bit limbs.
3. **Work Queues** – The VM dispatcher packages GPU-friendly tasks (e.g., batches of vector additions or sets of Poseidon hashes) and submits them to a queue. After all kernels finish, results are copied back and committed in program order.
4. **Multi‑GPU Scheduling** – With eight devices available, tasks are striped across GPUs by a deterministic hash of the program counter and call depth. This ensures every node assigns work to the same GPU index for a given instruction sequence. Large batches (e.g., Merkle tree updates) are divided into equal chunks per device.
5. **Fallback Paths** – If any GPU fails, or the `gpu` feature is disabled, the dispatcher reverts to the existing CPU/Metal paths. The opcode semantics are unchanged so consensus cannot diverge.

## Scheduler Integration

The [`Scheduler`](../src/parallel.rs) detects all available GPUs on start up using `GpuManager` and exposes `gpu_count()` as a hint for higher layers. GPU assignment is purely data-driven so each task always maps to the same device across nodes. Transactions with heavy vector or hashing workloads can thus run in parallel across up to eight GPUs while CPU threads handle the coordination.
Runtime behaviour can be adjusted with environment variables:
- `IVM_DISABLE_CUDA` – disable offloading even when compiled with the `cuda` feature.
- `IVM_MAX_GPUS` – limit the number of GPUs initialised.
- `IVM_FORCE_CUDA_SELFTEST_FAIL` – force CUDA golden self‑test to fail and disable the backend (tests/dev).
- `IVM_DISABLE_METAL` – disable Metal backend even on supported macOS hosts (tests/dev).
- `IVM_FORCE_METAL_SELFTEST_FAIL` – force Metal golden self‑test to fail and disable the backend (tests/dev).

SIMD selection is driven by configuration. For deterministic runs or benchmarks,
set `AccelerationPolicy::with_forced_simd(Some(SimdChoice::Scalar|Sse2|Avx2|Avx512|Neon))`
via `IvmConfig` or call `ivm::set_forced_simd` in tests; unsupported choices
fall back to the scalar backend.

## Ensuring Determinism

- **Pure Integer Arithmetic** – Kernels avoid floating point math entirely. All operations use 32‑bit or 64‑bit integers and fixed‑width field limbs, matching the CPU code exactly.
- **Fixed Reduction Order** – Parallel reductions (e.g., in Merkle hashing) accumulate results in a predetermined order per chunk so thread scheduling cannot change the final digest.
- **Synchronous Commits** – The VM waits for all kernels in a cycle to finish before applying their outputs to the state. Results are committed sequentially in instruction order as done for CPU execution.
- **Golden Self‑tests and Auto‑Disable** – On startup/first use, GPU backends (Metal, CUDA) execute small golden vectors (vadd32, SHA‑256, Keccak). Any mismatch disables the backend at runtime and the VM falls back to CPU scalar/SIMD paths, preserving correctness.
- **Deterministic Work Assignment** – Mapping from instruction index to GPU ID is purely data driven (e.g., `gpu_id = hash(tx_id, instr_index) % 8`). Every node derives the same mapping and thus launches kernels in the same sequence.
- **Runtime selection + fallbacks** – `AccelerationConfig` and env overrides (`IVM_DISABLE_{CUDA,METAL}`, `IVM_FORCE_*_SELFTEST_FAIL`) short‑circuit GPU use. CUDA helpers either return `None` (Poseidon/Keccak/BN254) or a CPU result wrapped in `Some` (AES rounds) so callers always get deterministic outputs. Tests in `crates/ivm/tests/cuda_disable_on_mismatch.rs` cover forced self‑test failures and config disables for SHA‑256, Poseidon, and AES.

## Summary Roadmap

1. Implement CUDA kernels for vector helpers and SHA‑256.
   - `sha256_compress_cuda` is now built from `cuda/sha256.ptx` and is invoked by the `SHA256BLOCK` opcode when GPUs are available. The kernel performs the same integer operations as the CPU reference ensuring bitwise identical output.
   - On macOS, `sha256_compress_metal` is compiled at startup and used when a Metal device is detected. This compute pass mirrors the CPU algorithm exactly so results remain deterministic.
2. Extend to Poseidon, SHA‑3 and AES once initial infrastructure is stable.
3. Add CUDA kernels for signature checks (Ed25519 batching) building on the existing BN254 field pipeline.
4. Integrate a `GpuManager` that tracks available devices and schedules workloads deterministically across them.

By restricting GPU code to deterministic integer operations and committing results in program order, offloading does not alter the VM’s observable behaviour. Nodes without GPUs simply fall back to the existing Rust implementations and produce identical outputs.

## Repository Implementation

The `gpu` module exposes a `GpuManager` used by the scheduler to open CUDA contexts and assign tasks deterministically. GPU selection depends only on the task ID:

```rust
pub fn gpu_for_task(&self, task_id: u64) -> usize {
    (task_id as usize) % self.gpus.len()
}
```

The number of initialised devices is capped by the `IVM_MAX_GPUS` environment variable:

```rust
let max = std::env::var("IVM_MAX_GPUS")
    .ok()
    .and_then(|v| v.parse::<u32>().ok())
    .unwrap_or(count);
let limit = std::cmp::min(count, max);
```

Unit tests verify that setting `IVM_DISABLE_CUDA` disables GPU use and that `IVM_MAX_GPUS` limits the device count:

```rust
#[test]
fn disable_cuda_via_env() {
    std::env::set_var("IVM_DISABLE_CUDA", "1");
    let vm = IVM::new(1_000);
    assert!(!vm.use_cuda, "VM should not enable CUDA when IVM_DISABLE_CUDA is set");
    std::env::remove_var("IVM_DISABLE_CUDA");
}

#[test]
fn limit_gpu_count_env() {
    std::env::set_var("IVM_MAX_GPUS", "1");
    let mgr = GpuManager::init().unwrap();
    assert!(mgr.device_count() <= 1);
    std::env::remove_var("IVM_MAX_GPUS");
}
```

Additional tests use a simple `MockNode` wrapper to run the same program with and without CUDA. By toggling the `IVM_DISABLE_CUDA` variable each node executes either the GPU or CPU path and their outputs are compared:

```rust
struct MockNode { use_cuda: bool }

impl MockNode {
    fn execute(&self, prog: &[u8]) -> [u32; 8] { /* run IVM and return registers */ }
}

#[test]
fn deterministic_across_hardware() {
    let gpu = MockNode { use_cuda: true };
    let cpu = MockNode { use_cuda: false };
    assert_eq!(gpu.execute(&prog), cpu.execute(&prog));
}
```
