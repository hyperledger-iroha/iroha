# IVM ⇄ ISI ⇄ Data Model ⇄ Kotodama — Alignment Review

This document audits how the Iroha Virtual Machine (IVM) instruction set and syscall surface map to the Iroha Special Instructions (ISI) and `iroha_data_model`, and how Kotodama compiles into that stack. It identifies current gaps and proposes concrete improvements so the four layers fit together deterministically and ergonomically.

Note on bytecode target: Kotodama smart contracts compile to Iroha Virtual Machine (IVM) bytecode (`.to`). They do not target “risc5”/RISC‑V as a standalone architecture. Any RISC‑V‑like encodings referenced here are part of IVM’s mixed instruction format and remain an implementation detail.

## Scope and Sources
- IVM: `crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` and `crates/ivm/docs/*`.
- ISI/Data Model: `crates/iroha_data_model/src/isi/*`, `crates/iroha_core/src/smartcontracts/isi/*`, and docs `docs/source/data_model_and_isi_spec.md`.
- Kotodama: `crates/kotodama_lang/src/*`, docs in `crates/ivm/docs/*`.
- Core integration: `crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`.

Terminology
- “ISI” refers to built‑in instruction types that mutate world state via the executor (e.g., RegisterAccount, Mint, Transfer).
- “Syscall” refers to IVM `SCALL` with an 8‑bit number that delegates to the host for ledger operations.

---

## Current Mapping (As Implemented)

### IVM Instructions
- Arithmetic, memory, control flow, crypto, vector, and ZK helpers are defined in `instruction.rs` and implemented in `ivm.rs`. These are self‑contained and deterministic; acceleration paths (SIMD/Metal/CUDA) have CPU fallbacks.
- System/host boundary is via `SCALL` (opcode 0x60). Numbers are listed in `syscalls.rs` and include world operations (register/unregister domain/account/asset, mint/burn/transfer, role/permission ops, triggers) plus helpers (`GET_PRIVATE_INPUT`, `COMMIT_OUTPUT`, `GET_MERKLE_PATH`, etc.).

### Host Layer
- Trait `IVMHost::syscall(number, &mut IVM)` lives in `host.rs`.
- DefaultHost implements only non‑ledger helpers (alloc, heap growth,
  inputs/outputs, ZK proof helpers, feature discovery) — it does NOT perform
  world state mutations.
- The `ivm` crate also has codec/test hosts: `CoreHost` validates pointer-ABI
  TLVs for representative syscalls, while `mock_wsv.rs` provides an in-memory
  WSV harness for VM and Kotodama tests.
- The production runtime adapter is
  `iroha_core::smartcontracts::ivm::host::CoreHostImpl`. It decodes pointer-ABI
  arguments, queues built-in ISI, delegates helper syscalls to the VM host
  layer, and executes nested contract calls through the same permission and
  manifest checks as direct contract transactions.

### ISI and Data Model
- Built‑in ISI types and semantics are implemented in `iroha_core::smartcontracts::isi::*` and documented in `docs/source/data_model_and_isi_spec.md`.
- `InstructionBox` uses a registry with stable “wire IDs” and Norito encoding; native execution dispatch is the current code path in core.

### Core Integration of IVM
- `State::execute_trigger(..)` clones the cached `IVM`, attaches a `CoreHost::with_accounts_and_args`, and then calls `load_program` + `run`.
- `CoreHostImpl` implements `IVMHost`: stateful syscalls are decoded via the
  pointer-ABI TLV layout, mapped to built-in ISI (`InstructionBox`), and
  queued. Once the VM returns, the host hands those ISI to the regular executor
  so permissions, invariants, events, and telemetry remain identical to native
  execution. Helper syscalls that do not touch WSV still delegate to the VM host
  layer.
- Contract dispatch consumes manifest entrypoint metadata for both direct
  `ContractCall` and nested `CALL_CONTRACT`; callers must hold the named
  permission directly or through an assigned role before the callee runs.
- `executor.rs` continues to run built-in ISI natively; the VM host adapter is
  the bridge for IVM contracts, not a replacement for the native executor.

### Kotodama → IVM
- Kotodama has lexer, parser, semantic analysis, IR, register allocation, and
  codegen in `crates/kotodama_lang`.
- Contract forms, public/view/test entrypoints, state handles, structs, tuple
  returns, triggers, permission annotations, typed JSON/Norito helpers, and
  internal helper calls are wired through semantic analysis and codegen.
