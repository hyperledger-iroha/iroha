# Iroha 3 Data Model – Deep Dive

This document explains the structures, identifiers, traits, and protocols that
form the first-release Iroha 3 data model, as implemented in the
`iroha_data_model` crate and used across the workspace.

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

- `Name`: Valid textual identifier. Disallows whitespace and reserved characters `@`, `#`, `$` (used in composite IDs). Constructible via `FromStr` with validation. Names must arrive in their exact Unicode NFC spelling; alternate canonically equivalent spellings are rejected rather than rewritten. The special name `genesis` is reserved (checked case-insensitively).
- `IdBox`: A sum-type envelope for any supported ID (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `PeerId`, `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Useful for generic flows and Norito encoding as a single type.
- `ChainId`: Opaque chain identifier used for replay protection in transactions.

String forms of IDs (round-trippable with `Display`/`FromStr`):
- `DomainId`: `name` (e.g., `wonderland`).
- `AccountId`: canonical domainless account identifier encoded via `AccountAddress` as I105 only. Strict parser inputs must be canonical I105; domain suffixes (`@domain`), account-alias literals, canonical hex parser input, legacy `norito:` payloads, and `uaid:`/`opaque:` account parser forms are rejected. On-chain account aliases use `name@domain.dataspace` or `name@dataspace` and resolve to canonical `AccountId` values.
- `AssetDefinitionId`: canonical unprefixed Base58 address over the canonical asset-definition bytes. This is the public asset ID. On-chain asset aliases use `name#domain.dataspace` or `name#dataspace` and resolve only to this canonical Base58 asset ID.
- `AssetId`: public asset identifier in canonical bare Base58 form. Asset aliases like `name#dataspace` or `name#domain.dataspace` resolve to `AssetId`. Internal ledger holdings may additionally expose split `asset + account + optional dataspace` fields where needed, but that composite shape is not the public `AssetId`.
- `NftId`: `nft$domain` (e.g., `rose$garden`).
- `PeerId`: `public_key` (peer equality is by public key).

## Entities

### Domain
- `DomainId { name: Name }` – unique name.
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Builder: `NewDomain` with `with_logo`, `with_metadata`, then `Registrable::build(authority)` sets `owned_by`.

### Account
- `AccountId` is the canonical domainless account identity keyed by the controller and encoded as canonical I105.
- `Account { id, metadata, label?, uaid?, opaque_ids[] }` — `label` is an optional primary `AccountAlias` used by rekey records, `uaid` carries the optional Nexus-wide [Universal Account ID](./universal_accounts_guide.md), and `opaque_ids` tracks hidden identifiers bound to that UAID. A canonical UAID is always a Blake2b-256 digest with the final-byte LSB set to `1`; account onboarding must provide it explicitly. Stored account state no longer carries any linked-domain field.
- Builders:
  - `NewAccount` via `Account::new(id)` registers the canonical domainless account subject.
- Alias model:
  - Canonical account identity never includes a domain or dataspace segment.
  - `AccountAlias` values are separate SNS bindings layered on top of `AccountId`.
  - Domain-qualified aliases such as `merchant@banka.paynet` carry both a domain and dataspace in the alias binding.
  - Dataspace-root aliases such as `merchant@paynet` carry only the dataspace and therefore pair naturally with `Account::new(...)`.
  - Tests and fixtures should seed the universal `AccountId` first, then add alias leases, alias permissions, and any domain-owned state separately instead of encoding domain assumptions into the account identity itself.
  - Public singular account lookup now focuses on aliases (`FindAliasesByAccountId`); account identity itself stays domainless.

### Asset Definitions and Assets
- `AssetDefinitionId { aid_bytes: [u8; 16] }` exposed textually as an unprefixed Base58 address with versioning and checksum.
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Quantity }`.
  - `name` is required human-facing display text and must not contain `#`/`@`.
  - `alias` is optional and must be one of:
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    with the left segment exactly matching `AssetDefinition.name`.
  - Alias lease state is stored authoritatively in the persisted alias-binding record; the inline `alias` field is derived when definitions are read back through core/Torii APIs.
  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, where `status` is one of `permanent`, `leased_active`, `leased_grace`, or `expired_pending_cleanup`.
  - Alias resolution uses the latest committed block timestamp rather than node wall clock. Once `grace_until_ms` has passed, alias selectors stop resolving immediately even if sweep cleanup has not removed the stale binding yet; direct definition reads may still report the lingering binding as `expired_pending_cleanup`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Builders: `AssetDefinition::new(id, spec)` or convenience `numeric(id)`; `name` is required and must be set via `.with_name(...)`.
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Quantity }` with storage-friendly `AssetEntry`/`AssetValue`.
- `AssetBalanceScope`: `Global` for unrestricted balances and `Dataspace(DataSpaceId)` for dataspace-restricted balances.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Quantity>` exposed for summary APIs.

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
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, retry_policy, metadata }`.
  - Construction: `Action::new(...)` is the single fallible constructor and returns `ActionValidationError` when the filter is forbidden, a filter-bound authority conflicts with the action authority, or retry-policy constraints are violated. `with_retry_policy(...)` is fallible for the same reason; there is no generally available infallible constructor.
  - `Repeats`: `Indefinitely` or `Exactly(u32)`; ordering and depletion utilities included.
  - `retry_policy`: optional scheduled-time-trigger retry settings; non-scheduled triggers reject retry policies.
  - Safety: `TriggerCompleted` cannot be used as an action’s filter (validated during (de)serialization).
  - Enabled state: triggers are enabled by default. The reserved `__enabled` metadata flag disables execution when set to `false` or `0`; malformed values fail closed and are treated as disabled. Active-trigger queries return only enabled triggers with remaining repeats.
  - Pipeline triggers: state-mutating pipeline triggers are deterministic-only. They may target transaction `Approved`/`Rejected` events or block `Approved` events derived from committed block validation. Local queue, warning, witness, merge-ledger, created, committed, and rejected-block notifications remain subscription events only.
- `EventBox`: sum type for pipeline, pipeline-batch, data, time, execute-trigger, and trigger-completed events; `EventFilterBox` mirrors that for subscriptions and trigger filters.
- `TriggerCompletedEvent`: reports the trigger id, `trigger_execution_hash`, `step_index`, and success/failure outcome for each trigger invocation. `trigger_execution_hash` identifies the actual trigger invocation rather than an external transaction, and `step_index` is the zero-based position inside the trigger sequence including chained data triggers.

## Parameters and Configuration

- System parameter families (all `Default`ed, carry getters, and convert to individual enums):
- `SumeragiParameters { block_cadence_ms, max_clock_drift_ms, key_activation_lead_blocks, key_overlap_grace_blocks, key_expiry_grace_blocks, key_require_hsm, key_allowed_algorithms, key_allowed_hsm_providers }`. The cadence and key policy are signed chain context; only the clock-drift variant remains mutable through the generic parameter enum.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes, max_time_to_live_ms }`. `max_time_to_live_ms` defaults to one day and bounds every signature-bound transaction lifetime.
  - `SmartContractParameters { fuel, memory, execution_depth, max_output_items, max_output_bytes }`. The output limits bound the aggregate queued instructions, durable writes, FastPQ entries, completed AXT states, and access artifacts retained by one IVM execution.
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

