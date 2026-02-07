---
lang: kk
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
---

# Iroha v2 Data Model and ISI — Implementation‑Derived Spec

This specification is reverse‑engineered from the current implementation across `iroha_data_model` and `iroha_core` to aid design review. Paths in backticks point to the authoritative code.

## Scope
- Defines canonical entities (domains, accounts, assets, NFTs, roles, permissions, peers, triggers) and their identifiers.
- Describes state‑changing instructions (ISI): types, parameters, preconditions, state transitions, emitted events, and error conditions.
- Summarizes parameter management, transactions, and instruction serialization.

Determinism: All instruction semantics are pure state transitions without hardware‑dependent behavior. Serialization uses Norito; VM bytecode uses the IVM and is validated host‑side before on‑chain execution.

---

## Entities and Identifiers
IDs have stable string forms with `Display`/`FromStr` round‑trip. Name rules forbid whitespace and the reserved `@ # $` characters.

- `Name` — validated textual identifier. Rules: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Domain: `{ id, logo, metadata, owned_by }`. Builders: `NewDomain`. Code: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — canonical addresses are produced via `AccountAddress` (IH58 / `sora…` compressed / hex) and Torii normalises inputs through `AccountAddress::parse_any`. IH58 is the preferred account format; the `sora…` form is second-best for Sora-only UX. The familiar `alias@domain` string is retained as a routing alias only. Account: `{ id, metadata }`. Code: `crates/iroha_data_model/src/account.rs`.
- Account admission policy — domains control implicit account creation by storing a Norito-JSON `AccountAdmissionPolicy` under metadata key `iroha:account_admission_policy`. When the key is absent, the chain-level custom parameter `iroha:default_account_admission_policy` provides the default; when that is also absent, the hard default is `ImplicitReceive` (first release). The policy tags `mode` (`ExplicitOnly` or `ImplicitReceive`) plus optional per-transaction (default `16`) and per-block creation caps, an optional `implicit_creation_fee` (burn or sink account), `min_initial_amounts` per asset definition, and an optional `default_role_on_create` (granted after `AccountCreated`, rejects with `DefaultRoleError` if missing). Genesis cannot opt in; disabled/invalid policies reject receipt-style instructions for unknown accounts with `InstructionExecutionError::AccountAdmission`. Implicit accounts stamp metadata `iroha:created_via="implicit"` before `AccountCreated`; default roles emit a follow-up `AccountRoleGranted`, and executor owner-baseline rules let the new account spend its own assets/NFTs without extra roles. Code: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. Definition: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Code: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId` — `asset#domain#account` or `asset##account` if domains match, where `account` is the canonical `AccountId` string (IH58 preferred). Asset: `{ id, value: Numeric }`. Code: `crates/iroha_data_model/src/asset/{id.rs,value.rs}`.
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Code: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Role: `{ id, permissions: BTreeSet<Permission> }` with builder `NewRole { inner: Role, grant_to }`. Code: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Code: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — peer identity (public key) and address. Code: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Trigger: `{ id, action }`. Action: `{ executable, repeats, authority, filter, metadata }`. Code: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` with checked insert/remove. Code: `crates/iroha_data_model/src/metadata.rs`.
- Subscription pattern (application layer): plans are `AssetDefinition` entries with `subscription_plan` metadata; subscriptions are `Nft` records with `subscription` metadata; billing is executed by time triggers referencing subscription NFTs. See `docs/source/subscriptions_api.md` and `crates/iroha_data_model/src/subscription.rs`.
- **Cryptographic primitives** (feature `sm`):
  - `Sm2PublicKey` / `Sm2Signature` mirror the canonical SEC1 point + fixed-width `r∥s` encoding for SM2. Constructors enforce curve membership and distinguishing ID semantics (`DEFAULT_DISTID`), while verification rejects malformed or high-range scalars. Code: `crates/iroha_crypto/src/sm.rs` and `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` exposes the GM/T 0004 digest as a Norito-serialisable `[u8; 32]` newtype used wherever hashes appear in manifests or telemetry. Code: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` represents 128-bit SM4 keys and is shared between host syscalls and data-model fixtures. Code: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  These types sit alongside the existing Ed25519/BLS/ML-DSA primitives and are available to data-model consumers (Torii, SDKs, genesis tooling) once the `sm` feature is enabled.