- Codegen emits pointer-ABI TLVs for ledger syscalls and host helpers:
  - `MintAsset` sets x10=account, x11=asset, x12=&NoritoBytes(Numeric), then
    calls `SYSCALL_MINT_ASSET`.
  - `BurnAsset`, `TransferAsset`, batch transfers, roles, permissions, triggers,
    contract lifecycle helpers, and selected query/sysvar helpers follow the
    same pointer-ABI convention.
- Test helpers such as `invoke_entrypoint_as(...)` use the test-host intrinsic
  surface and support scalar, unit, pointer, and tuple-returning entrypoints.

---

## Alignment Boundaries

1. Core host coverage and parity

`CoreHostImpl` maps the first-release contract syscalls used by Kotodama and the
runtime contract surface into queued ISI, including domains/accounts/assets,
NFTs, roles, permissions, triggers, parameters, smart-contract lifecycle
operations, queries, AXT helpers, ZK verifier calls, and nested contract calls.
Allowed syscall numbers outside that production mapping fail closed with a
metered host rejection until an explicit mapping and test coverage are added.
Soracloud host syscalls are ABI-listed and access-described; dedicated
Soracloud handler execution uses `irohad`'s `SoracloudIvmHost`, while ordinary
`CoreHostImpl` contract execution validates their request TLV, schema,
operation, and payload variant, then fails closed with metered `NotImplemented`.

2. Syscall surface vs. ISI/Data Model naming and coverage

NFT syscalls use canonical `SYSCALL_NFT_*` names aligned with
`iroha_data_model::nft`. Role, permission, trigger, parameter, and smart
contract lifecycle syscalls are tied to concrete core ISI in `CoreHostImpl`.
The syscall tables in `crates/ivm/docs/syscalls.md` remain the source to update
when a new number, pointer type, or gas rule is introduced.

3. ABI for passing typed data across the VM/host boundary

Pointer-ABI TLVs are decoded in the runtime host (`decode_tlv_typed` and
specialized helpers), giving a deterministic path for IDs, metadata, JSON,
Norito payloads, and contract-call arguments. Kotodama emits the matching TLVs
for its builtins and fails closed when static access descriptors cannot be
derived for production manifests.

4. Gas and error mapping consistency

IVM opcodes charge per-op gas. Runtime host syscalls charge deterministic
size-aware costs for queued ISI, query payloads, ZK verification, contract
calls, and helper payloads. Host errors are normalized into VM-visible errors or
explicit status registers before state changes are committed.

5. Determinism across acceleration paths

Hardware-assisted paths keep deterministic CPU fallbacks, and blockchain-visible
results must remain byte-for-byte identical across peers. `SETVL`,
`PARBEGIN`, and `PAREND` are public deterministic hints/no-op markers in the
current VM behavior; acceleration work must preserve the same observable trace
semantics.

6. Kotodama language surface vs. ledger semantics

Kotodama now wires contract bodies, state handles, structs, tuples, triggers,
permissions, typed parameters/returns, dynamic contract calls, and host helper
builtins into the IVM host model. Public entrypoints require `permission(...)`
when they call privileged operations, and `view` functions reject stateful
effects.

---

## Maintenance Steps

### A. Extend the production host by explicit ABI slices

For each new syscall in `ivm::syscalls`, add the ordered ABI-list entry,
document the register and pointer types, implement or intentionally reject the
number in `CoreHostImpl`, and add focused runtime tests that prove the queued
ISI matches native execution semantics.

### B. Keep typed-value ABI rules canonical

Use Norito-framed pointer-ABI TLVs for structured arguments. VM registers carry
pointers to values such as `AccountId`, `AssetDefinitionId`, `Name`, `Json`,
`NftId`, and `NoritoBytes(Numeric)`, while the host decodes them with the same
Norito-backed data-model types used by native ISI.

### C. Keep syscall naming and coverage aligned with ISI/Data Model

Maintain the mapping table in `crates/ivm/docs/syscalls.md` and the host tests
in `crates/iroha_core/src/smartcontracts/ivm/host.rs` whenever a syscall or ISI
shape changes. Privileged ISI must continue to be enforced through the same
permission path as native execution.

### D. Preserve gas and error consistency

Gas costs must be input-size predictable and platform-independent. Host-side ISI
errors should continue to normalize into deterministic VM errors or status
register conventions without committing partial state.

### E. Preserve deterministic acceleration behavior

SIMD, Metal, CUDA, and other accelerated paths must keep deterministic scalar
fallbacks and equivalence tests. Any optimization that changes reduction order,
trace shape, or public output must stay outside consensus-visible behavior.

