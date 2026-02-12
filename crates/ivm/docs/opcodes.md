# Instruction Set Overview

This document lists the opcodes understood by the Iroha Virtual Machine. Each
opcode is represented as a constant grouped by category in
[`instruction.rs`](../src/instruction.rs) and documented in the generated API
reference. The instructions are grouped by category for convenience.

IVM's opcodes allow building arbitrary loops and data transformations, ensuring the instruction set is Turing complete. Every program is limited by its gas budget, providing deterministic halting. Register and memory operations use 64-bit words optimized for fast financial calculations.

Bulk helpers like `load_bytes` and `store_bytes` enable efficient block copies used by hashing instructions.

Conventions and notes
- Hex values in the tables refer to the primary opcode byte in the wide encoding (8-bit).
- Mode gating: vector/SIMD instructions execute only when the program header `VECTOR` bit is set; otherwise they deterministically trap with a mode-disabled error. ZK-specific instructions execute only when the `ZK` bit is set. Reserved HTM instructions are disabled unless an explicit feature/mode enables them.
- Pointer-ABI: instructions that dereference Norito TLV pointers (e.g., signature verify opcodes) follow the pointer-ABI documented in `syscalls.md`/`tlv_examples.md`. Hosts/VM validate TLVs on first dereference and trap on invalid envelopes.
- Memory: all loads/stores require natural alignment; region permissions (INPUT/OUTPUT/CODE/HEAP/STACK) are enforced uniformly. Misaligned accesses deterministically trap with `MisalignedAccess`.
- Control flow: `JALR` masks the low two bits of the computed target so control transfers are 4-byte aligned.
- See also: `instruction.rs` API docs for authoritative field extractors and masks used by encoders/decoders.
## Encoding Format

IVM v1.1 standardises on a single 32‑bit word layout with an 8‑bit primary opcode and three 8‑bit operand slots. Helpers in `encoding::wide` and `instruction::wide` provide canonical encoders/decoders for this layout, and all code emission in the Kotodama compiler and reference tooling uses these helpers exclusively.

```
31       24 23       16 15        8 7         0
+----------+-----------+-----------+-----------+
| OPCODE   | operand0  | operand1  | operand2  |
+----------+-----------+-----------+-----------+
```

The meaning of each operand depends on the opcode (rd/rs1/rs2, immediates, syscall numbers, etc.). Branch immediates are expressed in instruction words (i8) rather than raw byte offsets.

## Operand Conventions

- rd: destination register (writes results). `r0` is the zero register and ignores writes.
- rs1/rs2: source registers (read-only operands).
- Imm variants: some compact forms replace `rs2` with a small immediate.
- Vector forms: may interpret `rd/rs1/rs2` as lane groups or pointers depending on the opcode; see the opcode table notes.


## Memory

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0x30 | `LOAD64` | Load 64-bit word from memory |
| 0x31 | `STORE64` | Store 64-bit word to memory |
| 0x32 | `LOAD128` | Load 128-bit vector (requires `VECTOR` mode; traps otherwise) |
| 0x33 | `STORE128` | Store 128-bit vector (requires `VECTOR` mode; traps otherwise) |

## Arithmetic

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0x01 | `ADD` | Register addition |
| 0x02 | `SUB` | Register subtraction |
| 0x03 | `AND` | Bitwise AND |
| 0x04 | `OR` | Bitwise OR |
| 0x05 | `XOR` | Bitwise XOR |
| 0x06 | `SLL` | Shift left logical |
| 0x07 | `SRL` | Shift right logical |
| 0x08 | `SRA` | Shift right arithmetic |
| 0x09 | `SLT` | Set if less-than (signed) |
| 0x0A | `SLTU` | Set if less-than (unsigned) |
| 0x0B | `CMOV` | Conditional move |
| 0x0C | `NOT` | Bitwise NOT |
| 0x0D | `NEG` | Negate register |
| 0x0E | `SEQ` | Set if equal |
| 0x0F | `SNE` | Set if not equal |
| 0x10 | `MUL` | Multiply (low) |
| 0x11 | `MULH` | Multiply high (signed) |
| 0x12 | `MULHU` | Multiply high (unsigned) |
| 0x13 | `MULHSU` | Multiply high (signed×unsigned) |
| 0x14 | `DIV` | Divide (signed) |
| 0x15 | `DIVU` | Divide (unsigned) |
| 0x16 | `REM` | Remainder (signed) |
| 0x17 | `REMU` | Remainder (unsigned) |
| 0x18 | `ROTL` | Rotate left |
| 0x19 | `ROTR` | Rotate right |
| 0x1A | `POPCNT` | Population count |
| 0x1B | `CLZ` | Count leading zeros |
| 0x1C | `CTZ` | Count trailing zeros |
| 0x20 | `ADDI` | Add immediate |
| 0x21 | `ANDI` | Bitwise AND immediate |
| 0x22 | `ORI` | Bitwise OR immediate |
| 0x23 | `XORI` | Bitwise XOR immediate |
| 0x24 | `CMOVI` | Conditional move immediate |
| 0x25 | `ROTL_IMM` | Rotate left immediate |
| 0x26 | `ROTR_IMM` | Rotate right immediate |

