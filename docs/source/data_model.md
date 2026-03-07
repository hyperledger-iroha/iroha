# Iroha v2 Data Model – Deep Dive

This document explains the structures, identifiers, traits, and protocols that form the Iroha v2 data model, as implemented in the `iroha_data_model` crate and used across the workspace. It is meant to be a precise reference you can review and propose updates to.

## Scope and Foundations

- Purpose: Provide canonical types for domain objects (domains, accounts, assets, NFTs, roles, permissions, peers), state-changing instructions (ISI), queries, triggers, transactions, blocks, and parameters.
- Serialization: All public types derive Norito codecs (`norito::codec::{Encode, Decode}`) and schema (`iroha_schema::IntoSchema`). JSON is used selectively (e.g., for HTTP and `Json` payloads) behind feature flags.
- IVM note: Certain deserialization-time validations are disabled when targeting the Iroha Virtual Machine (IVM), since the host performs validation before invoking contracts (see crate docs in `src/lib.rs`).
- FFI gates: Some types are conditionally annotated for FFI via `iroha_ffi` behind `ffi_export`/`ffi_import` to avoid overhead when FFI is not needed.

## Core Traits and Helpers

- `Identifiable`: Entities have a stable `Id` and `fn id(&self) -> &Self::Id`. Should be derived with `IdEqOrdHash` for map/set friendliness.
- `Registrable`/`Registered`: Many entities (e.g., `Domain`, `AssetDefinition`, `Role`) use a builder pattern. `Registered` ties the runtime type to a lightweight builder type (`With`) suitable for registration transactions.
- `HasMetadata`: Unified access to a key/value `Metadata` map.
- `IntoKeyValue`: Storage split helper to store `Key` (ID) and `Value` (data) separately to reduce duplication.
- `Owned<T>`/`Ref<'world, K, V>`: Lightweight wrappers used in storages and query filters to avoid unnecessary copies.

## Names and Identifiers

