# IVM hardware acceleration

IVM computes the same public result, gas, errors, state effects, event order,
and verification decision on every hardware class. SIMD is compiled into all
builds and selected by CPU capability. Ordinary macOS builds now compile the
Metal backend by default; the Metal feature has target-scoped dependencies and
is inert on other operating systems. The node's file-loaded `[accel]` policy
enables the available backends by default and provides operator opt-outs and
GPU limits. Developer environment shims are ignored by shipping binaries.

Metal and CUDA may be used only after their own device and kernel admission
checks pass. A backend failure quarantines the affected candidate, discards its
unpublished output, and lets the caller recompute from the original input on a
qualified fallback. Selection and fallback do not change consensus semantics.
For Merkle leaf hashing and root construction, the qualified Metal owner times
bounded public synthetic samples at four sizes. The measurements include block
preparation, buffer transfer, launch, completion, and readback. Every sample is
checked against the canonical CPU result before the profile may select Metal;
selection depends only on public leaf count and the qualified CPU path.
Configured operator floors remain effective. Other helpers still use fixed
workload thresholds; complete signed cost profiles remain open release work.
An individual Keccak permutation or AES round runs on the CPU because its
transfer and launch cost exceeds the work. AES round batches use one GPU batch
dispatch only after the public transfer-size floor; small Ed25519 batches also
stay on the CPU. The direct single-operation GPU entrypoints remain in the
hardware qualification gate. The ARM AES round path now applies the round key
after MixColumns, matching the scalar V1 operation and passing the hardware
parity self-test.

## Metal on macOS

The process retains one charged record for each observed Metal registry identity,
including quarantined devices. Discovery enumerates devices independently within
`resource_limits.discovery_ordinals` and `resource_limits.devices`. The separate
`max_gpus` cap counts healthy initialized or initializing owners; a failed first
GPU does not consume the only active slot. A complete bounded enumeration also
quarantines previously observed identities that have disappeared; a truncated
inventory cannot establish absence. Reappearing identities retain their original
quarantine. `Some(0)` prevents discovery and native
entry. Concurrent borrowers retain their original pipeline, health and allocation
owners. Operator opt-outs and discovery restarts never clear physical quarantine
or forgive uncertain command/buffer charges.

A private device lease pins public synthetic calibration and the operation it
selects, including both stages of a Merkle root. Merkle, AES batch and Ed25519
batch selection compares qualified costs across eligible devices and retains the
CPU comparison and configured workload floors. Costs include preparation,
transfer, launch, completion and readback. New calibration shares one sampling
deadline across devices. Discovery and each public calibration family retain a
nonblocking fair cursor across passes, advancing before an actual attempt. A slow
transient failure cannot repeatedly take the first turn. Candidates deferred by
an exhausted budget or a busy calibration owner receive no new retry penalty;
cached profiles remain comparable when the sampling deadline expires. If a native
callback unwinds, subsequent discovery/calibration recovers only its scalar
scheduler timestamp and cursor, preserving the recorded cooldown and next turn.
Contention still declines immediately. Physical quarantine and coupled registry
or cost-profile mutexes are not reset by scheduler recovery. An explicit discovery
restart retains its existing meaning of clearing only the retry timestamp. Discovery and calibration check time budgets between native operations;
a single command retains its existing ten-second completion bound.

Rust registry storage is reserved from the shared process metadata envelope before
allocation. Admission, losing construction and refunds occur outside registry
writers. The initial physical record capacity is retained for the process
lifetime; increasing that storage ceiling requires a process restart. Native
buffer backing and command permits retain their original shared resource charges.
A command failure quarantines only its original physical device. Uncertain work
retains native storage and permits; another healthy device can continue within
remaining shared capacity. Destination copies happen only after exact-owner
acceptance, so declined work recomputes from its original input. AES attempt output
retains and rebinds its selected device through the final copy, including direct
API calls entered without an existing thread-local physical binding.

The 16 production pipeline families cover vector arithmetic and bit operations,
SHA-256 compression and Merkle helpers, Keccak, AES round/batch helpers, and
Ed25519 verification. Startup probes and diagnostic commands produce no production
execution receipt.
Run the required hardware gate on a Metal host:

```sh
cargo test --locked -p ivm --features metal-hardware-tests --lib required_metal_hardware -- --test-threads=1 --nocapture
```

