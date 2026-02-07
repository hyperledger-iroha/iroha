---
lang: uz
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3f40329b9968530dea38745b49f7fee4d55aeb461e515e6f97b5b5986cb27e3f
source_last_modified: "2026-01-21T19:17:13.238594+00:00"
translation_last_reviewed: 2026-02-07
---

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
- DefaultHost implements only non‑ledger helpers (alloc, heap growth, inputs/outputs, ZK proof helpers, feature discovery) — it does NOT perform world state mutations.
- A demo `WsvHost` exists in `mock_wsv.rs` that maps a subset of asset operations (Transfer/Mint/Burn) to a tiny in‑memory WSV using `AccountId`/`AssetDefinitionId` via ad‑hoc integer→ID maps in registers x10..x13.

### ISI and Data Model
- Built‑in ISI types and semantics are implemented in `iroha_core::smartcontracts::isi::*` and documented in `docs/source/data_model_and_isi_spec.md`.
- `InstructionBox` uses a registry with stable “wire IDs” and Norito encoding; native execution dispatch is the current code path in core.

### Core Integration of IVM
- `State::execute_trigger(..)` clones the cached `IVM`, attaches a `CoreHost::with_accounts_and_args`, and then calls `load_program` + `run`.
- `CoreHost` implements `IVMHost`: stateful syscalls are decoded via the pointer‑ABI TLV layout, mapped to built-in ISI (`InstructionBox`), and queued. Once the VM returns, the host hands those ISI to the regular executor so permissions, invariants, events, and telemetry remain identical to native execution. Helper syscalls that do not touch WSV still delegate to `DefaultHost`.
- `executor.rs` continues to run built-in ISI natively; migrating the validator executor itself to IVM remains future work.

### Kotodama → IVM
- Frontend pieces exist (lexer/parser/minimal semantics/IR/regalloc).
- Codegen (`kotodama::compiler`) emits a subset of IVM ops and uses `SCALL` for asset operations:
  - `MintAsset` → set x10=account, x11=asset, x12=&NoritoBytes(Numeric); `SCALL SYSCALL_MINT_ASSET`.
  - `BurnAsset`/`TransferAsset` similar (amount passed as NoritoBytes(Numeric) pointer).
- Demos `koto_*_demo.rs` show using `WsvHost` with integer indices mapped to IDs for quick testing.

---

## Gaps and Mismatches

1) Core host coverage and parity
- Status: `CoreHost` now exists in core and translates many ledger syscalls into ISI that execute via the standard path. Coverage is still incomplete (e.g., some roles/permission/trigger syscalls are stubs), and parity tests are required to guarantee the enqueued ISI produce the same state/events as native execution.

2) Syscall surface vs. ISI/Data Model naming and coverage
- NFTs: syscalls now expose canonical `SYSCALL_NFT_*` names aligned with `iroha_data_model::nft`.
- Roles/Permissions/Triggers: syscall list exists, but no reference implementation or mapping table tying each call to a concrete ISI in core.
- Parameters/semantics: some syscalls do not specify parameter encoding (typed IDs vs. pointers) or gas semantics; ISI semantics are well‑defined.

3) ABI for passing typed data across the VM/host boundary
- Pointer‑ABI TLVs are now decoded in `CoreHost` (`decode_tlv_typed`), giving a deterministic path for IDs, metadata, and JSON payloads. Work remains to ensure every syscall documents the expected pointer types and that Kotodama emits the correct TLVs (including error handling when the policy rejects a type).

4) Gas and error mapping consistency
- IVM opcodes charge per‑op gas; CoreHost now returns extra gas for ISI syscalls using the native gas schedule (including batch transfers and the vendor ISI bridge), and ZK verify syscalls reuse the confidential gas schedule. DefaultHost still keeps minimal costs for test coverage.
- Error surfaces differ: IVM returns `VMError::{OutOfGas,PermissionDenied,...}`; ISI returns `InstructionExecutionError` categories (`Find`, `Repetition`, `InvariantViolation`, `Math`, `Type`, `Mintability`, `InvalidParameter`).