Important traits: `Identifiable`, `Registered`/`Registrable` (builder pattern), `HasMetadata`, `IntoKeyValue`. Code: `crates/iroha_data_model/src/lib.rs`.

Events: Every entity has events emitted on mutations (create/delete/owner changed/metadata changed, etc.). Code: `crates/iroha_data_model/src/events/`.

---

## Parameters (Chain Configuration)
- Families: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, plus `custom: BTreeMap`.
- Single enums for diffs: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Aggregator: `Parameters`. Code: `crates/iroha_data_model/src/parameter/system.rs`.

Setting parameters (ISI): `SetParameter(Parameter)` updates the corresponding field and emits `ConfigurationEvent::Changed`. Code: `crates/iroha_data_model/src/isi/transparent.rs`, executor in `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## Instruction Serialization and Registry
- Core trait: `Instruction: Send + Sync + 'static` with `dyn_encode()`, `as_any()`, stable `id()` (defaults to concrete type name).
- `InstructionBox`: `Box<dyn Instruction>` wrapper. Clone/Eq/Ord operate on `(type_id, encoded_bytes)` so equality is by value.
- Norito serde for `InstructionBox` serializes as `(String wire_id, Vec<u8> payload)` (falls back to `type_name` if no wire ID). Deserialization uses a global `InstructionRegistry` mapping identifiers to constructors. Default registry includes all built‑in ISI. Code: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: Types, Semantics, Errors
Execution is implemented via `Execute for <Instruction>` in `iroha_core::smartcontracts::isi`. Below lists the public effects, preconditions, emitted events, and errors.

### Register / Unregister
Types: `Register<T: Registered>` and `Unregister<T: Identifiable>`, with sum types `RegisterBox`/`UnregisterBox` covering concrete targets.

- Register Peer: inserts into world peers set.
  - Preconditions: must not already exist.
  - Events: `PeerEvent::Added`.
  - Errors: `Repetition(Register, PeerId)` if duplicate; `FindError` on lookups. Code: `core/.../isi/world.rs`.

- Register Domain: builds from `NewDomain` with `owned_by = authority`. Disallowed: `genesis` domain.
  - Preconditions: domain non‑existence; not `genesis`.
  - Events: `DomainEvent::Created`.
  - Errors: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Code: `core/.../isi/world.rs`.

- Register Account: builds from `NewAccount`, disallowed in `genesis` domain; `genesis` account cannot be registered.
  - Preconditions: domain must exist; account non‑existence; not in genesis domain.
  - Events: `DomainEvent::Account(AccountEvent::Created)`.
  - Errors: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Code: `core/.../isi/domain.rs`.

- Register AssetDefinition: builds from builder; sets `owned_by = authority`.
  - Preconditions: definition non‑existence; domain exists.
  - Events: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Errors: `Repetition(Register, AssetDefinitionId)`. Code: `core/.../isi/domain.rs`.

- Register NFT: builds from builder; sets `owned_by = authority`.
  - Preconditions: NFT non‑existence; domain exists.
  - Events: `DomainEvent::Nft(NftEvent::Created)`.
  - Errors: `Repetition(Register, NftId)`. Code: `core/.../isi/nft.rs`.

- Register Role: builds from `NewRole { inner, grant_to }` (first owner recorded via account‑role mapping), stores `inner: Role`.
  - Preconditions: role non‑existence.
  - Events: `RoleEvent::Created`.
  - Errors: `Repetition(Register, RoleId)`. Code: `core/.../isi/world.rs`.