The gate runs in an isolated process and requires the complete allowed native
identity inventory to have charged records. It fails when Metal is absent, an
observed device cannot qualify, a pipeline falls back to CPU, or parity breaks.
For every recorded device it compares all 16 families against CPU references at
several sizes, including odd Merkle reductions and valid and invalid signatures.
`metal_completed_dispatches(MetalKernel)` exposes aggregate production counts;
the gate also requires positive counts bound to each exact physical identity. Completion alone does not
establish correctness, so the scalar comparisons are mandatory. Local M1 Ultra
evidence is recorded in `target/kotodama-metal-local-evidence.json`; that
changing-worktree component run is not an unchanged release-candidate result.

An additional unbound direct AES control requires true accepted output and exact
physical counters for all four batch families, for both destination and in-place
APIs. This prevents an outer qualification binding from concealing an escaped
output owner's acceptance failure:

```sh
cargo test --locked -p ivm --features metal-hardware-tests --lib vector::metal_qualification::required_metal_unbound_aes_keeps_exact_owner_through_copy -- --exact --nocapture
```

The separate two-device destructive control requires actual hardware and cannot
pass on a one-device runner:

```sh
cargo test --locked -p ivm --features metal-hardware-tests --lib vector::metal_qualification::required_metal_two_devices_isolate_quarantine -- --exact --nocapture
```

It checks uncertain backing/permit retention, zero-cap refusal, sticky quarantine
across policy reload, and actual second-device dispatch with `max_gpus = 1`.

Metal release qualification still needs signed artifact provenance, authenticated
benchmark profiles, coverage for other kernels, device/driver/artifact matrix
evidence, and mixed-hardware validator parity. Metal physical failures currently
quarantine the affected device; finer independent kernel quarantine remains open. The process owner loads an embedded precompiled V1 Metal
library and performs no startup shader compilation. The local optional Xcode
Metal toolchain built the pinned candidate; release signing and unchanged-
candidate device qualification remain open.

## CUDA artifacts and runtime

The patched `cust_raw` boundary loads the CUDA driver at runtime. A CPU-only
machine can start a CUDA-enabled binary without a driver. Linux loads
`libcuda.so.1` (including the WSL system path); Windows restricts
`nvcuda.dll` to System32. Missing drivers or symbols leave the CPU path
available. CUDA is not yet a default Cargo feature because the required ten
checked-in PTX files and signed provenance are absent.

`build.rs` has three explicit build-time modes:

- `bundled` (default for CUDA builds) copies validated checked-in PTX and fails
  if any family is missing.
- `generate` invokes `nvcc` only for a qualification candidate.
- `check` regenerates every family and requires byte identity with its bundled
  artifact.

The ten source families are AES, bitonic sort, BN254, Poseidon, SHA-256,
SHA-256 leaves, SHA-256 pair reduction, SHA-3, signature, and vector. The
retired floating-point diagnostic kernel is outside this inventory. Startup
never downloads or compiles kernels. See [the CUDA artifact record](../cuda/README.md)
for the open pinned-toolchain, two-run reproducibility, signed provenance, and
real-hardware gates.

The common process acceleration envelope owns CUDA discovery, primary contexts,
modules, streams, host staging, pinned buffers and device buffers. File-loaded
`[accel.resource_limits]` supplies finite enabled defaults; explicit zero forbids
that resource. Admission never waits for capacity. A refused attempt leaves the
original input available for a complete CPU recomputation. State destinations
retain their original execution lease; native SDK snapshots and outputs use
charged process-host owners until the final foreign-runtime copy.

Discovery treats devices independently and checks public launch geometry against
capabilities. Kernel failures exclude the device/kernel/artifact candidate;
uncertain context, stream or completion state quarantines the physical device.
A timed-out attempt retains unsafe-to-free storage and its charges. Late driver
completion cannot undo that quarantine. Task scopes retain their selected device
through compound operations. There is no second GPU manager or parallel
admission registry. Native batch helpers fill caller destinations only after
complete result validation; ordinary helpers handle qualified CPU fallback.

IVM Metal no-copy buffers, command permits and Rust registry records use this
common resource envelope. These controls do not establish FASTPQ Metal physical
custody or remove its separate runtime-compilation path. CUDA cost ranking,
authenticated performance profiles, complete fastest-path coverage, opaque native
pipeline accounting and physical resource qualification remain open.

Run the mandatory CUDA hardware gate on each qualified CUDA runner:

```sh
cargo test --locked -p ivm --test cuda_hardware --features cuda-hardware-tests -- --nocapture
```

It requires real completed kernel work on every discovered usable device and
scalar parity for all ten families. Driverless policy tests establish loading
and state transitions only; they do not qualify GPU execution. The same
candidate must then pass CPU/SIMD/Metal/CUDA parity and a four-validator
mixed-hardware network with mandatory signed RS16 payload availability before
release.