### F. Keep Kotodama compiler wiring and manifests synchronized

When adding or changing builtins, update semantic effects, access descriptor
derivation, IR lowering, register allocation, codegen, test-host execution, and
manifest documentation together. Production manifests should remain specific:
dynamic or malformed helper payloads fail closed instead of widening access
descriptors.

---

## Representative Mapping Table

This is a readable subset. The canonical list is
`crates/ivm/docs/syscalls.md` plus the ordered ABI list in
`ivm::syscalls::abi_syscall_list()`.

- `SYSCALL_REGISTER_DOMAIN(id: ptr DomainId)` → ISI `Register<Domain>`
- `SYSCALL_REGISTER_ACCOUNT(id: ptr AccountId)` → ISI `Register<Account>`
- `SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, mintable: u8)` → ISI
  `Register<AssetDefinition>`
- `SYSCALL_MINT_ASSET(account: ptr AccountId, asset: ptr AssetDefinitionId,
  amount: ptr NoritoBytes(Numeric))` → ISI `Mint<Numeric, Asset>`
- `SYSCALL_BURN_ASSET(account: ptr AccountId, asset: ptr AssetDefinitionId,
  amount: ptr NoritoBytes(Numeric))` → ISI `Burn<Numeric, Asset>`
- `SYSCALL_TRANSFER_ASSET(from: ptr AccountId, to: ptr AccountId,
  asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric))` → ISI
  `Transfer<Asset>`
- `SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes<TransferAssetBatch>)` → ISI
  `TransferAssetBatch`
- `SYSCALL_NFT_MINT_ASSET(id: ptr NftId, owner: ptr AccountId)` → ISI
  `Register<Nft>`
- `SYSCALL_NFT_TRANSFER_ASSET(from: ptr AccountId, to: ptr AccountId,
  id: ptr NftId)` → ISI `Transfer<Nft>`
- `SYSCALL_NFT_SET_METADATA(id: ptr NftId, key: ptr Name, value: ptr Json)` →
  ISI `SetKeyValue<Nft>`
- `SYSCALL_NFT_BURN_ASSET(id: ptr NftId)` → ISI `Unregister<Nft>`
- `SYSCALL_CREATE_ROLE(name: ptr Name, permissions: ptr Json)` → ISI
  `Register<Role>`
- `SYSCALL_DELETE_ROLE(name: ptr Name)` → ISI `Unregister<Role>`
- `SYSCALL_GRANT_ROLE(account: ptr AccountId, role: ptr Name)` → ISI
  `Grant<Role>`
- `SYSCALL_REVOKE_ROLE(account: ptr AccountId, role: ptr Name)` → ISI
  `Revoke<Role>`
- `SYSCALL_GRANT_PERMISSION(account: ptr AccountId, permission: ptr Name|Json)`
  → ISI `Grant<Permission>`
- `SYSCALL_REVOKE_PERMISSION(account: ptr AccountId, permission: ptr Name|Json)`
  → ISI `Revoke<Permission>`
- `SYSCALL_CREATE_TRIGGER(spec: ptr Json|Trigger)` → ISI `Register<Trigger>`
- `SYSCALL_REMOVE_TRIGGER(name: ptr Name)` → ISI `Unregister<Trigger>`
- `SYSCALL_SET_TRIGGER_ENABLED(name: ptr Name, enabled: int)` → ISI
  `SetKeyValue<Trigger>`
- `SYSCALL_SET_PARAMETER(param: ptr Parameter)` → ISI `SetParameter`
- `SYSCALL_CALL_CONTRACT(request pointers)` → nested contract execution with
  manifest entrypoint permission enforcement

Notes
- “ptr T” means a pointer in a register to Norito-encoded bytes for T, stored
  in VM memory; the host decodes it into the corresponding `iroha_data_model`
  type.
- Return conventions are syscall-specific. Mutating ISI syscalls queue
  instructions or return a VM error; query/helper syscalls return scalars,
  pointer TLVs, status vectors, or nested contract return registers as
  documented in `crates/ivm/docs/syscalls.md`.

---

## Risk Controls

- Keep native ISI execution as the authoritative mutation path: IVM contracts
  queue the same ISI and rely on the regular executor for permission checks,
  invariants, events, and telemetry.
- Require focused parity tests when a host mapping is added or changed.
- Keep ABI hash and syscall-list golden tests in sync with any syscall-surface
  change.
- Preserve pointer-ABI round-trip fixtures in
  `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs`.