- `Executable`: `Instructions(ConstVec<InstructionBox>)`, `ContractCall(ContractInvocation)`, `Ivm(IvmBytecode)`, `IvmProved(IvmProved)`, or a flat ordered `Batch(ConstVec<ExecutableBatchItem>)`. The append-only mixed variant is introduced by `DATA_MODEL_VERSION = 3`: `Executable::Batch` uses tag `4`, while its item tags are `0` for `Instruction(InstructionBox)` and `1` for `ContractCall(ContractInvocation)`. Existing executable tags `0..=3` and their canonical bytes remain unchanged. Raw IVM bytecode and nested batches are excluded from batch items. `IvmBytecode` serializes as base64 (transparent newtype over `Vec<u8>`).
- `TransactionBuilder`: constructs the ten-field first-release transaction
  payload with `domain`, `authority`, `creation_time_ms`, `instructions`,
  `time_to_live_ms`, `nonce`, `fee_payment`, the signature-bound
  `admission_intent`, `metadata`, and `attachments`.
  - Helpers: `with_instructions`, `with_executable_batch`, `with_bytecode`,
    `with_executable`, `with_fee_payment_intent`, `with_admission_intent`,
    `with_metadata`, `with_attachments`, `set_nonce`, `set_ttl`,
    `set_creation_time`, `sign`.
  - Public Torii submission requires `TransactionAdmissionIntent::QueuePlanSynced`;
    omission and unknown intent tags fail closed rather than consulting metadata.
  - Admission rejects an empty mixed batch and schedules a valid one as a global live-state barrier. Its items execute in input order against one transaction view and commit or roll back as one atomic unit. A transaction batch containing a contract call requires one signature-bound gas limit shared by all of its explicit ISIs and calls; fees settle once for the transaction.