- Register Trigger: stores the trigger in the appropriate trigger set by filter kind.
  - Preconditions: If filter is not mintable, `action.repeats` must be `Exactly(1)` (otherwise `MathError::Overflow`). Duplicate IDs prohibited.
  - Events: `TriggerEvent::Created(TriggerId)`.
  - Errors: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` on conversion/validation failures. Code: `core/.../isi/triggers/mod.rs`.

- Unregister Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger: removes the target; emits deletion events. Additional cascading removals:
  - Unregister Domain: removes all accounts in domain, their roles, permissions, tx-sequence counters, account labels, and UAID bindings; deletes their assets (and per-asset metadata); removes all asset definitions in the domain; deletes NFTs in the domain and any NFTs owned by the removed accounts; removes triggers whose authority domain matches. Events: `DomainEvent::Deleted`, plus per-item deletion events. Errors: `FindError::Domain` if missing. Code: `core/.../isi/world.rs`.
  - Unregister Account: removes account’s permissions, roles, tx-sequence counter, account label mapping, and UAID bindings; deletes assets owned by the account (and per-asset metadata); deletes NFTs owned by the account; removes triggers whose authority is that account. Events: `AccountEvent::Deleted`, plus `NftEvent::Deleted` per removed NFT. Errors: `FindError::Account` if missing. Code: `core/.../isi/domain.rs`.
  - Unregister AssetDefinition: deletes all assets of that definition and their per-asset metadata. Events: `AssetDefinitionEvent::Deleted` and `AssetEvent::Deleted` per asset. Errors: `FindError::AssetDefinition`. Code: `core/.../isi/domain.rs`.
  - Unregister NFT: removes NFT. Events: `NftEvent::Deleted`. Errors: `FindError::Nft`. Code: `core/.../isi/nft.rs`.
  - Unregister Role: revokes the role from all accounts first; then removes the role. Events: `RoleEvent::Deleted`. Errors: `FindError::Role`. Code: `core/.../isi/world.rs`.
  - Unregister Trigger: removes trigger if present; duplicate unregister yields `Repetition(Unregister, TriggerId)`. Events: `TriggerEvent::Deleted`. Code: `core/.../isi/triggers/mod.rs`.

### Mint / Burn
Types: `Mint<O, D: Identifiable>` and `Burn<O, D: Identifiable>`, boxed as `MintBox`/`BurnBox`.

- Asset (Numeric) mint/burn: adjusts balances and definition’s `total_quantity`.
  - Preconditions: `Numeric` value must satisfy `AssetDefinition.spec()`; mint allowed by `mintable`:
    - `Infinitely`: always allowed.
    - `Once`: allowed exactly once; the first mint flips `mintable` to `Not` and emits `AssetDefinitionEvent::MintabilityChanged`, plus a detailed `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` for auditability.
    - `Limited(n)`: allows `n` additional mint operations. Each successful mint decrements the counter; when it reaches zero the definition flips to `Not` and emits the same `MintabilityChanged` events as above.
    - `Not`: error `MintabilityError::MintUnmintable`.
  - State changes: creates asset if missing on mint; removes asset entry if balance becomes zero on burn.
  - Events: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (when `Once` or `Limited(n)` exhausts its allowance).
  - Errors: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Code: `core/.../isi/asset.rs`.

- Trigger repetitions mint/burn: changes `action.repeats` count for a trigger.
  - Preconditions: on mint, filter must be mintable; arithmetic must not overflow/underflow.
  - Events: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Errors: `MathError::Overflow` on invalid mint; `FindError::Trigger` if missing. Code: `core/.../isi/triggers/mod.rs`.

### Transfer
Types: `Transfer<S: Identifiable, O, D: Identifiable>`, boxed as `TransferBox`.

- Asset (Numeric): subtract from source `AssetId`, add to destination `AssetId` (same definition, different account). Delete zeroed source asset.
  - Preconditions: source asset exists; value satisfies `spec`.
  - Events: `AssetEvent::Removed` (source), `AssetEvent::Added` (destination).
  - Errors: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Code: `core/.../isi/asset.rs`.

- Domain ownership: changes `Domain.owned_by` to destination account.
  - Preconditions: both accounts exist; domain exists.
  - Events: `DomainEvent::OwnerChanged`.
  - Errors: `FindError::Account/Domain`. Code: `core/.../isi/domain.rs`.

- AssetDefinition ownership: changes `AssetDefinition.owned_by` to destination account.
  - Preconditions: both accounts exist; definition exists; source must currently own it.
  - Events: `AssetDefinitionEvent::OwnerChanged`.
  - Errors: `FindError::Account/AssetDefinition`. Code: `core/.../isi/account.rs`.

- NFT ownership: changes `Nft.owned_by` to destination account.
  - Preconditions: both accounts exist; NFT exists; source must currently own it.
  - Events: `NftEvent::OwnerChanged`.
  - Errors: `FindError::Account/Nft`, `InvariantViolation` if source doesn’t own the NFT. Code: `core/.../isi/nft.rs`.

### Metadata: Set/Remove Key‑Value
Types: `SetKeyValue<T>` and `RemoveKeyValue<T>` with `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. Boxed enums provided.