## Control Flow

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0x40 | `BEQ` | Branch if equal |
| 0x41 | `BNE` | Branch if not equal |
| 0x42 | `BLT` | Branch if less than |
| 0x43 | `BGE` | Branch if greater or equal |
| 0x44 | `BLTU` | Branch if less than unsigned |
| 0x45 | `BGEU` | Branch if greater or equal unsigned |
| 0x46 | `JAL` | Jump and link |
| 0x47 | `JR` | Jump to register |
| 0x48 | `JALR` | Jump and link register |
| 0x49 | `HALT` | Stop execution |
| 0x4A | `JMP` | Absolute jump |
| 0x4B | `JALS` | Jump and link (short form) |

## Syscalls and System

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0x60 | `SCALL` | Invoke host syscall |
| 0x61 | `GETGAS` | Read remaining gas |
| 0x62 | `SYSTEM` | System helpers (host-defined) |

## Cryptography and SIMD

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0x70 | `VADD32` | Vector add 32-bit lanes |
| 0x71 | `VADD64` | Vector add 64-bit lanes |
| 0x72 | `VAND` | Vector AND |
| 0x73 | `VXOR` | Vector XOR |
| 0x74 | `VOR` | Vector OR |
| 0x75 | `VROT32` | Vector rotate left 32 |
| 0x76 | `SETVL` | Set logical vector length (I-type imm; 0 → 1; clamped to host max) |
| 0x77 | `PARBEGIN` | Begin parallel region hint *(reserved)* |
| 0x78 | `PAREND` | End parallel region hint *(reserved)* |
| 0x80 | `SHA256BLOCK` | SHA-256 compression round |
| 0x81 | `SHA3BLOCK` | SHA-3 compression round (rs1=&state[25]*8, rs2=&block[136], rd=&out_state[25]*8) |
| 0x82 | `POSEIDON2` | Poseidon permutation (2-ary) |
| 0x83 | `POSEIDON6` | Poseidon permutation (6-ary) |
| 0x84 | `PUBKGEN` | BLS12-381 public key generation |
| 0x85 | `VALCOM` | Validate commitment |
| 0x86 | `ECADD` | Elliptic curve addition |
| 0x87 | `ECMUL_VAR` | Variable-time EC multiply |
| 0x88 | `AESENC` | AES encryption round |
| 0x89 | `AESDEC` | AES decryption round |
| 0x8A | `BLAKE2S` | BLAKE2s compression (rs1=&input[64], writes first 16 bytes to rd/rd+1) |
| 0x8B | `ED25519VERIFY` | Ed25519 signature verification (rs1=&Blob(msg), rs2=&Blob(sig), rd=&Blob(pubkey) → rd=1/0) |
| 0x8C | `ECDSAVERIFY` | ECDSA (secp256k1) signature verification (rs1=&Blob(msg), rs2=&Blob(sig), rd=&Blob(pubkey) → rd=1/0) |
| 0x8D | `DILITHIUMVERIFY` | Dilithium signature verification (rs1=&Blob(msg), rs2=&Blob(sig), rd=&Blob(pubkey) → rd=1/0) |
| 0x8E | `PAIRING` | BLS12-381 pairing check |
| 0x8F | `ED25519BATCHVERIFY` | Deterministic Ed25519 batch verification using a Norito-encoded request (rs1=&NoritoBytes(Ed25519BatchRequest), rd=result 1/0, rs2=failure index; max 512 entries; gas = base + per-entry charge) |

Notes
- Vector ops (`VADD*`/`VAND`/`VXOR`/`VOR`/`VROT32`, `LOAD128`/`STORE128`) are available only when the header `VECTOR` bit is set; otherwise a deterministic mode-disabled trap occurs. `SETVL` sets the logical vector length used for gas scaling and ILP vector helpers; it does not change the physical SIMD width.
- Signature verify opcodes and hash/compression ops consume or produce data via TLV pointers when specified; see `syscalls.md` for pointer-ABI TLV layout and examples.

