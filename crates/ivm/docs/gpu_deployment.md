# GPU Deployment Guide

This guide explains how to enable and operate the optional CUDA acceleration
feature of IVM. The offloading infrastructure is transparent: accelerated
results must match the CPU path byte for byte, so validator nodes do not need
homogeneous GPU hardware for consensus. Homogeneous fleets can still simplify
performance qualification and operations.

Note
- CUDA PTX installation is fail-closed: `build.rs` consumes checked-in PTX by
  default, while explicit `generate` and byte-for-byte `check` modes are
  available on qualified CUDA hosts. It never emits a placeholder or silently
  falls back to a local compiler. The public CUDA helper surface
  now covers vectors, SHA‑256/Merkle, Keccak, Poseidon2/6, AES rounds/batches,
  BN254 arithmetic, Ed25519 batch verification, and the scheduler bitonic-sort
  helper, with focused fallback/disable-path tests guarding the fail-closed
  behavior. The 11 reproducible PTX files and their signed provenance manifest
  are not yet checked in, so the default CUDA build remains a release blocker;
  see [`cuda/README.md`](../cuda/README.md).

## Metal on macOS

Compile IVM with the `metal` Cargo feature on macOS. At runtime the VM checks
for a compatible Metal device when `[accel].enable_metal = true`.
If found, vector helpers and the SHA‑256 compression round are executed using
Metal compute kernels. Nodes without a Metal GPU simply fall back to the CPU
path and produce identical results.

## Enabling CUDA support

1. Compile IVM with the `cuda` Cargo feature:
   ```bash
   cargo build --release --features cuda
   ```
   Release builds require the qualified checked-in PTX. A pinned CUDA toolkit is
   required only to run the explicit `IVM_CUDA_PTX_MODE=generate` or
   `IVM_CUDA_PTX_MODE=check` qualification path; matching NVIDIA drivers remain
   required at runtime.
2. Configure `[accel].enable_cuda = true` (the default) to permit runtime GPU
   discovery and offload eligible vector and hashing operations. Set it to
   `false` for CPU-only operation.
3. Set `[accel].max_gpus` to a positive integer to cap device count, or `0` for
   no cap. The VM initialises at most that many devices in deterministic order.

## Determinism considerations

Consensus-facing CUDA kernels operate on fixed-width integers and avoid
non-deterministic reductions. The backend runs golden-vector parity checks
against the scalar implementation and disables itself on a mismatch. The
legacy `vector_add_f32` diagnostic helper is outside consensus and must be
removed or explicitly classified before the PTX manifest is signed.

Different GPU models, driver versions, or a CPU-only node may have different
performance, but they must produce the same observable VM result. A golden
self-test mismatch disables the affected backend and falls back to CPU.

## Operator checklist

- Install matching CUDA drivers on every node.
- Verify the signed PTX manifest and exact checked-in artifact digests.
- Qualify each deployed GPU/driver combination against the CPU golden vectors.
- Verify `[accel].enable_cuda` is true when GPUs should be used.
- Optionally set `[accel].max_gpus` if fewer than all detected devices should be used.
- Monitor logs for `CUDA GPU available` during the IVM startup banner.

## Rollout plan: 8×A100 cluster

1. Prepare a staging environment mirroring production with eight NVIDIA A100
   GPUs per node.
2. Install the NVIDIA driver and CUDA toolkit matching the version used for
   compilation.
3. In the pinned CUDA environment, run `IVM_CUDA_PTX_MODE=check cargo build
   --release --features cuda`, then deploy a build that consumes those exact
   checked-in PTX bytes.
4. Run the full `cargo test` suite on the staging hardware to validate the GPU
   paths.
5. Execute a small testnet for several days, comparing block hashes between the
   GPU-enabled staging network and a CPU-only reference network. The hashes must
   match exactly.
6. Measure throughput improvements; expect heavy vector or hashing workloads to
   scale across all eight GPUs.
7. Once performance and determinism are confirmed, repeat the deployment steps
   for production nodes. Ensure all nodes run the same driver and CUDA version.