- Set: inserts or replaces `Metadata[key] = Json(value)`.
- Remove: removes the key; error if missing.
- Events: `<Target>Event::MetadataInserted` / `MetadataRemoved` with the old/new values.
- Errors: `FindError::<Target>` if the target doesn’t exist; `FindError::MetadataKey` on missing key for removal. Code: `crates/iroha_data_model/src/isi/transparent.rs` and executor impls per target.

### Permissions and Roles: Grant / Revoke
Types: `Grant<O, D>` and `Revoke<O, D>`, with boxed enums for `Permission`/`Role` to/from `Account`, and `Permission` to/from `Role`.

- Grant Permission to Account: adds `Permission` unless already inherent. Events: `AccountEvent::PermissionAdded`. Errors: `Repetition(Grant, Permission)` if duplicate. Code: `core/.../isi/account.rs`.
- Revoke Permission from Account: removes if present. Events: `AccountEvent::PermissionRemoved`. Errors: `FindError::Permission` if absent. Code: `core/.../isi/account.rs`.
- Grant Role to Account: inserts `(account, role)` mapping if absent. Events: `AccountEvent::RoleGranted`. Errors: `Repetition(Grant, RoleId)`. Code: `core/.../isi/account.rs`.
- Revoke Role from Account: removes mapping if present. Events: `AccountEvent::RoleRevoked`. Errors: `FindError::Role` if absent. Code: `core/.../isi/account.rs`.
- Grant Permission to Role: rebuilds role with permission added. Events: `RoleEvent::PermissionAdded`. Errors: `Repetition(Grant, Permission)`. Code: `core/.../isi/world.rs`.
- Revoke Permission from Role: rebuilds role without that permission. Events: `RoleEvent::PermissionRemoved`. Errors: `FindError::Permission` if absent. Code: `core/.../isi/world.rs`.

### Triggers: Execute
Type: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Behavior: enqueues an `ExecuteTriggerEvent { trigger_id, authority, args }` for the trigger subsystem. Manual execution is allowed only for by-call triggers (`ExecuteTrigger` filter); the filter must match and the caller must be the trigger action authority or hold `CanExecuteTrigger` for that authority. When a user-provided executor is active, trigger execution is validated by the runtime executor and consumes the transaction’s executor fuel budget (base `executor.fuel` plus optional metadata `additional_fuel`).
- Errors: `FindError::Trigger` if not registered; `InvariantViolation` if called by non‑authority. Code: `core/.../isi/triggers/mod.rs` (and tests in `core/.../smartcontracts/isi/mod.rs`).