- Trigger actions may also carry a mixed batch. One trigger invocation preserves the same ordered atomic semantics and shares one deterministic trigger gas budget across all items.
- `SignedTransaction` (versioned with `iroha_version`): carries `TransactionSignature` and payload; provides hashing and signature verification.
- Entrypoints and results:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` with hashing helpers.
  - `ExecutionStep(ConstVec<InstructionBox>)`: a single ordered batch of instructions in a transaction.

The current SDK/node compatibility handshake is `DATA_MODEL_VERSION = 4`.
Version 3 remains the historical introduction point for the append-only mixed
batch above. Version 4 changes canonical validation-fee governance bytes by
requiring exact `plain_electorate_rules` in policy and payout-lifecycle
proposal instructions, retaining those rules in enacted registry entries, and
binding finalized authorization to a frozen PLAIN electorate. SDKs must reject
a node advertising any other data-model version before submission.

### Validation-fee PLAIN governance

- `ValidationFeePlainElectorateRulesV1` is part of each native proposal
  fingerprint. It fixes the voting asset, bond-escrow account, slash-receiver
  account, ballot amount and duration, citizenship amount, member cap,
  conviction parameters, turnout and approval threshold, and the closed
  proposal-operator eligibility rule. The first-release cap is 256 members;
  Taira retains an exact 150-XOR bond, 3,600-block inclusive referendum window,
  and PLAIN-only finalization.
- `ValidationFeePlainElectorateSnapshotV1` freezes the electorate at the
  referendum's `h_start` boundary after the seven-body approval gate. Its
  canonical, duplicate-free members retain account id, uninterrupted
  `bonded_height`, and exact `bonded_amount`; the snapshot also binds the
  proposal id/operator, capture and gate heights, member count, and a
  domain-separated `roster_root`. The proposal operator must have bonded at or
  before the gate; every other member must have bonded strictly after the gate
  and before capture.
- `ValidationFeeParliamentAuthorizationV1` retains the snapshot root, count,
  capture height, and approval-gate height alongside the proposal fingerprint,
  Parliament roster root, referendum window, PLAIN finalization, and enactment
  height. Registry validation requires these anchors and thresholds to match
  the retained rules, and requires
  `effective_from_height = enacted_at_height + 120,960` exactly.
- Every new validation-fee lock retains the proposal-bound asset, escrow, and
  slash receiver. Lock, release, slash, and restitution move the exact numeric
  balance atomically through those retained identities; missing or mismatched
  custody evidence, or a failed release, retains the lock and fails closed.
  Referenced accounts, asset definitions, and their containing domains cannot
  be unregistered while an active lock or a proposed/approved validation-fee
  proposal still depends on them.

## Blocks

- `SignedBlock` (versioned) encapsulates:
  - `signatures: BTreeSet<BlockSignature>` (from validators),
  - `payload: BlockPayload` with the header, the sole canonical
    `external_entrypoints: Vec<TransactionEntrypoint>` sequence, and the required
    V1 DA, NPoS, and execution-context option fields,
  - `result: BlockResult` (secondary execution state) containing `time_triggers`, entry/result Merkle trees, `transaction_results`, `committed_fragment_count`, `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`, AXT and trigger records, the AXT policy snapshot, and lane-finality statements.
- Utilities: `presigned`, fallible `set_transaction_results(...)` and `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `add_signature`, `replace_signatures`.
- Every `BlockPayload` and `BlockResult` V1 field is present on wire, including
  empty vectors and `None` options. Pre-release layouts that omitted empty
  entrypoints or trailing fields are rejected instead of being hydrated or
  defaulted. Block-local roster evidence is not part of V1: reconfiguration
  authority comes solely from the authenticated Sumeragi v2 height context and
  its parent CommitQC, and the longer pre-release roster-bearing block layouts
  are rejected.
