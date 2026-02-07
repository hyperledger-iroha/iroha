---
lang: uz
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T19:17:13.237630+00:00"
translation_last_reviewed: 2026-02-07
---

# IVM Bytecode Header


Magic
- 4 bytes: ASCII `IVM\0` at offset 0.

Layout (current)
- Offsets and sizes (17 bytes total):
  - 0..4: magic `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8` (feature bits; see below)
  - 7: `vector_length: u8`
  - 8..16: `max_cycles: u64` (little‑endian)
  - 16: `abi_version: u8`

Mode bits
- `ZK = 0x01`, `VECTOR = 0x02`, `HTM = 0x04` (reserved/feature‑gated).

Fields (meaning)
- `abi_version`: syscall table and pointer‑ABI schema version.
- `mode`: feature bits for ZK tracing/VECTOR/HTM.
- `vector_length`: logical vector length for vector ops (0 → unset).
- `max_cycles`: execution padding bound used in ZK mode and admission.

Notes
- Endianness and layout are defined by the implementation and bound to `version`. The on‑wire layout above reflects the current implementation in `crates/ivm_abi/src/metadata.rs`.
- A minimal reader can rely on this layout for current artifacts and should handle future changes via `version` gating.
- Hardware acceleration (SIMD/Metal/CUDA) is opt-in per host. The runtime reads `AccelerationConfig` values from `iroha_config`: `enable_simd` forces scalar fallbacks when false, while `enable_metal` and `enable_cuda` gate their respective backends even when compiled in. These toggles are applied through `ivm::set_acceleration_config` before VM creation.
- Mobile SDKs (Android/Swift) surface the same knobs; `IrohaSwift.AccelerationSettings`
  calls `connect_norito_set_acceleration_config` so macOS/iOS builds can opt into Metal /
  NEON while keeping deterministic fallbacks.
- Operators can also force-disable specific backends for diagnostics by exporting `IVM_DISABLE_METAL=1` or `IVM_DISABLE_CUDA=1`. These environment overrides take precedence over configuration and keep the VM on the deterministic CPU path.

Durable state helpers and ABI surface
- The durable state helper syscalls (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* and JSON/SCHEMA encode/decode) are part of the V1 ABI and are included in `abi_hash` computation.
- CoreHost wires STATE_{GET,SET,DEL} to WSV-backed durable smart-contract state; dev/test hosts may use overlays or local persistence but must preserve the same observable behavior.

Validation
- Node admission accepts only `version_major = 1` and `version_minor = 0` headers.
- `mode` must only contain known bits: `ZK`, `VECTOR`, `HTM` (unknown bits are rejected).
- `vector_length` is advisory and may be non‑zero even if the `VECTOR` bit is not set; admission enforces an upper bound only.
- Supported `abi_version` values: first release accepts only `1` (V1); other values are rejected at admission.

### Policy (generated)
The following policy summary is generated from the implementation and should not be edited manually.

<!-- BEGIN GENERATED HEADER POLICY -->
| Field | Policy |
|---|---|
| version_major | 1 |
| version_minor | 0 |
| mode (known bits) | 0x07 (ZK=0x01, VECTOR=0x02, HTM=0x04) |
| abi_version | 1 |
| vector_length | 0 or 1..=64 (advisory; independent of VECTOR bit) |
<!-- END GENERATED HEADER POLICY -->

### ABI Hashes (generated)
The following table is generated from the implementation and lists canonical `abi_hash` values for supported policies.

<!-- BEGIN GENERATED ABI HASHES -->
| Policy | abi_hash (hex) |
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- Minor updates may add instructions behind `feature_bits` and reserved opcode space; major updates may change encodings or remove/repurpose only together with a protocol upgrade.
- Syscall ranges are stable; unknown for the active `abi_version` yields `E_SCALL_UNKNOWN`.
- Gas schedules are bound to the `version` and require golden vectors on change.

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
