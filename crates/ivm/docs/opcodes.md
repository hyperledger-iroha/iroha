# Instruction Set Overview

This document lists the opcodes understood by the Iroha Virtual Machine. Each
opcode is represented as a constant grouped by category in
[`instruction.rs`](../src/instruction.rs) and documented in the generated API
reference. The instructions are grouped by category for convenience.

IVM's opcodes allow building arbitrary loops and data transformations, ensuring the instruction set is Turing complete. Every program is limited by its gas budget, providing deterministic halting. Register and memory operations use 64-bit words optimized for fast financial calculations.

ABI v1 admits 58 opcodes: scalar arithmetic, 64-bit memory access, typed literals, control flow and host calls. Hashing and signature verification are syscalls, and the instruction set contains no vector, cryptographic or zero-knowledge opcodes. Every opcode byte not listed below is unassigned and is rejected with `InvalidOpcode` carrying that byte, both at artifact admission and at runtime.

Conventions and notes
- Hex values in the tables refer to the primary opcode byte in the wide encoding (8-bit).
- Mode bits: `ZK` is the only program-header mode bit. It enables privacy tags, private-memory tracking and trace padding; it gates no opcode. A header with any other mode bit is rejected.
- Pointer-ABI: no opcode dereferences a Norito TLV pointer. `LDLIT` yields a prevalidated literal pointer; syscalls consume pointers under the pointer-ABI documented in `syscalls.md`/`tlv_examples.md`.
- Memory: all loads/stores require natural alignment; region permissions (INPUT/OUTPUT/CODE/HEAP/STACK) are enforced uniformly. Misaligned accesses deterministically trap with `MisalignedAccess`.
- Control flow: `JALR` adds its signed 14-bit immediate to the register base with 64-bit two's-complement wrapping, then masks the low two bits of the target so control transfers are 4-byte aligned. `JAL` carries `rd` plus a signed 19-bit word offset. `JMP` and `JALS` use the whole low 24 bits as a signed word offset; `JALS` writes its return address to `r1`. Deployable artifacts carrying a `CNTR` interface additionally enforce protected returns: direct calls linking `r1` push the expected return PC onto a host-protected stack, and only canonical `JALR r0, r1, 0` returns may pop it. A mismatch, noncanonical indirect transfer or `HALT` traps `ControlFlowViolation` and a depth above 1,024 traps `CallDepthExceeded`, before the target executes. At invocation start the VM captures the aligned initial `r1`; an empty protected stack must return to that exact captured address, which must also be a `HALT` instruction or the host's end-of-code sentinel. Raw code loaded with `IVM::load_code` retains the general opcode semantics for low-level tests and tooling, and each load installs the default non-ZK execution profile independently of the previously loaded image.
- Indexed literals: `LDLIT` and `LDI64` carry `rd` plus an unsigned 16-bit `LTLB` index. The authenticated table descriptor names either an exact ABI-v1 pointer TLV or an exact eight-byte signed scalar. Program loading rejects unknown kinds, noncanonical lengths, gaps, aliases, out-of-range indices, and instruction/entry kind mismatches before execution. Scalar entries never grant pointer provenance.
- First-release availability: `LDLIT`, `LDI64`, `JAL`, `JMP`, and `JALS` are unconditional ABI-v1 instructions. They do not consult a feature bit or execution-mode flag and are present in every IVM build.
- Proof-relation coverage: the V1 inventory in [`proof_coverage/opcodes.rs`](../src/proof_coverage/opcodes.rs) names all 58 admitted opcodes with their step effect, PC-transition class, whole-invocation relation obligations and interpreter fault surface. A compile-time all-256-opcode check requires admission changes to update that inventory; see [`proof_coverage.md`](proof_coverage.md). It is a coverage guard only; it supplies no execution constraints or STARK proof and cannot authorize `IvmProved` admission.
- Kotodama's relaxing assembler uses one-word `JAL` transfers when the signed
  19-bit word range is sufficient and one-word `JMP`/`JALS` transfers
  otherwise. Transfers beyond the signed 24-bit range route through sparse
  two-word trampoline islands. Each island has a direct skip on the normal
  fallthrough path and a direct `JMP` on the far path, preserving statically
  verifiable control flow and linking protected calls exactly once.
- See also: `instruction.rs` API docs for authoritative field extractors and masks used by encoders/decoders.
## Encoding Format