- `BlockHeader` JSON likewise carries every nullable commitment as an explicit
  value or `null`. Consensus signatures use one versioned V1 header projection
  that always includes the nullable NPoS- and execution-context commitments;
  field presence never selects an alternate historical hash layout. The result
  Merkle root remains outside that pre-execution signature projection and is
  validated after deterministic execution.
- Nested execution contexts use the same exact rule: Native-AMX receipts,
  merge references, and every nullable certified-execution binding must appear
  explicitly, while unknown JSON fields and shortened pre-release Norito
  layouts fail closed.
- Merkle commitments bind each application-tree root to its exact non-zero leaf count. Raw typed leaf hashes and internal nodes are separated by the stable `iroha:merkle:leaf:v1\0` and `iroha:merkle:internal:v1\0` domains; the result root is placed into the block header.
- Serialized Merkle trees carry only a V1 hash-scheme discriminant and at most 65,536 canonical leaf-node hashes. Internal nodes and roots are derived caches: decoders rebuild them deterministically and do not accept the retired full-node-vector layout.
- Block inclusion proofs (`BlockProofs`) expose block identity, the authenticated executed-block wire hash, exact entry/result root-and-count commitments, both proofs, and the `fastpq_transcripts` map. The entry commitment covers the full executed-entrypoint order (including scheduled entrypoints), so entry and result proof indices and leaf counts must match. `TrustedBlockProofAnchor::from_untrusted_finality_artifact` verifies the complete Sumeragi-v2 finality artifact and its exact header association before deriving an anchor from the `CommitQC` execution commitment. Verifiers must compare the requested entry hash and index, every commitment value, and the exact FASTPQ transcript projection with that target-specific anchor; values copied from the proof response are not a trust anchor.
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
- DSL: Implemented unconditionally in `query::dsl` with projection traits (`HasProjection<PredicateMarker>` / `SelectorMarker`) for compile-time-checked predicates and selectors. Iterable requests carry a canonical item discriminator plus encoded query, predicate, and selector components.

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

- Features control optional APIs (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `http`, `fault_injection`).
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
use iroha_primitives::numeric::Quantity;

// Domain
let domain_id = DomainId::try_new("wonderland", "universal").unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(kp.public_key().clone());
let new_account = Account::new(account_id.clone())
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id = AssetDefinitionId::new(
    domain_id.clone(),
    "usd".parse().unwrap(),
);
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_name("USD Coin".to_owned())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Quantity::from(100_u32));

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

Asset-definition id / alias quick reference (CLI + Torii):

```bash
# Register an asset definition with a canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#bankb.paynet

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#paynet

# Mint using alias + account components
iroha ledger asset mint \
  --definition-alias pkr#bankb.paynet \
  --account sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV \
  --quantity 500

# Resolve alias to the canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#bankb.paynet"}'
```

Migration note:
- Old `name#domain` asset-definition IDs are not accepted in v1.
- Public asset selectors use one asset-definition format only: canonical Base58 ids. Aliases remain optional selectors, but resolve to the same canonical id.
- Public asset lookups address owned balances with `asset + account + optional scope`; raw encoded `AssetId` literals are an internal representation and are not part of the Torii/CLI selector surface.
- `POST /v1/assets/definitions/query` and `GET /v1/assets/definitions` accept asset-definition filters/sorts over `alias_binding.status`, `alias_binding.lease_expiry_ms`, `alias_binding.grace_until_ms`, and `alias_binding.bound_at_ms` in addition to `id`, `name`, `alias`, and `metadata.*`.

## Versioning

- `SignedTransaction`, `SignedBlock`, and `SignedQuery` are canonical Norito-encoded structs. Each implements `iroha_version::Version` to prefix their payload with the current ABI version (currently `1`) when encoded via `EncodeVersioned`.

## Documentation Ownership

Keep this source-adjacent data-model specification aligned with the
implementation. The public query and instruction catalogs, examples, and
translations are maintained in the sibling `iroha-docs` repository and
published at <https://docs.iroha.tech/>.
