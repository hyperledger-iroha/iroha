# GPU Deployment Guide

This guide explains how to enable and operate the optional CUDA acceleration
feature of IVM. The offloading infrastructure is designed to be transparent –
results stay identical to the CPU path – but all nodes in a consensus group must
use homogeneous GPU hardware for deterministic performance.

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

When running on macOS the VM automatically checks for a compatible Metal device.
If found, vector helpers and the SHA‑256 compression round are executed using
Metal compute kernels. No additional build flags are required. Nodes without a
Metal GPU simply fall back to the CPU path and produce identical results.

## Enabling CUDA support

1. Compile IVM with the `cuda` Cargo feature:
   ```bash
   cargo build --release --features cuda
   ```
   Release builds require the qualified checked-in PTX. A pinned CUDA toolkit is
   required only to run the explicit `IVM_CUDA_PTX_MODE=generate` or
   `IVM_CUDA_PTX_MODE=check` qualification path; matching NVIDIA drivers remain
   required at runtime.
2. At runtime the VM automatically detects GPUs and will offload certain vector
   and hashing operations. Set `IVM_DISABLE_CUDA=1` to force CPU execution even
   on systems with GPUs.
3. To restrict the number of GPUs used, set `IVM_MAX_GPUS` to an integer value.
   The VM will initialise at most that many devices in deterministic order.

## Determinism considerations

Consensus-facing CUDA kernels operate on fixed-width integers and avoid
non-deterministic reductions. The backend runs golden-vector parity checks
against the scalar implementation and disables itself on a mismatch. The
legacy `vector_add_f32` diagnostic helper is outside consensus and must be
removed or explicitly classified before the PTX manifest is signed.

For consensus safety every validating node **must** use the same GPU model and
CUDA driver version. Mixing different hardware (for example A100 and H100) is
not recommended as it may lead to subtle timing differences or driver behaviour
changes.

## Operator checklist

- Install matching CUDA drivers on every node.
- Verify the signed PTX manifest and exact checked-in artifact digests.
- Ensure each machine contains the same GPU model and memory size.
- Verify that `IVM_DISABLE_CUDA` is **not** set when GPUs should be utilised.
- Optionally set `IVM_MAX_GPUS` if fewer than all detected devices should be used.
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