5) Determinism across acceleration paths
- IVM vector/CUDA/Metal have CPU fallbacks, but some ops remain reserved (`SETVL`, PARBEGIN/PAREND) and are not part of the deterministic core yet.
- Merkle trees differ between IVM and node (`ivm::merkle_tree` vs `iroha_crypto::MerkleTree`) — a unification item already appears in `roadmap.md`.

6) Kotodama language surface vs. intended ledger semantics
- Compiler emits a small subset; most language features (state/structs, triggers, permissions, typed params/returns) are not wired to the host/ISI model yet.
- No capability/effect typing to ensure that syscalls are legal for the authority.

---

## Recommendations (Concrete Steps)

### A. Implement a production IVM host in core
- Add `iroha_core::smartcontracts::ivm::host` module implementing `ivm::host::IVMHost`.
- For each syscall in `ivm::syscalls`:
  - Decode arguments via a canonical ABI (see B.), construct the corresponding built‑in ISI or call the same core logic directly, execute it against `StateTransaction`, and map errors deterministically back to an IVM return code.
  - Charge gas deterministically using a per‑syscall table defined in core (and exposed to IVM via `SYSCALL_GET_PARAMETER` if needed in the future). Initially, return fixed extra gas from the host for each call.
- Thread `authority: &AccountId` and `&mut StateTransaction` into the host so permission checks and events are identical to native ISI.
- Update `State::execute_trigger(ExecutableRef::Ivm)` to attach this host before `vm.run()` and return the same `ExecutionStep` semantics as ISI (events are already emitted in core; consistent behavior should be validated).

### B. Define a deterministic VM/host ABI for typed values
- Use Norito on the VM side for structured arguments:
  - Pass pointers (in x10..x13, etc.) to VM memory regions containing Norito‑encoded values for types like `AccountId`, `AssetDefinitionId`, `Numeric`, `Metadata`.
  - Host reads bytes via `IVM` memory helpers and decodes with Norito (`iroha_data_model` already derives `Encode/Decode`).
- Add minimal helpers in Kotodama codegen to serialize literal IDs into code/constant pools or to prepare call frames in memory.
- Amounts are `Numeric` and are passed as NoritoBytes pointers; other complex types also pass by pointer.
- Document this in `crates/ivm/docs/calling_convention.md` and add examples.

### C. Align syscall naming and coverage with ISI/Data Model
- Rename NFT‑related syscalls for clarity: canonical names now follow the `SYSCALL_NFT_*` pattern (`SYSCALL_NFT_MINT_ASSET`, `SYSCALL_NFT_SET_METADATA`, etc.).
- Publish a mapping table (doc + code comments) from each syscall to core ISI semantics, including:
  - Parameters (registers vs. pointers), expected preconditions, events, and error mappings.
  - Gas charges.
- Ensure there is a syscall for each built‑in ISI that should be invocable from Kotodama (domains, accounts, assets, roles/permissions, triggers, parameters). If an ISI must remain privileged, document it and enforce via permission checks in the host.

### D. Unify errors and gas
- Add a translation layer in the host: map `InstructionExecutionError::{Find,Repetition,InvariantViolation,Math,Type,Mintability,InvalidParameter}` to specific `VMError` codes or an extended result convention (e.g., set `x10=0/1` and use a well‑defined `VMError::HostRejected { code }`).
- Introduce a gas table in core for syscalls; mirror it in IVM docs; ensure costs are input‑size predictable and platform‑independent.