### Upgrade and Log
- `Upgrade { executor }`: migrates the executor using provided `Executor` bytecode, updates executor and its data model, emits `ExecutorEvent::Upgraded`. Errors: wrapped as `InvalidParameterError::SmartContract` on migration failure. Code: `core/.../isi/world.rs`.
- `Log { level, msg }`: emits a node log with the given level; no state changes. Code: `core/.../isi/world.rs`.

### Error Model
Common envelope: `InstructionExecutionError` with variants for evaluation errors, query failures, conversions, entity not found, repetition, mintability, math, invalid parameter, and invariant violation. Enumerations and helpers are in `crates/iroha_data_model/src/isi/mod.rs` under `pub mod error`.

---

## Transactions and Executables
- `Executable`: either `Instructions(ConstVec<InstructionBox>)` or `Ivm(IvmBytecode)`; bytecode serializes as base64. Code: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: constructs, signs, and packages an executable with metadata, `chain_id`, `authority`, `creation_time_ms`, optional `ttl_ms`, and `nonce`. Code: `crates/iroha_data_model/src/transaction/`.
- At runtime, `iroha_core` executes `InstructionBox` batches via `Execute for InstructionBox`, downcasting to the appropriate `*Box` or concrete instruction. Code: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Runtime executor validation budget (user-provided executor): base `executor.fuel` from parameters plus optional transaction metadata `additional_fuel` (`u64`), shared across instruction/trigger validations within the transaction.

---

## Invariants and Notes (from tests and guards)
- Genesis protections: cannot register the `genesis` domain or accounts in `genesis` domain; `genesis` account cannot be registered. Code/tests: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Numeric assets must satisfy their `NumericSpec` on mint/transfer/burn; spec mismatch yields `TypeError::AssetNumericSpec`.
- Mintability: `Once` allows a single mint and then flips to `Not`; `Limited(n)` allows exactly `n` mints before flipping to `Not`. Attempts to forbid minting on `Infinitely` cause `MintabilityError::ForbidMintOnMintable`, and configuring `Limited(0)` yields `MintabilityError::InvalidMintabilityTokens`.
- Metadata operations are key‑exact; removing a non‑existent key is an error.
- Trigger filters can be non‑mintable; then `Register<Trigger>` only permits `Exactly(1)` repeats.
- Trigger metadata key `__enabled` (bool) gates execution; missing defaults to enabled, and disabled triggers are skipped across data/time/by-call paths.
- Determinism: all arithmetic uses checked operations; under/overflow returns typed math errors; zero balances drop asset entries (no hidden state).

---

## Practical Examples
- Minting and transfer:
  - `Mint::asset_numeric(10, asset_id)` → adds 10 if allowed by spec/mintability; events: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → moves 5; events for removal/addition.
- Metadata updates:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert; removal via `RemoveKeyValue::account(...)`.
- Role/permission management:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)`, and their `Revoke` counterparts.
- Trigger lifecycle:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` with mintability check implied by filter; `ExecuteTrigger::new(id).with_args(&args)` must match configured authority.
  - Triggers can be disabled by setting metadata key `__enabled` to `false` (missing defaults to enabled); toggle via `SetKeyValue::trigger` or the IVM `set_trigger_enabled` syscall.
  - Trigger storage is repaired on load: duplicate ids, mismatched ids, and triggers referencing missing bytecode are dropped; bytecode reference counts are recomputed.
  - If a trigger's IVM bytecode is missing at execution time, the trigger is removed and the execution is treated as a no-op with a failure outcome.
  - Depleted triggers are removed immediately; if a depleted entry is encountered during execution it is pruned and treated as missing.
- Parameter update:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` updates and emits `ConfigurationEvent::Changed`.

---

## Traceability (selected sources)
 - Data model core: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI definitions and registry: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI execution: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Events: `crates/iroha_data_model/src/events/**`.
 - Transactions: `crates/iroha_data_model/src/transaction/**`.

If you want this spec expanded into a rendered API/behavior table or cross‑linked to every concrete event/error, say the word and I’ll extend it.