Every instruction is one 32-bit little-endian word. Bits 31..24 hold the primary opcode; the
remaining 24 bits are laid out by the opcode's operand format. There are 32 general-purpose
registers, so every register field is five bits wide. Helpers in `encoding::wide` and
`instruction::wide` are the canonical encoders/decoders (`instruction::wide::OperandFormat`,
`operand_format`, `validate_word`), and all code emission in the Kotodama compiler and reference
tooling uses them exclusively. The encoders reject out-of-range registers and immediates instead of
truncating them.

| Format | Opcodes | Bits 23..19 | Bits 18..14 | Bits 13..0 |
|---|---|---|---|---|
| Register3 | ALU, compare, MUL*, DIV*, REM*, ROTL/ROTR, MIN/MAX, DIV_CEIL, GCD, MEAN, CMOV | `rd` | `rs1` | `rs2` in 13..9; 8..0 zero |
| Register2 | NOT, NEG, POPCNT, CLZ, CTZ, ISQRT, ABS | `rd` | `rs1` | zero |
| Immediate | ADDI, ANDI, ORI, XORI, CMOVI, LOAD64, JALR | `rd` | `rs1`/base | signed `imm14` |
| RotateImmediate | ROTL_IMM, ROTR_IMM | `rd` | `rs1` | amount in 5..0; 13..6 zero |
| Store | STORE64 | value | base | signed byte displacement |
| Branch | BEQ, BNE, BLT, BGE, BLTU, BGEU | left | right | signed word offset |
| Jal | JAL | `rd` | signed 19-bit word offset in 18..0 | |
| Offset24 | JMP, JALS | signed 24-bit word offset in 23..0 | | |
| Syscall | SCALL | 24-bit syscall number in 23..0 | | |
| Literal | LDLIT, LDI64 | `rd` | 18..16 zero; `u16` index in 15..0 | |
| NoOperands | HALT | zero | zero | zero |

Every bit a format does not assign is reserved and must be zero. Admission rejects a word with a
reserved bit set (`DecodeError`), so each admitted instruction has exactly one encoding. Branch and
direct-transfer offsets are expressed in instruction words relative to the instruction itself. The
format table, register count, ZK tag rule and instruction fault mapping are bound into the ABI
descriptor hash.

## Operand Conventions

- rd: destination register (writes results). `r0` is the zero register and ignores writes.
- rs1/rs2: source registers (read-only operands).
- Immediate variants replace `rs2` with a signed 14-bit immediate.
- ZK mode joins privacy tags: a two-source result is private when either source is private.
  Branch operands, DIV/DIVU/REM/REMU/DIV_CEIL/ABS operands, memory bases, JALR bases and CMOV/CMOVI
  conditions must be public.
- Faults: a zero divisor traps `Numeric(DivisionByZero)`; `i64::MIN / -1`, `REM(i64::MIN, -1)`,
  `ABS(i64::MIN)` and a ceiling quotient above `i64::MAX` trap `Numeric(RegisterIntegerOverflow)`.
  ADD/SUB/MUL/NEG/shifts/rotates wrap without faulting.


## Memory

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0x30 | `LOAD64` | Load 64-bit word from memory |
| 0x31 | `STORE64` | Store 64-bit word to memory |
| 0x34 | `LDLIT` | Load a prevalidated typed-TLV pointer from `LTLB[index:u16]` into `rd` |
| 0x35 | `LDI64` | Load a prevalidated exact little-endian signed `i64` from `LTLB[index:u16]` into `rd` |

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
| 0x14 | `DIV` | Divide (signed); traps on zero divisor or `i64::MIN / -1` overflow |
| 0x15 | `DIVU` | Divide (unsigned) |
| 0x16 | `REM` | Remainder (signed); traps on zero divisor or `i64::MIN % -1` overflow |
| 0x17 | `REMU` | Remainder (unsigned) |
| 0x18 | `ROTL` | Rotate left |
| 0x19 | `ROTR` | Rotate right |
| 0x1A | `POPCNT` | Population count |
| 0x1B | `CLZ` | Count leading zeros |
| 0x1C | `CTZ` | Count trailing zeros |
| 0x1D | `ISQRT` | Integer square root |
| 0x1E | `MIN` | Minimum (signed) |
| 0x1F | `MAX` | Maximum (signed) |
| 0x20 | `ADDI` | Add immediate |
| 0x21 | `ANDI` | Bitwise AND immediate |
| 0x22 | `ORI` | Bitwise OR immediate |
| 0x23 | `XORI` | Bitwise XOR immediate |
| 0x24 | `CMOVI` | Conditional move immediate |
| 0x25 | `ROTL_IMM` | Rotate left immediate |
| 0x26 | `ROTR_IMM` | Rotate right immediate |
| 0x27 | `ABS` | Absolute value (signed); traps on `i64::MIN` overflow |
| 0x28 | `DIV_CEIL` | Ceiling division (signed); traps on zero divisor or `i64::MIN / -1` overflow |
| 0x29 | `GCD` | Greatest common divisor (unsigned magnitude) |
| 0x2A | `MEAN` | Overflow-free arithmetic mean (signed) |

