# IVM Bytecode Header


Magic
- 4 bytes: ASCII `IVM\0` at offset 0.

Layout (per engine)

The execution root's selected host profile fixes the engine before the header is parsed, so a
reader never tries both layouts. ABI V1, the interface profile, is the frozen baseline engine
([IVM baseline and C10 host profiles](runtime/ivm_baseline_and_c10_profiles.md)); the C10
engine serves only explicitly selected profiles.
<!-- BEGIN GENERATED HEADER LAYOUT -->
- ABI V1, the interface profile (frozen baseline engine): 49 bytes total
  - 0..4: magic `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8` (feature bits; see below)
  - 7: `vector_length: u8`
  - 8..16: `max_cycles: u64` (little‑endian cycle limit; nonzero)
  - 16: `abi_version: u8`
  - 17..49: `abi_hash: [u8; 32]` (Iroha Hash v1 commitment to the canonical descriptor of the profile)
- C10 profiles (`iroha.ivm.runtime_lifecycle.v1`, and C10 ABI V1 when selected explicitly): 48 bytes total
  - 0..4: magic `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8` (feature bits; see below)
  - 7..15: `max_cycles: u64` (little‑endian ZK trace bound; zero without `ZK`)
  - 15: `abi_version: u8`
  - 16..48: `abi_hash: [u8; 32]` (Iroha Hash v1 commitment to the canonical descriptor of the profile)
<!-- END GENERATED HEADER LAYOUT -->

Mode bits
- Baseline: `ZK = 0x01`, `VECTOR = 0x02`.
- C10: `ZK = 0x01`. It is the only mode bit.

