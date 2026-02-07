---
lang: az
direction: ltr
source: docs/source/gas_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a2e92d81f17dbd015894a9b61f6acc40d4116a06aefe476a9f8d0ba4d6d3955
source_last_modified: "2026-01-30T18:06:03.184151+00:00"
translation_last_reviewed: 2026-02-07
---

# IVM Gas Model

This document defines the canonical gas schedule for the Iroha Virtual Machine
(IVM) and explains how the schedule is hashed and applied. The source of truth
for costs is `crates/ivm/src/gas.rs`; the schedule table below is a rendered
view of that canonical mapping.

## Scope

- Applies to IVM bytecode execution (Executable::Ivm).
- Native ISI gas metering is defined separately in `crates/iroha_core/src/gas.rs`.
- ISO 20022 opcodes are reserved in ABI v1 and do not carry gas entries yet.

## Determinism and schedule hash

The gas schedule is deterministic and derived from opcode → cost pairs. The
canonical digest is computed over the ordered opcode list with each entry
serialized as:

- opcode byte (u8)
- cost (u64, little-endian)

The hash is exposed by:

- `ivm::gas::schedule_hash()` (canonical schedule hash)
- `ivm::limits::schedule_hash()` (host-facing alias)

Use this digest to verify that all peers share the same schedule when wiring
config or telemetry checks.

## Vector scaling and HTM retries

- Vector ops (`VADD*`, `VAND`, `VXOR`, `VOR`, `VROT32`) scale with the logical
  vector length set by `SETVL`. The base costs in the table are scaled by
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES` (baseline = 2 lanes).
- HTM retries multiply the cost by `(retries + 1)`; most consensus paths do not
  incur retries.

## Canonical opcode gas table

The table below lists the base costs used by `ivm::gas::cost_of`. Vector scaling
and HTM retries are applied on top of these base values as noted above.

| Category | Opcode | Mnemonic | Base gas |
|---|---:|---|---:|
| arithmetic | 0x01 | `ADD` | 1 |
| arithmetic | 0x02 | `SUB` | 1 |
| arithmetic | 0x03 | `AND` | 1 |
| arithmetic | 0x04 | `OR` | 1 |
| arithmetic | 0x05 | `XOR` | 1 |
| arithmetic | 0x06 | `SLL` | 1 |
| arithmetic | 0x07 | `SRL` | 1 |
| arithmetic | 0x08 | `SRA` | 1 |
| arithmetic | 0x0D | `NEG` | 1 |
| arithmetic | 0x0C | `NOT` | 1 |
| arithmetic | 0x20 | `ADDI` | 1 |
| arithmetic | 0x21 | `ANDI` | 1 |
| arithmetic | 0x22 | `ORI` | 1 |
| arithmetic | 0x23 | `XORI` | 1 |
| arithmetic | 0x10 | `MUL` | 3 |
| arithmetic | 0x11 | `MULH` | 3 |
| arithmetic | 0x12 | `MULHU` | 3 |
| arithmetic | 0x13 | `MULHSU` | 3 |
| arithmetic | 0x14 | `DIV` | 10 |
| arithmetic | 0x15 | `DIVU` | 10 |
| arithmetic | 0x16 | `REM` | 10 |
| arithmetic | 0x17 | `REMU` | 10 |
| arithmetic | 0x18 | `ROTL` | 2 |
| arithmetic | 0x19 | `ROTR` | 2 |
| arithmetic | 0x25 | `ROTL_IMM` | 2 |
| arithmetic | 0x26 | `ROTR_IMM` | 2 |
| arithmetic | 0x1A | `POPCNT` | 6 |
| arithmetic | 0x1B | `CLZ` | 6 |
| arithmetic | 0x1C | `CTZ` | 6 |
| arithmetic | 0x1D | `ISQRT` | 6 |
| arithmetic | 0x1E | `MIN` | 1 |
| arithmetic | 0x1F | `MAX` | 1 |
| arithmetic | 0x27 | `ABS` | 1 |
| arithmetic | 0x28 | `DIV_CEIL` | 12 |
| arithmetic | 0x29 | `GCD` | 12 |
| arithmetic | 0x2A | `MEAN` | 2 |
| arithmetic | 0x09 | `SLT` | 2 |
| arithmetic | 0x0A | `SLTU` | 2 |
| arithmetic | 0x0E | `SEQ` | 2 |
| arithmetic | 0x0F | `SNE` | 2 |
| arithmetic | 0x0B | `CMOV` | 3 |
| arithmetic | 0x24 | `CMOVI` | 3 |
| memory | 0x30 | `LOAD64` | 3 |
| memory | 0x31 | `STORE64` | 3 |
| memory | 0x32 | `LOAD128` | 5 |
| memory | 0x33 | `STORE128` | 5 |
| control | 0x40 | `BEQ` | 1 |
| control | 0x41 | `BNE` | 1 |
| control | 0x42 | `BLT` | 1 |
| control | 0x43 | `BGE` | 1 |
| control | 0x44 | `BLTU` | 1 |
| control | 0x45 | `BGEU` | 1 |
| control | 0x46 | `JAL` | 2 |
| control | 0x48 | `JALR` | 2 |
| control | 0x47 | `JR` | 2 |
| control | 0x4A | `JMP` | 2 |
| control | 0x4B | `JALS` | 2 |
| control | 0x49 | `HALT` | 0 |
| system | 0x60 | `SCALL` | 5 |
| system | 0x61 | `GETGAS` | 0 |
| crypto | 0x70 | `VADD32` | 2 |
| crypto | 0x71 | `VADD64` | 2 |
| crypto | 0x72 | `VAND` | 1 |
| crypto | 0x73 | `VXOR` | 1 |
| crypto | 0x74 | `VOR` | 1 |
| crypto | 0x75 | `VROT32` | 1 |
| crypto | 0x76 | `SETVL` | 1 |
| crypto | 0x77 | `PARBEGIN` | 0 |
| crypto | 0x78 | `PAREND` | 0 |
| crypto | 0x80 | `SHA256BLOCK` | 50 |
| crypto | 0x81 | `SHA3BLOCK` | 50 |
| crypto | 0x82 | `POSEIDON2` | 10 |
| crypto | 0x83 | `POSEIDON6` | 10 |
| crypto | 0x84 | `PUBKGEN` | 50 |
| crypto | 0x85 | `VALCOM` | 50 |
| crypto | 0x86 | `ECADD` | 20 |
| crypto | 0x87 | `ECMUL_VAR` | 100 |
| crypto | 0x8E | `PAIRING` | 500 |
| crypto | 0x88 | `AESENC` | 30 |
| crypto | 0x89 | `AESDEC` | 30 |
| crypto | 0x8A | `BLAKE2S` | 40 |
| crypto | 0x8B | `ED25519VERIFY` | 1000 |
| crypto | 0x8F | `ED25519BATCHVERIFY` | 500 |
| crypto | 0x8C | `ECDSAVERIFY` | 1500 |
| crypto | 0x8D | `DILITHIUMVERIFY` | 5000 |
| zk | 0xA0 | `ASSERT` | 1 |
| zk | 0xA1 | `ASSERT_EQ` | 1 |
| zk | 0xA2 | `FADD` | 1 |
| zk | 0xA3 | `FSUB` | 1 |
| zk | 0xA4 | `FMUL` | 3 |
| zk | 0xA5 | `FINV` | 5 |
| zk | 0xA6 | `ASSERT_RANGE` | 1 |

