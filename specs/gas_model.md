# IVM Gas Model

This document defines the canonical gas schedule for the Iroha Virtual Machine
(IVM) and explains how the schedule is hashed and applied. The source of truth
for costs is `crates/ivm/src/gas.rs`; the schedule table below is a rendered
view of that canonical mapping.

## Scope

- Applies to raw IVM bytecode, proved IVM execution, deployed `ContractCall`,
  and contract-call items inside `Executable::Batch`.
- Native ISI gas metering is defined separately in `crates/iroha_core/src/gas.rs`.
- A transaction batch containing any contract call has one signature-bound gas
  limit. Native item costs and every contract invocation consume that common
  budget in canonical input order; the transaction settles gas once rather
  than once per item. State changes remain atomic if an item exhausts the
  shared limit.
- Proposal-queue accounting charges every runtime-dependent executable (raw or
  proved IVM, deployed contract calls, and mixed batches) its complete
  signature-bound gas limit. Missing bounds fail queue admission instead of
  becoming zero-cost work. Native-only executables retain deterministic ISI
  metering. Lane selection applies these costs to the on-chain
  `ivm_gas_limit_per_block` budget; no environment-derived gas fallback
  exists.
- A trigger batch likewise uses one shared deterministic budget: the remaining
  block budget when it is configured, or the existing default trigger cap when
  block gas is unlimited. The cap applies to the batch as a whole, not once per
  contract-call item, and an item failure rolls back the trigger batch.
- ISO 20022 opcodes are reserved in ABI v1 and do not carry gas entries yet.

## Heap governance

Gas and heap memory are independent deterministic limits. The on-chain
`smart_contract.memory` parameter is the exact heap ceiling in bytes for raw,
generic, deployed, nested, trigger, view, simulated, and mandatory
proved-execution derivation/replay. `executor.memory` applies the same rule to
user-provided runtime-executor validation and migration. Both default to the
ABI V1 maximum of 1 MiB and `SetParameter` rejects larger values because the
next address begins the INPUT region. The ceiling also bounds `GROW_HEAP`; a
guest cannot expand past governance. Warm-runtime keys contain the heap
ceiling, so changing either parameter deterministically selects a matching
runtime baseline.

## Retained-effect governance

Gas limits bound computation, while the on-chain
`smart_contract.max_output_items` and `smart_contract.max_output_bytes`
parameters bound the effect material a host may retain during one execution.
The aggregate includes queued instructions, pending FastPQ entries, durable
state writes, completed AXT state, and access-log artifacts. The first-release
defaults are 4,096 items and 8 MiB.

All consensus, contract, trigger, access-prepass, and proved-execution hosts
load these values from the same world-state snapshot. A reservation that would
cross either boundary returns `HostOutputBudgetExceeded` from the active
syscall, aborting the VM and transaction before any retained artifacts are
applied. Tooling may apply a stricter transport limit component-wise, but such
a limit cannot expand or replace consensus validity.

## Determinism and schedule hash

The gas schedule is deterministic. Descriptor format 3, domain-separated as
`iroha.ivm.gas-schedule.v3`, commits to:

- the ordered opcode byte (`u8`) and cost (`u64`, little-endian) table;
- every ordered, named host/numeric formula parameter;
- every staged-metering phase name and `u8` tag directly; and
- the exhaustive ordered ABI-v1 syscall metering records (mode, gas class,
  quote strategy, formula, parameter family, and minimum gas).

The staged phase table is `0 Entry`, `1 PointerHeader`, `2 PointerEnvelope`,
`3 PayloadHash`, `4 NoritoDecode`, `5 CanonicalValidation`, `6 Arithmetic`,
`7 Normalization`, and `8 OutputSerialization`. Changing a phase name, tag, or
order therefore changes the schedule hash even if all numeric constants remain
unchanged.

The hash is exposed by:

- `ivm::gas::schedule_hash()` (canonical schedule hash)
- `ivm::limits::schedule_hash()` (host-facing alias)

Every node advertises this digest in its signed consensus handshake. Peer
admission requires an exact match, so validators with different executable gas
schedules reject one another before exchanging consensus traffic. Telemetry
exposes the same digest for operator diagnosis.

