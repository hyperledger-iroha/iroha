# GPU Deployment Guide

This guide explains how to enable and operate the optional CUDA acceleration
feature of IVM. The offloading infrastructure is designed to be transparent –
results stay identical to the CPU path – but all nodes in a consensus group must
use homogeneous GPU hardware for deterministic performance.

Note
- CUDA PTX build integration is automated: `build.rs` compiles `cuda/*.cu`
  with `nvcc` when available, falls back to bundled `.ptx` artifacts when they
  exist, and otherwise emits deterministic stub PTX so the runtime keeps CUDA
  disabled and falls back to CPU paths cleanly. The public CUDA helper surface
  now covers vectors, SHA‑256/Merkle, Keccak, Poseidon2/6, AES rounds/batches,
  BN254 arithmetic, Ed25519 batch verification, and the scheduler bitonic-sort
  helper, with focused fallback/disable-path tests guarding the fail-closed
  behavior.

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
   The CUDA toolkit and matching NVIDIA drivers must be installed.
2. At runtime the VM automatically detects GPUs and will offload certain vector
   and hashing operations. Set `IVM_DISABLE_CUDA=1` to force CPU execution even
   on systems with GPUs.
3. To restrict the number of GPUs used, set `IVM_MAX_GPUS` to an integer value.
   The VM will initialise at most that many devices in deterministic order.

## Determinism considerations

CUDA kernels operate purely on integers and avoid any non-deterministic features
such as atomic floating point operations. Reduction steps are ordered and all
kernels run in IEEE 754 deterministic modes where applicable. As a result, two
nodes executing the same block with identical GPU models will produce bit-for-bit
identical state roots.

For consensus safety every validating node **must** use the same GPU model and
CUDA driver version. Mixing different hardware (for example A100 and H100) is
not recommended as it may lead to subtle timing differences or driver behaviour
changes.

## Operator checklist

- Install matching CUDA drivers on every node.
- Ensure each machine contains the same GPU model and memory size.
- Verify that `IVM_DISABLE_CUDA` is **not** set when GPUs should be utilised.
- Optionally set `IVM_MAX_GPUS` if fewer than all detected devices should be used.
- Monitor logs for `CUDA GPU available` during the IVM startup banner.

## Rollout plan: 8×A100 cluster

1. Prepare a staging environment mirroring production with eight NVIDIA A100
   GPUs per node.
2. Install the NVIDIA driver and CUDA toolkit matching the version used for
   compilation.
3. Build IVM with `cargo build --release --features cuda` and deploy to the
   staging nodes.
4. Run the full `cargo test` suite on the staging hardware to validate the GPU
   paths.
5. Execute a small testnet for several days, comparing block hashes between the
   GPU-enabled staging network and a CPU-only reference network. The hashes must
   match exactly.
6. Measure throughput improvements; expect heavy vector or hashing workloads to
   scale across all eight GPUs.
7. Once performance and determinism are confirmed, repeat the deployment steps
   for production nodes. Ensure all nodes run the same driver and CUDA version.
