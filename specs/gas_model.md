# IVM Gas Model

This document defines the canonical gas schedule for the Iroha Virtual Machine
(IVM) and explains how the schedule is hashed and applied. The source of truth
for costs is `crates/ivm/src/gas.rs`; the schedule table below is a rendered
view of that canonical mapping.

## Scope

- Applies to raw IVM bytecode, proved IVM execution, deployed `ContractCall`,
  and contract-call items inside `Executable::Batch`.
- Native ISI gas metering is defined separately in `crates/iroha_core/src/gas.rs`.
- Native prepared overlays add accountable executed gas to the cumulative
  parent-block budget exactly once whether their state commits or rolls back;
  sequential and parallel apply enforce the same limit. The deterministic
  authored/base charge is reserved against the parent budget before any
  prepared effect is applied. Nested trigger or host work is appended to that
  retained total, never overwritten during fee settlement, and a configured
  block cap cannot be exceeded by post-cap execution.
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
- Live mixed-batch overlay-limit rejection is fee-exempt because its effects do
  not commit, but all work attempted before the bound is detected remains
  accountable to the parent block exactly once. Other rejected live execution
  retains its ordinary fee lifecycle and the same exact gas total.
- Opcode bytes outside the canonical opcode gas table are unassigned, rejected
  as `InvalidOpcode`, and carry no gas entry.

## Invocation gas limit

Gas is the only execution budget. The caller owns it: a root invocation runs
under the signature-bound transaction gas limit within the remaining block
budget (and, for a quarantine-lane transaction, within the committed
`quarantine_tx_max_gas` cap), and a nested invocation draws from its parent's
remaining gas, which is billed back to the parent. The budget is shared by
instructions, ZK trace padding (one gas per padded cycle) and syscalls. No
artifact header field converts into gas; only `HALT` is free, so gas alone
bounds the number of executed instructions.

The header `max_cycles` field is the ZK trace bound. It is nonzero exactly when
the header selects ZK mode; node policy `ivm_max_cycles_upper_bound` caps it.

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

The gas schedule is deterministic. Descriptor format 4, domain-separated as
`iroha.ivm.gas-schedule.v4`, commits to:

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

## Canonical opcode gas table

The table below lists the price of each of the 59 admitted opcodes, as returned
by `ivm::gas::cost_of`. Every opcode has one fixed price: no operand, mode or
hardware property scales it. `HALT` is the only free opcode, and it ends the
run. Hashing and signature verification are syscalls, metered by the syscall
records committed in the schedule descriptor.

| Category | Opcode | Mnemonic | Gas |
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
| control | 0x4A | `JMP` | 2 |
| control | 0x4B | `JALS` | 2 |
| control | 0x49 | `HALT` | 0 |
| system | 0x60 | `SCALL` | 5 |
| system | 0x62 | `SYSTEM` (`SCALLX`) | 5 |