- `Name`: Valid textual identifier. Disallows whitespace and reserved characters `@`, `#`, `$` (used in composite IDs). Constructible via `FromStr` with validation. Names are normalized to Unicode NFC on parse (canonically equivalent spellings are treated as identical and stored composed). The special name `genesis` is reserved (checked case-insensitively).
- `IdBox`: A sum-type envelope for any supported ID (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `PeerId`, `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Useful for generic flows and Norito encoding as a single type.
- `ChainId`: Opaque chain identifier used for replay protection in transactions.

String forms of IDs (round-trippable with `Display`/`FromStr`):
- `DomainId`: `name` (e.g., `wonderland`).
- `AccountId`: canonical identifier encoded via `AccountAddress`, which exposes IH58, Sora compressed (`sora…`), and canonical hex codecs (`AccountAddress::to_ih58`, `to_compressed_sora`, `canonical_hex`, `parse_encoded`). IH58 is the preferred account format; the `sora…` form is second-best for Sora-only UX. The human-friendly routing alias `alias@domain` is preserved for UX but is no longer treated as the authoritative identifier. Torii normalises incoming strings through `AccountAddress::parse_encoded`. Account IDs support both single-key and multisig controllers.
- `AssetDefinitionId`: `asset#domain` (e.g., `xor#soramitsu`).
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`: `nft$domain` (e.g., `rose$garden`).
- `PeerId`: `public_key` (peer equality is by public key).

## Entities

### Domain
- `DomainId { name: Name }` – unique name.
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`.
- Builder: `NewDomain` with `with_logo`, `with_metadata`, then `Registrable::build(authority)` sets `owned_by`.

### Account
- `AccountId { domain: DomainId, controller: AccountController }` (controller = single key or multisig policy).
- `Account { id, metadata, label?, uaid? }` — `label` is an optional stable alias used by rekey records, `uaid` carries the optional Nexus-wide [Universal Account ID](./universal_accounts_guide.md).
- Builder: `NewAccount` via `Account::new(id)`; `HasMetadata` for both builder and entity.

### Asset Definitions and Assets
- `AssetDefinitionId { domain: DomainId, name: Name }`.
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Builders: `AssetDefinition::new(id, spec)` or convenience `numeric(id)`; setters for `metadata`, `mintable`, `owned_by`.
- `AssetId { account: AccountId, definition: AssetDefinitionId }`.
- `Asset { id, value: Numeric }` with storage-friendly `AssetEntry`/`AssetValue`.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` exposed for summary APIs.

### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (content is arbitrary key/value metadata).
- Builder: `NewNft` via `Nft::new(id, content)`.

### Roles and Permissions
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` with builder `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` – the `name` and payload schema must align with the active `ExecutorDataModel` (see below).

### Peers
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` and parsable `public_key@address` string form.

### Cryptographic primitives (feature `sm`)
- `Sm2PublicKey` and `Sm2Signature`: SEC1-compliant points and fixed-width `r∥s` signatures for SM2. Constructors validate curve membership and distinguishing IDs; Norito encoding mirrors the canonical representation used by `iroha_crypto`.
- `Sm3Hash`: `[u8; 32]` newtype representing the GM/T 0004 digest, used in manifests, telemetry, and syscall responses.
- `Sm4Key`: 128-bit symmetric key wrapper shared between host syscalls and data-model fixtures.
These types sit alongside the existing Ed25519/BLS/ML-DSA primitives and become part of the public schema once the workspace is built with `--features sm`.

### Triggers and Events
- `TriggerId { name: Name }` and `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` or `Exactly(u32)`; ordering and depletion utilities included.
  - Safety: `TriggerCompleted` cannot be used as an action’s filter (validated during (de)serialization).
- `EventBox`: sum type for pipeline, pipeline-batch, data, time, execute-trigger, and trigger-completed events; `EventFilterBox` mirrors that for subscriptions and trigger filters.

## Parameters and Configuration

- System parameter families (all `Default`ed, carry getters, and convert to individual enums):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` groups all families and a `custom: BTreeMap<CustomParameterId, CustomParameter>`.
- Single-parameter enums: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` for diff-like updates and iteration.
- Custom parameters: executor-defined, carried as `Json`, identified by `CustomParameterId` (a `Name`).

## ISI (Iroha Special Instructions)

- Core trait: `Instruction` with `dyn_encode`, `as_any`, and a stable per-type identifier `id()` (defaults to the concrete type name). All instructions are `Send + Sync + 'static`.
- `InstructionBox`: owned `Box<dyn Instruction>` wrapper with clone/eq/ord implemented via type ID + encoded bytes.
- Built-in instruction families are organized under:
  - `mint_burn`, `transfer`, `register`, and a `transparent` bundle of helpers.
  - Type enums for meta flows: `InstructionType`, boxed sums like `SetKeyValueBox` (domain/account/asset_def/nft/trigger).
- Errors: rich error model under `isi::error` (evaluation type errors, find errors, mintability, math, invalid parameters, repetition, invariants).
- Instruction registry: the `instruction_registry!{ ... }` macro builds a runtime decode registry keyed by type name. Used by `InstructionBox` clone and Norito serde to achieve dynamic (de)serialization. If no registry has been explicitly set via `set_instruction_registry(...)`, a built-in default registry with all core ISI is lazily installed on first use to keep binaries robust.

## Transactions

- `Executable`: either `Instructions(ConstVec<InstructionBox>)` or `Ivm(IvmBytecode)`. `IvmBytecode` serializes as base64 (transparent newtype over `Vec<u8>`).
- `TransactionBuilder`: constructs a transaction payload with `chain`, `authority`, `creation_time_ms`, optional `time_to_live_ms` and `nonce`, `metadata`, and an `Executable`.
  - Helpers: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_creation_time`, `sign`.
- `SignedTransaction` (versioned with `iroha_version`): carries `TransactionSignature` and payload; provides hashing and signature verification.
- Entrypoints and results:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` with hashing helpers.
  - `ExecutionStep(ConstVec<InstructionBox>)`: a single ordered batch of instructions in a transaction.

## Blocks

- `SignedBlock` (versioned) encapsulates:
  - `signatures: BTreeSet<BlockSignature>` (from validators),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (secondary execution state) containing `time_triggers`, entry/result Merkle trees, `transaction_results`, and `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Utilities: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `add_signature`, `replace_signatures`.
- Merkle roots: transaction entrypoints and results are committed via Merkle trees; result Merkle root is placed into the block header.
- Block inclusion proofs (`BlockProofs`) expose both entry/result Merkle proofs and the `fastpq_transcripts` map so off-chain provers can fetch the transfer deltas associated with a transaction hash.
- `ExecWitness` messages (streamed via Torii and piggy-backed on consensus gossip) now include both `fastpq_transcripts` and prover-ready `fastpq_batches: Vec<FastpqTransitionBatch>` with embedded `public_inputs` (dsid, slot, roots, perm_root, tx_set_hash), so external provers can ingest canonical FASTPQ rows without re-encoding transcripts.

## Queries

- Two flavors:
  - Singular: implement `SingularQuery<Output>` (e.g., `FindParameters`, `FindExecutorDataModel`).
  - Iterable: implement `Query<Item>` (e.g., `FindAccounts`, `FindAssets`, `FindDomains`, etc.).
- Type-erased forms:
  - `QueryBox<T>` is a boxed, erased `Query<Item = T>` with Norito serde backed by a global registry.
  - `QueryWithFilter<T> { query, predicate, selector }` pairs a query with a DSL predicate/selector; converts into an erased iterable query via `From`.
- Registry and codecs:
  - `query_registry!{ ... }` builds a global registry mapping concrete query types to constructors by type name for dynamic decode.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` and `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` is a sum-type over homogeneous vectors (e.g., `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), plus tuple and extension helpers for efficient pagination.
- DSL: Implemented in `query::dsl` with projection traits (`HasProjection<PredicateMarker>` / `SelectorMarker`) for compile-time-checked predicates and selectors. A `fast_dsl` feature exposes a lighter variant if needed.

## Executor and Extensibility

- `Executor { bytecode: IvmBytecode }`: the validator-executed code bundle.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` declares the executor-defined domain:
  - Custom configuration parameters,
  - Custom instruction identifiers,
  - Permission token identifiers,
  - A JSON schema describing custom types for client tooling.
- Customization samples exist under `data_model/samples/executor_custom_data_model` demonstrating:
  - Custom permission token via `iroha_executor_data_model::permission::Permission` derive,
  - Custom parameter defined as a type convertible into `CustomParameter`,
  - Custom instructions serialized into `CustomInstruction` for execution.

### CustomInstruction (executor-defined ISI)

- Type: `isi::CustomInstruction { payload: Json }` with stable wire id `"iroha.custom"`.
- Purpose: envelope for executor-specific instructions in private/consortium networks or for prototyping, without forking the public data model.
- Default executor behavior: the built-in executor in `iroha_core` does not execute `CustomInstruction` and will panic if encountered. A custom executor must downcast `InstructionBox` to `CustomInstruction` and deterministically interpret the payload on all validators.
- Norito: encodes/decodes via `norito::codec::{Encode, Decode}` with schema included; the `Json` payload is serialized deterministically. Round-trips are stable so long as the instruction registry includes `CustomInstruction` (it is part of the default registry).
- IVM: Kotodama compiles to IVM bytecode (`.to`) and is the recommended path for application logic. Only use `CustomInstruction` for executor-level extensions that cannot yet be expressed in Kotodama. Ensure determinism and identical executor binaries across peers.
- Not for public networks: do not use for public chains where heterogeneous executors risk consensus forks. Prefer proposing new built-in ISI upstream when you need platform features.

## Metadata

- `Metadata(BTreeMap<Name, Json>)`: key/value store attached to multiple entities (`Domain`, `Account`, `AssetDefinition`, `Nft`, triggers, and transactions).
- API: `contains`, `iter`, `get`, `insert`, and (with `transparent_api`) `remove`.

## Features and Determinism

- Features control optional APIs (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `http`, `fault_injection`).
- Determinism: All serialization uses Norito encoding to be portable across hardware. IVM bytecode is an opaque byte blob; execution must not introduce non-deterministic reductions. The host validates transactions and supplies inputs to IVM deterministically.

### Transparent API (`transparent_api`)

- Purpose: exposes full, mutable access to the `#[model]` structs/enums for internal components such as Torii, executors, and integration tests. Without it, those items are intentionally opaque so external SDKs only see safe constructors and encoded payloads.
- Mechanics: the `iroha_data_model_derive::model` macro rewrites each public field with `#[cfg(feature = "transparent_api")] pub` and keeps a private copy for the default build. Enabling the feature flips those cfgs, so destructuring `Account`, `Domain`, `Asset`, etc. becomes legal outside their defining modules.
- Surface detection: the crate exports a `TRANSPARENT_API: bool` constant (generated into either `transparent_api.rs` or `non_transparent_api.rs`). Downstream code can check this flag and branch when it needs to fall back to opaque helpers.
- Enabling: add `features = ["transparent_api"]` to the dependency in `Cargo.toml`. Workspace crates that need the JSON projection (e.g., `iroha_torii`) forward the flag automatically, but third-party consumers should keep it off unless they control the deployment and accept the broader API surface.

## Quick Examples

Create a domain and account, define an asset, and build a transaction with instructions:

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(domain_id.clone(), kp.public_key().clone());
let new_account = Account::new(account_id.clone()).with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

Query accounts and assets with the DSL:

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

Use IVM smart contract bytecode:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## Versioning

- `SignedTransaction`, `SignedBlock`, and `SignedQuery` are canonical Norito-encoded structs. Each implements `iroha_version::Version` to prefix their payload with the current ABI version (currently `1`) when encoded via `EncodeVersioned`.

## Review Notes / Potential Updates

- Query DSL: consider documenting a stable user-facing subset and examples for common filters/selectors.
- Instruction families: expand public docs listing the built-in ISI variants exposed by `mint_burn`, `register`, `transfer`.

---
If any part needs more depth (e.g., full ISI catalog, complete query registry list, or block header fields), let me know and I’ll extend those sections accordingly.