## ISO 20022 Messaging

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0x90 | `MSG_CREATE` | Create ISO 20022 message object |
| 0x91 | `MSG_CLONE` | Clone current ISO 20022 message |
| 0x92 | `MSG_SET` | Set field on ISO 20022 message |
| 0x93 | `MSG_GET` | Get field from ISO 20022 message |
| 0x94 | `MSG_ADD` | Append repeating ISO 20022 segment |
| 0x95 | `MSG_REMOVE` | Remove ISO 20022 field or segment |
| 0x96 | `MSG_CLEAR` | Clear ISO 20022 message contents |
| 0x97 | `MSG_PARSE` | Parse raw ISO 20022 data |
| 0x98 | `MSG_SERIALIZE` | Serialize ISO 20022 message |
| 0x99 | `MSG_VALIDATE` | Validate ISO 20022 message (required-field, numeric, IBAN with country-length + mod-97 enforcement, and BIC checks; pacs.008 requires debtor/creditor accounts and agent BICs; camt.052/053/054 enforce account identifiers, currencies, and balanced entries; pacs.002/pain.002 demand original group identifiers and status codes; pacs.004 ensures original instruction linkage plus returned amount; pacs.007, pacs.028/029, and camt.056 require original references for recall/status flows) |
| 0x9A | `MSG_SIGN` | Sign ISO 20022 message (Ed25519, secp256k1, or ML-DSA) |
| 0x9B | `MSG_VERIFY_SIG` | Verify ISO 20022 message signature (Ed25519, secp256k1, or ML-DSA) |
| 0x9C | `MSG_SEND` | Validate, serialize, and dispatch ISO 20022 message through channel |
| 0x9D | `ENCODE_STR` | Encode primitive or binary value to string (numeric amount, Base64) |
| 0x9E | `DECODE_STR` | Decode string into typed or binary value (supports Base64) |
| 0x9F | `VALIDATE_FORMAT` | Validate string against format (IBAN with country-length + mod-97 enforcement, BIC, numeric) |

## Zero Knowledge

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0xA0 | `ASSERT` | Assert zero |
| 0xA1 | `ASSERT_EQ` | Assert registers equal |
| 0xA2 | `FADD` | Field addition |
| 0xA3 | `FSUB` | Field subtraction |
| 0xA4 | `FMUL` | Field multiplication |
| 0xA5 | `FINV` | Field inverse |
| 0xA6 | `ASSERT_RANGE` | Assert numeric range |

For detailed semantics see the `instruction` module in the API reference.

Note: Public parallel markers (`PARBEGIN`/`PAREND`) are no‑ops by design. `SETVL` sets only the logical vector length used for gas scaling and helpers; it does not change the physical SIMD width. Any accelerated backend must preserve bit‑exact results relative to the scalar path.

Gas policy notes
- Gas charges are deducted before an instruction executes. If an instruction traps (e.g., due to misalignment or permission), the base gas for that instruction has still been consumed.
- Vector op gas scales with the current logical vector length set by `SETVL` (base cost defined per op; scaling uses a two‑lane baseline). `PARBEGIN`/`PAREND` are public markers and are no‑ops with zero gas.


## Selected Syscall Numbers

The `SCALL` instruction uses an 8‑bit identifier to call host services.  A few
new helpers are available:

| Hex | Constant | Description |
|----:|----------|-------------|
| 0xF4 | `SYSCALL_PROVE_EXECUTION` | Reserved (execution proving integration; currently not implemented by default hosts) |
| 0xF5 | `SYSCALL_GROW_HEAP` | Increase heap size by `x10` bytes |
| 0xF6 | `SYSCALL_VERIFY_PROOF` | Reserved (execution-proof verification integration; currently not implemented by default hosts) |
| 0xF7 | `SYSCALL_GET_MERKLE_PATH` | Write the Merkle path for address `x10` to memory at `x11` |

### NFT syscall naming alignment

The following syscalls use data‑model aligned names:

| Hex | Constant | Alias | Description |
|----:|----------|-------|-------------|
| 0x25 | `SYSCALL_NFT_MINT_ASSET` | - | Create an NFT (non‑fungible asset) |
| 0x26 | `SYSCALL_NFT_TRANSFER_ASSET` | - | Transfer an NFT between accounts |
| 0x27 | `SYSCALL_NFT_SET_METADATA` | - | Set NFT metadata/content (key‑value) |
| 0x28 | `SYSCALL_NFT_BURN_ASSET` | - | Burn an NFT |

Note: These names reflect the canonical `iroha_data_model::nft` terminology.