### E. Determinism and shared primitives
- Complete Merkle tree unification (see roadmap) and remove/alias `ivm::merkle_tree` to `iroha_crypto` with identical leaves and proofs.
- Keep `SETVL`/PARBEGIN/PAREND` reserved until end‑to‑end determinism checks and a deterministic scheduler strategy are in place; document that IVM ignores these hints today.
- Ensure acceleration paths produce byte‑for‑byte identical outputs; where not feasible, guard behind features with a test ensuring CPU fallback equivalence.

### F. Kotodama compiler wiring
- Extend codegen to the canonical ABI (B.) for IDs and complex parameters; stop using integer→ID demo maps.
- Add builtins mapping directly to ISI syscalls beyond assets (domains/accounts/roles/permissions/triggers) with clear names.
- Add compile‑time capability checks and optional `permission(...)` annotations; fallback to runtime host errors when static proof is not possible.
- Add unit tests in `crates/ivm/tests/kotodama.rs` that compile and run small contracts end‑to‑end using a test host that decodes Norito arguments and mutates a temporary WSV.

### G. Documentation and developer ergonomics
- Update `docs/source/data_model_and_isi_spec.md` with the syscall mapping table and ABI notes.
- Add a new doc “IVM Host Integration Guide” in `crates/ivm/docs/` describing how to implement an `IVMHost` over real `StateTransaction`.
- Clarify in `README.md` and crate docs that Kotodama targets IVM `.to` bytecode and that syscalls are the bridge into world state.

---

## Suggested Mapping Table (Initial Draft)

Representative subset — finalize and expand during host implementation.

- SYSCALL_REGISTER_DOMAIN(id: ptr DomainId) → ISI Register<Domain>
- SYSCALL_REGISTER_ACCOUNT(id: ptr AccountId) → ISI Register<Account>
- SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, mintable: u8) → ISI Register<AssetDefinition>
- SYSCALL_MINT_ASSET(account: ptr AccountId, asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric)) → ISI Mint<Numeric, Asset>
- SYSCALL_BURN_ASSET(account: ptr AccountId, asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric)) → ISI Burn<Numeric, Asset>
- SYSCALL_TRANSFER_ASSET(from: ptr AccountId, to: ptr AccountId, asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric)) → ISI Transfer<Asset>
- SYSCALL_TRANSFER_V1_BATCH_BEGIN() / SYSCALL_TRANSFER_V1_BATCH_END() → ISI TransferAssetBatch (open/close the scope; individual entries are lowered via `transfer_asset`)
- SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes<TransferAssetBatch>) → Submit a pre-encoded batch when contracts already serialized the entries off-chain
- SYSCALL_NFT_MINT_ASSET(id: ptr NftId, owner: ptr AccountId) → ISI Register<Nft>
- SYSCALL_NFT_TRANSFER_ASSET(from: ptr AccountId, to: ptr AccountId, id: ptr NftId) → ISI Transfer<Nft>
- SYSCALL_NFT_SET_METADATA(id: ptr NftId, content: ptr Metadata) → ISI SetKeyValue<Nft>
- SYSCALL_NFT_BURN_ASSET(id: ptr NftId) → ISI Unregister<Nft>
- SYSCALL_CREATE_ROLE(id: ptr RoleId, role: ptr Role) → ISI Register<Role>
- SYSCALL_GRANT_ROLE(account: ptr AccountId, role: ptr RoleId) → ISI Grant<Role>
- SYSCALL_REVOKE_ROLE(account: ptr AccountId, role: ptr RoleId) → ISI Revoke<Role>
- SYSCALL_SET_PARAMETER(param: ptr Parameter) → ISI SetParameter

Notes
- “ptr T” means a pointer in a register to Norito‑encoded bytes for T, stored in VM memory; the host decodes it into the corresponding `iroha_data_model` type.
- Return convention: success sets `x10=1`; failure sets `x10=0` and may raise `VMError::HostRejected` for fatal errors.

---

## Risks and Rollout Plan
- Start by wiring host for a narrow set (Assets + Accounts) and add focused tests.
- Keep native ISI execution as the authoritative path while host semantics mature; run both paths under a “shadow mode” in tests to assert identical end effects and events.
- Once parity is validated, enable IVM host for IVM triggers in production; later consider routing regular transactions through IVM as well.

---

## Outstanding Work
- Finalise the Kotodama helpers that pass Norito‑encoded pointers (`crates/ivm/src/kotodama_std.rs`) and surface them through the compiler CLI.
- Publish the syscall gas table (including helper syscalls) and keep CoreHost enforcement/tests aligned with it.
- ✅ Added round-trip Norito fixtures covering the pointer-argument ABI; see `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs` for manifest and NFT pointer coverage kept in CI.
