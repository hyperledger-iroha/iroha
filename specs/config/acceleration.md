# Acceleration configuration and resource custody

The first release has one file-owned `[accel]` policy. Production does not read
`ACCEL_*` environment aliases. SIMD, Metal and CUDA default to enabled; actual
availability requires compiled support, discovered capability and successful
operation/artifact qualification. Hardware selection cannot change execution
results, gas, effects or verification decisions.

| Key | Default | Meaning |
| --- | --- | --- |
| `enable_simd`, `enable_metal`, `enable_cuda` | `true` | Permit the qualified backend. |
| `max_gpus` | omitted | IVM device-selection cap. Zero explicitly opts out. |
| `merkle_min_leaves_gpu` | 8192 | Generic GPU Merkle threshold. |
| `merkle_min_leaves_metal`, `merkle_min_leaves_cuda` | omitted | Inherit the generic threshold. |
| `prefer_cpu_sha2_max_leaves_aarch64`, `prefer_cpu_sha2_max_leaves_x86` | omitted | Inherit the compiled 32768-leaf CPU preference. |

Optional counts distinguish omission from explicit zero. No zero-as-auto decoder
or alternate production environment representation is retained. Thresholds are
resource/selection policy, not evidence that a path is fastest for a workload.

`[accel.resource_limits]` configures the separate process acceleration-attempt
envelope. Its ordinary finite defaults do not require a performance profile:

| Key | Default | Accounted scope |
| --- | ---: | --- |
| `host_bytes` | 268435456 | Ordinary host backing, including staged output. |
| `pinned_bytes` | 268435456 | Requested pinned host backing. |
| `device_bytes` | 1073741824 | Requested device backing across attempts. |
| `in_flight` | 16 | Complete prepared, submitted or uncertain work owners. |
| `metadata_bytes` | 16777216 | Variable registry and policy control backing. |
| `observed_devices` | 16 | Lifetime-observed physical records, including quarantine. |
| `discovery_ordinals` | 64 | Ordinals probed during a discovery pass. |
| `modules` | 304 | Retained opaque native module owners. |
| `streams` | 16 | Retained opaque native stream owners. |
| `artifact_bytes` | 16777216 | Immutable artifact bytes, including terminal NUL. |

Zero denies admission of the specified resource. These are explicit policy
ceilings, not free-memory fractions or measurements of allocator overhead or
opaque driver-private storage. Native driver/context/module memory remains
unmeasured and has finite owner-count bounds. Immutable device capabilities also
bound admitted requested bytes and launch geometry. Per-device requested-byte
permission does not charge the same allocation twice.

One `iroha_accel::ProcessResources` holds the original process pools. Reload
updates admission ceilings without refunding retained allocations or replacing
physical device identity/quarantine. A registry cannot grow its original fixed
record backing during reload. Capacity pressure selects local CPU fallback;
execution never waits for another attempt or a parent execution to release
capacity. Brief configuration/accounting synchronization remains.

Caller-owned output storage comes from its original `ExecutionMemoryLease`.
The process envelope does not duplicate or recreate that State budget. Native
attempts preserve original inputs and retain staged output charge until the full
successful copy into the caller destination. Failure cannot expose partial GPU
output; the caller recomputes from its original inputs.

The native bridge uses the single current C record in
`crates/connect_norito_bridge/include/connect_norito_bridge.h`. It carries every
resource limit and returns an error before mutation for malformed flags,
noncanonical optional counts or values outside the native count width. A null
setter input restores enabled defaults. A successful setter applies requested
policy; runtime status/readback establishes backend availability.

CUDA vector consumers use the shared physical owner. Remaining CUDA/FASTPQ and
Metal consumer custody, signed artifact provenance, profile selection, physical
kernel parity/counters, and mixed-hardware network qualification remain explicit
release gates until their corresponding implementations and evidence land.