## Ledger-query host gas

ABI-v1 ledger reads that execute through the query engine use the
`LedgerQueryV1` formula family. This includes generic query execution, typed
core-query gets and pages, parameter and contract lookups, balance reads, and
account-alias resolution. The exact post-execution charge is computed with
saturating `u64` arithmetic:

```text
gas = base
    + item_rate * processed_items
    + item_rate * offset_items
    + 2 * processed_bytes
```

The schedule descriptor commits the formula version and every cost constant:

| Descriptor parameter | Value | Meaning |
|---|---:|---|
| `ledger_query_formula_version` | 1 | Version of the complete request-kind, offset, sorting, item, and byte formula |
| `ledger_query_base_singular` | 1,000 | Base for a singular query |
| `ledger_query_base_iterable` | 2,500 | Base for an iterable query |
| `ledger_query_per_item` | 250 | Item rate for an unsorted query |
| `ledger_query_sort_multiplier` | 4 | Multiplier for a query that requests metadata sorting; its effective item rate is 1,000 |
| `ledger_query_per_byte` | 2 | Rate for each canonically measured byte |

Request semantics prevent pagination work from being charged twice:

- An unsorted `QueryRequest::Start` uses the 250-unit item rate and sets
  `offset_items` to the requested pagination offset. Bytes from skipped values
  are still included in `processed_bytes`.
- A metadata-sorted start query uses the 1,000-unit effective item rate and
  sets `offset_items` to zero. Its `processed_items` already counts the
  candidates scanned to form the sorted result, including work attributable to
  the offset.
- Singular queries use the 1,000-unit base with no offset. Iterable queries,
  including typed core-query pages, use the 2,500-unit base.
- For query-engine execution, `processed_bytes` includes canonical source
  traversal and the final framed query response. Direct singular lookup paths
  charge their canonical result payload instead. A typed core-query result
  additionally charges its projected page payload and the pointer-ABI leaf
  TLVs prepared for materialization, exactly once each.

The query syscalls reserve the caller's available bounded gas before
host-dependent work, derive one shared item-and-byte execution budget from
these constants, and reject work that cannot fit that budget before returning
guest-visible output. Changing any value or semantic rule above requires a
formula-version increment and produces a different gas-schedule hash.

## Ed25519 batch-verification gas

`ED25519BATCHVERIFY` accepts a Norito-encoded
`Ed25519BatchRequest { entries }` containing 1–512 entries. The encoded payload
is capped at 512 KiB and decoded with explicit per-sequence, per-field,
cumulative-element, cumulative-allocation, and nesting-depth limits. The
complete formula is:

```text
gas = 500
    + encoded_payload_bytes
    + 1,000 * admitted_entries
```

The 500-unit opcode base is charged first. For an in-range owned public
envelope, the byte charge is debited before checksum hashing or Norito
decoding. After a valid request passes the entry-count bound, the complete
entry charge is debited before verification. The opcode then performs one
strict Ed25519 verification per entry in canonical input order and reports the
first failure index; GPU availability cannot change acceptance or duplicate
work through a fallback pass.

The schedule descriptor commits the formula version, both rates, both request
bounds, and every Norito decode limit. A change to any of those values or to
the charge-point ordering therefore changes `ivm::gas::schedule_hash()` and is
rejected by the signed peer handshake when validators disagree.

## Vector scaling

- Vector ops (`VADD*`, `VAND`, `VXOR`, `VOR`, `VROT32`) scale with the logical
  vector length set by `SETVL`. The base costs in the table are scaled by
  `min(vector_len, VECTOR_BASE_LANES) / VECTOR_BASE_LANES` (baseline = 2 lanes).

## Canonical opcode gas table

The table below lists the base costs used by `ivm::gas::cost_of`. Vector scaling
is applied on top of these base values as noted above.

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
| memory | 0x34 | `LDLIT` | 1 |
| memory | 0x35 | `LDI64` | 1 |
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
| system | 0x62 | `SYSTEM` (`SCALLX`) | 5 |
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