Fields (meaning)
- `abi_version`: syscall table and pointer‑ABI schema version.
- `abi_hash`: authenticated Iroha Hash v1 commitment (Blake2b-256 with the
  final byte's least-significant bit set to 1) to the exact canonical ABI
  host descriptor selected by `abi_version` and the execution root's exact host
  profile; admission validates it before prefix or instruction decoding.
- `mode`: feature bits; `ZK` enables privacy tags, private-memory tracking and trace padding;
  the baseline `VECTOR` bit enables vector execution.
- `vector_length` (baseline only): logical vector length for vector ops (0 selects the runtime
  default).
- `max_cycles`, baseline: the program's cycle limit. It is required (nonzero), bounded by the
  `smart_contract` fuel parameter and the node's `ivm_max_cycles_upper_bound`, converts into the
  gas limit at the maximum instruction cost, and nested calls share the parent's cycle allowance.
- `max_cycles`, C10: the ZK trace bound. A ZK program traps `ExceededMaxCycles` when its executed
  cycles reach it and pads a successful trace to exactly this length (one gas per padded cycle).
  It is nonzero exactly when `mode` has the `ZK` bit; a header without `ZK` must carry `0`. Gas is
  the only C10 execution budget: the caller supplies it, nested invocations draw from the
  parent's remaining gas, and no header field converts into gas.

Notes
- Endianness and layout are defined by the implementation and bound to `version` and the selected engine. The on‑wire layouts above reflect `crates/ivm_abi/src/baseline.rs` (baseline) and `crates/ivm_abi/src/metadata.rs` (C10).
- A minimal reader can rely on this layout for current artifacts and should handle future changes via `version` gating.
- Hardware acceleration (SIMD/Metal/CUDA) is enabled by default when compiled and available. The runtime reads `AccelerationConfig` values from `iroha_config`: `enable_simd` forces scalar fallbacks when false, while `enable_metal` and `enable_cuda` gate their respective backends even when compiled in. These toggles are applied through `ivm::set_acceleration_config` before VM creation, and backend status only reports parity as OK after policy, hardware detection, and golden self-tests all pass.
- Mobile SDKs (Android/Swift) surface the same policy. `IrohaSwift.AccelerationSettings`
  calls `connect_norito_acceleration_config_set_v1` with the complete current record
  and its exact length; config/state readback uses `connect_norito_acceleration_config_get_v1`
  and `connect_norito_acceleration_state_get_v1`. Size-less exports are retired.
- Operator opt-outs and finite process ceilings come from `[accel]` and
  `[accel.resource_limits]` file configuration. Runtime environment variables do
  not select acceleration policy. Unsupported or unqualified devices use the
  deterministic CPU path; policy acceptance does not establish device qualification.

Durable state helpers and ABI surface
- The V1 ABI hash covers the durable State helper syscalls, canonical JSON/SCHEMA operations, and generic `POINTER_TO_NORITO`/`POINTER_FROM_NORITO` transport. Kotodama `int` uses canonical signed-512 `IntValueV1` frames.
- CoreHost wires STATE_{GET,SET,DEL} to WSV-backed durable smart-contract state; dev/test hosts may use overlays or local persistence but must preserve the same observable behavior.

Validation
- The fixed header `abi_hash` must equal the canonical 32-byte hash of the
  selected host descriptor. ABI V1 (baseline) and `iroha.ivm.runtime_lifecycle.v1`
  (C10) use `abi_version = 1` with distinct exact hashes. Live admission parses
  under the profile the execution root selects; the C10 convenience parsers of the
  `ivm` crate read only the C10 layout. Unknown hashes never select a fallback. The parser validates this field
  before decoding `CNTR`, any other prefix section, or the instruction stream.
- For deployable artifacts, the required `CNTR` section carries the same ABI
  hash. Admission requires the header's `abi_version`/`abi_hash` and the
  embedded `CNTR.abi_hash` to resolve to the same runtime descriptor, so both
  the fixed header and `CNTR` bind the artifact to the ABI. The on-chain manifest
  must match too. The runtime lifecycle host profile requires deployable `CNTR`
  metadata and is unavailable to generic programs and local test-harness artifacts.
- Every first-release execution profile requires `version_major = 1` and
  `version_minor = 1`. Header `1.0` is rejected before section decoding. Generic
  programs and explicit local test harnesses share this header; CNTR presence and
  the test loader capability determine their admission profile.
- Deployable contract artifacts must embed a `CNTR` section immediately after
  the fixed header and are rejected if that section is missing or inconsistent
  with the executable stream. Embedded `DBG1` metadata is forbidden for
  deployable contracts; source maps and debug data belong in hash-keyed
  sidecars. The generic metadata parser may still locate a structurally valid
  `DBG1` section for low-level tooling.
- If a `LTLB` literal table is present after the fixed header, or after the
  required `CNTR` section in a deployable artifact,
  its post-table padding must be canonical for the section offset: at most three
  zero bytes, exactly the alignment length implied by the literal header,
  entries, and data.
- `LTLB` contains a 16-byte header (`"LTLB"`, `count: u32`, `post_pad: u32`,
  `data_len: u32`), followed by `count` little-endian `u64` descriptors, the
  packed literal data, and canonical zero padding. A descriptor's high byte is
  its kind (`0` = pointer TLV, `1` = signed `i64`) and its low 56 bits are an
  offset relative to the `LTLB` marker. ABI v1 permits at most 65,536 entries.
  Descriptor targets must begin at the first data byte, be strictly increasing,
  and partition the complete data range without gaps or aliases. Pointer entries
  must be one exact, checksum-valid ABI-v1 TLV; scalar entries must be exactly
  eight little-endian bytes. The loader rejects unknown kinds, malformed
  payloads, out-of-range indices, and `LDLIT`/`LDI64` kind confusion before
  execution. Pointer provenance accepts only exact validated pointer-entry
  starts; scalar values, instruction bytes, headers, and interior addresses are
  never pointer objects.
- Generic and deployable `mode` parsing permits only `ZK` and `VECTOR` under the
  baseline and only `ZK` under C10; every other bit is rejected.
- Baseline: `vector_length` is `0` or `1..=64`; `0` selects the runtime default and the field
  may be non-zero even if the `VECTOR` bit is not set. `max_cycles` must be nonzero and within
  both the fuel parameter and `ivm_max_cycles_upper_bound`.
- C10: `max_cycles` must be nonzero exactly when `ZK` is set (`MissingMaxCycles`,
  `MaxCyclesWithoutZk`); node policy bounds a ZK trace bound by
  `ivm_max_cycles_upper_bound`.
- Supported `abi_version` values: first release accepts only `1` (V1); other values are rejected at admission.

### Policy (generated)
The following policy summary is generated from the implementation and should not be edited manually.

<!-- BEGIN GENERATED HEADER POLICY -->
| Field | ABI V1 (baseline) | C10 profiles |
|---|---|---|
| version_major | 1 | 1 |
| version_minor | 1 | 1 |
| mode (known bits) | 0x03 (ZK=0x01, VECTOR=0x02) | 0x01 (ZK=0x01) |
| vector_length | 0 or 1..=64 (0 selects runtime default; independent of VECTOR bit) | absent |
| max_cycles | nonzero cycle limit, bounded by fuel and the node ceiling | nonzero iff ZK (ZK trace bound; gas is the only budget) |
| abi_version | 1 | 1 |
<!-- END GENERATED HEADER POLICY -->

### ABI Hashes (generated)
The following table is generated from the implementation and lists canonical `abi_hash` values for supported policies.

The hash binds indexed-literal opcode/kind/layout semantics, the sorted allowed
syscall numbers, their canonical argument and return signatures, each syscall's
conservative host-access class, the sorted allowed pointer-ABI type IDs, exact
numeric domains/rules/JSON grammars/fault ordering, and typed durable-state
schema identities, enum tags, layouts, pointer mappings, traversal rules, and
caps. It also binds the Generic-program discriminator, reserved transaction
metadata, exact contract-bound syscall denylist, and rejection phase. Gas
prices are deliberately excluded; `ivm::gas::schedule_hash()` commits
to the canonical gas schedule, including every staged-metering phase name and
tag, separately. The descriptor-format revision does not introduce ABI v2:
first-release artifacts still declare `abi_version = 1` and stale hashes fail
closed.

<!-- BEGIN GENERATED ABI HASHES -->
| Policy | abi_hash (hex) |
|---|---|
| ABI v1 | 7531b45dd9d79aabc3a0f64ab30a053af847dfe248078cff839ab2e14e3261a7 |
| iroha.ivm.runtime_lifecycle.v1 | 76f057f25a3ba9a1525dc9b066de73c982f49c2ffd8180e65ddaeeb64d3515af |
<!-- END GENERATED ABI HASHES -->

- ABI V1 is the sole first-release version. Each engine fixes its own instruction
  encodings (the baseline's, or C10's for the explicitly selected C10 profiles);
  the pointer encodings are shared. Its `LDLIT`, `LDI64`, `JAL`, `JMP`, and
  `JALS` extensions are unconditional rather than feature-gated. A future
  post-release encoding break requires an explicit protocol/ABI upgrade; it
  must not silently reinterpret ABI-v1 opcode space.
- Syscall ranges are stable; unknown for the active `abi_version` yields `E_SCALL_UNKNOWN`.
- Gas schedules are committed independently by `ivm::gas::schedule_hash()` and
  require golden vectors on change.

Inspecting artifacts
- Use `ivm_tool inspect <file.to>` for a stable view of header fields.
- For development, examples/ include a small Makefile target `examples-inspect` that runs inspect over built artifacts.

Example (Rust): minimal magic + size check

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

Note: The exact header layout beyond the magic is versioned and implementation‑defined; prefer `ivm_tool inspect` for stable field names and values.