## Control Flow

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0x40 | `BEQ` | Branch if equal |
| 0x41 | `BNE` | Branch if not equal |
| 0x42 | `BLT` | Branch if less than |
| 0x43 | `BGE` | Branch if greater or equal |
| 0x44 | `BLTU` | Branch if less than unsigned |
| 0x45 | `BGEU` | Branch if greater or equal unsigned |
| 0x46 | `JAL` | Jump by signed 19-bit word offset and write the return address to `rd` |
| 0x48 | `JALR` | Jump and link register; deployable contracts permit only the canonical protected return `JALR r0, r1, 0` |
| 0x49 | `HALT` | Stop execution |
| 0x4A | `JMP` | Jump by signed 24-bit word offset |
| 0x4B | `JALS` | Jump by signed 24-bit word offset and write the return address to `r1` |

## Syscalls and System

| Hex | Mnemonic | Description |
|----:|----------|-------------|
| 0x60 | `SCALL` | Invoke the host syscall whose 24-bit number occupies bits 23..0 |

Gas policy notes
- Gas charges are deducted before an instruction executes. If an instruction traps (e.g., due to misalignment or permission), the base gas for that instruction has still been consumed.
- `HALT` is the only zero-gas opcode; it ends the run, so no repeatable instruction is free.
- Gas is the only execution budget. The caller supplies it and nested invocations draw from the parent's remaining gas; a program header never converts into gas. The header `max_cycles` field is only the ZK trace bound.


## Selected Syscall Numbers

The `SCALL` instruction carries a 24-bit syscall number. A few helpers are
available:

| Hex | Constant | Description |
|----:|----------|-------------|
| 0xF4 | `SYSCALL_EXECUTION_SUMMARY` | Return the deterministic, self-reported execution summary emitted by `DefaultHost` |
| 0xF5 | `SYSCALL_GROW_HEAP` | Increase heap size by `x10` bytes, without exceeding the host-installed runtime ceiling |
| 0xF6 | `SYSCALL_VERIFY_PROOF` | Verify registry-bound proof envelopes in `CoreHost`; `DefaultHost` returns `NotImplemented` |
| 0xF7 | `SYSCALL_GET_MERKLE_PATH` | Write the Merkle path for address `x10` to memory at `x11` |
| 0x010000 | `SYSCALL_QUERY_EXECUTE_NORITO` | Execute a Norito-encoded read-only query request |
| 0x010001 | `SYSCALL_CORE_QUERY_GET` | Return `Option<View>` for one stable-tagged core entity |
| 0x010002 | `SYSCALL_CORE_QUERY_PAGE` | Return `List<View,64>` plus `Option<int>` handles in canonical ID order |
| 0x010020..0x010025 | `SYSCALL_SYSVAR_*` | Read deterministic chain/block/authority/contract context |

### NFT syscall naming alignment

The following syscalls use data‑model aligned names:

| Hex | Constant | Alias | Description |
|----:|----------|-------|-------------|
| 0x25 | `SYSCALL_NFT_MINT_ASSET` | - | Create an NFT (non‑fungible asset) |
| 0x26 | `SYSCALL_NFT_TRANSFER_ASSET` | - | Transfer an NFT between accounts |
| 0x27 | `SYSCALL_NFT_SET_METADATA` | - | Set NFT metadata/content (key‑value) |
| 0x28 | `SYSCALL_NFT_BURN_ASSET` | - | Burn an NFT |

Note: These names reflect the canonical `iroha_data_model::nft` terminology.
