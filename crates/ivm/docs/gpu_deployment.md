# GPU deployment and qualification

IVM must produce the same results, gas, errors and commitments on CPU and GPU
nodes. SIMD selection is automatic. Ordinary macOS builds include Metal; the
file-loaded `[accel]` configuration enables available backends by default and
provides operator opt-outs and device limits. CPU-only nodes need no GPU driver.

## Current artifact gates

Metal now loads one embedded precompiled V1 library; startup performs no shader
compilation or download. Its exact bytes, source inventory, and one-host
reproduction are pinned in [the Metal artifact record](../metal/README.md).
The local Xcode optional Metal toolchain produced this candidate. Release
signing and exact-candidate device qualification remain open. Metal Merkle leaf and
root work uses a bounded, parity-checked public synthetic cost profile that
includes transfer and launch cost; other helpers retain fixed workload floors.
An incomplete or noisy calibration uses the CPU path and retries after a bounded
cooldown. A parity mismatch quarantines Metal. A changed CPU/SIMD policy
invalidates the old profile before another path decision.

CUDA loads its driver at runtime, so missing driver libraries do not prevent a
binary from starting. CUDA is not yet a default Cargo feature because the ten
required checked-in PTX files and their signed provenance are missing. A CUDA
build fails closed when bundled PTX is absent. Explicit `generate` and `check`
build modes are reserved for qualification with a pinned CUDA toolkit; shipping
binaries do not generate PTX on startup. See [the CUDA artifact record](../cuda/README.md)
and [hardware behavior](gpu_offloading.md).

## Operator policy

`[accel].enable_metal` and `[accel].enable_cuda` default to true. Set either to
false to disable that backend. `[accel].max_gpus` caps usable discovered devices;
zero leaves the count uncapped. Device or kernel admission failures quarantine
the affected choice, and callers recompute from original inputs on a qualified
fallback before publishing a result. Different hardware may change throughput
but never transaction validity or gas.

## Required release checks

1. Produce and verify reproducible, signed precompiled Metal libraries and all
   ten CUDA PTX families for the exact release source and toolchains.
2. Run the required real Metal and CUDA execution gates on each supported
   device, driver and kernel combination. Every family needs a positive
   completion receipt and CPU parity at representative sizes.
3. Confirm identical witness/output bytes where required, plus identical
   result, gas, error, state, event and commitment behavior across CPU, SIMD,
   Metal and CUDA.
4. Run a mixed-hardware four-validator network with mandatory signed RS16
   DA/RBC availability, failure/quarantine injection and restart recovery.

These physical and artifact checks are open; local component tests do not
qualify an unchanged release candidate.
