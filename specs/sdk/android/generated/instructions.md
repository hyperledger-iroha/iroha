<!-- Auto-generated via scripts/android_codegen_docs.py -->
# Android Instruction Reference

This file is generated from `instruction_manifest.json`. Do not edit manually.

## `iroha.burn`

> Schema summary: enum variants: Asset (Burn<Quantity, Asset>), TriggerRepetitions (Burn<u32, Trigger>).

- Rust type: `iroha_data_model::isi::mint_burn::BurnBox`
- Schema hash: `361f279124a0aad61978c80ff1c9ce0a`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Asset` | 0 | `Burn<Quantity, Asset>` |
| `TriggerRepetitions` | 1 | `Burn<u32, Trigger>` |

## `iroha.custom`

> Schema summary: struct fields: payload: Json.

- Rust type: `iroha_data_model::isi::transparent::CustomInstruction`
- Schema hash: `6b86902a75600648d186d52cd662b229`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `payload` | `Json` |

## `iroha.execute_trigger`

> Schema summary: struct fields: trigger: TriggerId, args: Json.

- Rust type: `iroha_data_model::isi::transparent::ExecuteTrigger`
- Schema hash: `d8988afd2c1dee721564dd8d57841eff`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `trigger` | `TriggerId` |
| `args` | `Json` |

## `iroha.governance.parliament.attempt.create.v1`

> Schema summary: struct fields: proposal: ProposalKind, attempt_sequence: u32.

- Rust type: `iroha_data_model::isi::governance::parliament::CreateParliamentGovernanceAttemptV1`
- Schema hash: `542173ac9f504109093eb8f4eeec5a4a`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `proposal` | `ProposalKind` |
| `attempt_sequence` | `u32` |

## `iroha.governance.parliament.transition.submit.v1`

> Schema summary: struct fields: governance_attempt_id: GovernanceAttemptId, transition: ParliamentLifecycleTransitionV1.

- Rust type: `iroha_data_model::isi::governance::parliament::SubmitParliamentLifecycleTransitionV1`
- Schema hash: `b2cf1e14d18c7639a3181615121997ae`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `governance_attempt_id` | `GovernanceAttemptId` |
| `transition` | `ParliamentLifecycleTransitionV1` |

## `iroha.grant`

> Schema summary: enum variants: Permission (Grant<Permission, Account>), Role (Grant<RoleId, Account>), RolePermission (Grant<Permission, Role>).

- Rust type: `iroha_data_model::isi::GrantBox`
- Schema hash: `0ff2ef6b29cba22cc60985135bec47de`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Permission` | 0 | `Grant<Permission, Account>` |
| `Role` | 1 | `Grant<RoleId, Account>` |
| `RolePermission` | 2 | `Grant<Permission, Role>` |

## `iroha.instruction.v1::governance::CastPlainBallot`

> Schema summary: struct fields: referendum_id: String, owner: AccountId, amount: Quantity, duration_blocks: u64, direction: u8.

- Rust type: `iroha_data_model::isi::governance::CastPlainBallot`
- Schema hash: `62b23313103064bc2c9d528ac3548949`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `referendum_id` | `String` |
| `owner` | `AccountId` |
| `amount` | `Quantity` |
| `duration_blocks` | `u64` |
| `direction` | `u8` |

## `iroha.instruction.v1::governance::CastZkBallot`

> Schema summary: struct fields: election_id: String, proof_b64: String, public_inputs_json: String.

- Rust type: `iroha_data_model::isi::governance::CastZkBallot`
- Schema hash: `abae0adf4d6ffedaa63d36522d4684c2`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `election_id` | `String` |
| `proof_b64` | `String` |
| `public_inputs_json` | `String` |

## `iroha.instruction.v1::governance::ProposeContractEmergencyHold`

> Schema summary: struct fields: proposal: ContractEmergencyHoldProposalV1.

- Rust type: `iroha_data_model::isi::governance::ProposeContractEmergencyHold`
- Schema hash: `259ae629d173211661e5b87f081de2a5`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `proposal` | `ContractEmergencyHoldProposalV1` |

## `iroha.instruction.v1::governance::ProposeContractLifecycleGovernance`

> Schema summary: struct fields: proposal: ContractLifecycleGovernanceProposalV1.

- Rust type: `iroha_data_model::isi::governance::ProposeContractLifecycleGovernance`
- Schema hash: `a38a4f48e3edaab60127fbe20af80ed2`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `proposal` | `ContractLifecycleGovernanceProposalV1` |

## `iroha.instruction.v1::governance::ProposeDeployContract`

> Schema summary: struct fields: contract_address: ContractAddress, code_hash: ContractCodeHash, abi_hash: ContractAbiHash, abi_version: AbiVersion, manifest_provenance: Option<ManifestProvenance>.

- Rust type: `iroha_data_model::isi::governance::ProposeDeployContract`
- Schema hash: `926530a822dece971cc0fb5ab36850c0`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `code_hash` | `ContractCodeHash` |
| `abi_hash` | `ContractAbiHash` |
| `abi_version` | `AbiVersion` |
| `manifest_provenance` | `Option<ManifestProvenance>` |

## `iroha.instruction.v1::governance::ProposeGlobalDataTriggerPermissionGovernance`

> Schema summary: struct fields: proposal: GlobalDataTriggerPermissionGovernanceProposalV1.

- Rust type: `iroha_data_model::isi::governance::ProposeGlobalDataTriggerPermissionGovernance`
- Schema hash: `27a3334fa47a6d6e5f37800d6da3a60d`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `proposal` | `GlobalDataTriggerPermissionGovernanceProposalV1` |

## `iroha.instruction.v1::governance::UpdatePlainConviction`

> Schema summary: struct fields: referendum_id: String, owner: AccountId, amount: Quantity, duration_blocks: u64.

- Rust type: `iroha_data_model::isi::governance::UpdatePlainConviction`
- Schema hash: `3d7a0f89e406395e812db917876a1a78`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `referendum_id` | `String` |
| `owner` | `AccountId` |
| `amount` | `Quantity` |
| `duration_blocks` | `u64` |

## `iroha.instruction.v1::kaigi::CreateKaigi`

> Schema summary: struct fields: call: NewKaigi, commitment: Option<KaigiParticipantCommitment>, nullifier: Option<KaigiParticipantNullifier>, roster_root: Option<Hash>, proof: Option<Vec<u8>>.

- Rust type: `iroha_data_model::isi::kaigi::CreateKaigi`
- Schema hash: `8c6eea2a5201bee243ea19cb08e50a08`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call` | `NewKaigi` |
| `commitment` | `Option<KaigiParticipantCommitment>` |
| `nullifier` | `Option<KaigiParticipantNullifier>` |
| `roster_root` | `Option<Hash>` |
| `proof` | `Option<Vec<u8>>` |

## `iroha.instruction.v1::kaigi::EndKaigi`

> Schema summary: struct fields: call_id: KaigiId, ended_at_ms: Option<u64>, commitment: Option<KaigiParticipantCommitment>, nullifier: Option<KaigiParticipantNullifier>, roster_root: Option<Hash>, proof: Option<Vec<u8>>.

- Rust type: `iroha_data_model::isi::kaigi::EndKaigi`
- Schema hash: `c32489d53f4f0e463df6504dddce9b7b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `ended_at_ms` | `Option<u64>` |
| `commitment` | `Option<KaigiParticipantCommitment>` |
| `nullifier` | `Option<KaigiParticipantNullifier>` |
| `roster_root` | `Option<Hash>` |
| `proof` | `Option<Vec<u8>>` |

## `iroha.instruction.v1::kaigi::JoinKaigi`

> Schema summary: struct fields: call_id: KaigiId, participant: AccountId, commitment: Option<KaigiParticipantCommitment>, nullifier: Option<KaigiParticipantNullifier>, roster_root: Option<Hash>, proof: Option<Vec<u8>>.

- Rust type: `iroha_data_model::isi::kaigi::JoinKaigi`
- Schema hash: `783156d69daed85cb5bc75b90f8a5657`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `participant` | `AccountId` |
| `commitment` | `Option<KaigiParticipantCommitment>` |
| `nullifier` | `Option<KaigiParticipantNullifier>` |
| `roster_root` | `Option<Hash>` |
| `proof` | `Option<Vec<u8>>` |

## `iroha.instruction.v1::kaigi::LeaveKaigi`

> Schema summary: struct fields: call_id: KaigiId, participant: AccountId, commitment: Option<KaigiParticipantCommitment>, nullifier: Option<KaigiParticipantNullifier>, roster_root: Option<Hash>, proof: Option<Vec<u8>>.

- Rust type: `iroha_data_model::isi::kaigi::LeaveKaigi`
- Schema hash: `be5cc959979a332405d134b7993b5fde`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `participant` | `AccountId` |
| `commitment` | `Option<KaigiParticipantCommitment>` |
| `nullifier` | `Option<KaigiParticipantNullifier>` |
| `roster_root` | `Option<Hash>` |
| `proof` | `Option<Vec<u8>>` |

## `iroha.instruction.v1::kaigi::RecordKaigiUsage`

> Schema summary: struct fields: call_id: KaigiId, duration_ms: u64, billed_gas: u64, usage_commitment: Option<KaigiAuthorizationScalarV1>, proof: Option<Vec<u8>>.

- Rust type: `iroha_data_model::isi::kaigi::RecordKaigiUsage`
- Schema hash: `af1e75920a73e5cbca28e61607f591ff`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `duration_ms` | `u64` |
| `billed_gas` | `u64` |
| `usage_commitment` | `Option<KaigiAuthorizationScalarV1>` |
| `proof` | `Option<Vec<u8>>` |

## `iroha.instruction.v1::kaigi::RegisterKaigiRelay`

> Schema summary: struct fields: relay: KaigiRelayRegistration.

- Rust type: `iroha_data_model::isi::kaigi::RegisterKaigiRelay`
- Schema hash: `b2a46ddb766ca24558c44bd0e7d07660`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `relay` | `KaigiRelayRegistration` |

## `iroha.instruction.v1::kaigi::ReportKaigiRelayHealth`

> Schema summary: struct fields: call_id: KaigiId, relay_id: AccountId, status: KaigiRelayHealthStatus, reported_at_ms: u64, notes: Option<String>.

- Rust type: `iroha_data_model::isi::kaigi::ReportKaigiRelayHealth`
- Schema hash: `5e2e838e9710b3daa517d38e4f16c9bb`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `relay_id` | `AccountId` |
| `status` | `KaigiRelayHealthStatus` |
| `reported_at_ms` | `u64` |
| `notes` | `Option<String>` |

## `iroha.instruction.v1::kaigi::SetKaigiRelayManifest`

> Schema summary: struct fields: call_id: KaigiId, relay_manifest: Option<KaigiRelayManifest>.

- Rust type: `iroha_data_model::isi::kaigi::SetKaigiRelayManifest`
- Schema hash: `18892cb3a3e8da2e425239969e583cd6`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `relay_manifest` | `Option<KaigiRelayManifest>` |

## `iroha.instruction.v1::kaigi::UnregisterKaigiRelay`

> Schema summary: struct fields: relay_id: AccountId.

- Rust type: `iroha_data_model::isi::kaigi::UnregisterKaigiRelay`
- Schema hash: `1fd543fd163a735c7d95807a6c1e2426`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `relay_id` | `AccountId` |

## `iroha.instruction.v1::ministry::SubmitAgendaProposal`

> Schema summary: struct fields: proposal: AgendaProposalV1.

- Rust type: `iroha_data_model::isi::ministry::SubmitAgendaProposal`
- Schema hash: `fea837e878a1a962db096737ec821aaf`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `proposal` | `AgendaProposalV1` |

## `iroha.instruction.v1::smart_contract_code::AcceptContractOwnership`

> Schema summary: struct fields: contract_address: ContractAddress, expected_revision: u64.

- Rust type: `iroha_data_model::isi::smart_contract_code::AcceptContractOwnership`
- Schema hash: `1d8e4481e42cd7886f1ee094a535739f`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `expected_revision` | `u64` |

**Smart-contract notes:**

- Only the pending account owner may accept; successful acceptance clears the offer and Parliament delegation and advances the lifecycle revision.

## `iroha.instruction.v1::smart_contract_code::ActivateContractInstance`

> Schema summary: struct fields: contract_address: ContractAddress, expected_revision: u64, code_hash: Hash.

- Rust type: `iroha_data_model::isi::smart_contract_code::ActivateContractInstance`
- Schema hash: `8ec0cf8ad0470dd7d021321f1cec8d47`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `expected_revision` | `u64` |
| `code_hash` | `Hash` |

**Smart-contract notes:**

- Requires a retained lifecycle record, its exact non-zero `expected_revision`, and registered manifest plus bytecode for `code_hash`; raw activation cannot create an address.
- Only the current account owner may submit this raw instruction; Parliament activation uses the certified governance lifecycle corridor.

## `iroha.instruction.v1::smart_contract_code::CancelContractOwnershipOffer`

> Schema summary: struct fields: contract_address: ContractAddress, expected_revision: u64.

- Rust type: `iroha_data_model::isi::smart_contract_code::CancelContractOwnershipOffer`
- Schema hash: `dcc862ede0f7394acff508355f730ff6`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `expected_revision` | `u64` |

**Smart-contract notes:**

- Only the current account owner may cancel an outstanding offer, guarded by the exact lifecycle `expected_revision`.

## `iroha.instruction.v1::smart_contract_code::CancelSmartContractCodeUpload`

> Schema summary: struct fields: code_hash: Hash.

- Rust type: `iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload`
- Schema hash: `ea496a080ec700168bae4fae3e679d2b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `code_hash` | `Hash` |

## `iroha.instruction.v1::smart_contract_code::CommitContractDeployment`

> Schema summary: struct fields: expected_deploy_nonce: u64, contract_address: ContractAddress, code_hash: Hash, contract_alias: ContractAlias, lease_expiry_ms: Option<u64>, expected_previous_contract_address: Option<ContractAddress>.

- Rust type: `iroha_data_model::isi::smart_contract_code::CommitContractDeployment`
- Schema hash: `2efc0e2e7080262cc3b17ad5866d6865`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `expected_deploy_nonce` | `u64` |
| `contract_address` | `ContractAddress` |
| `code_hash` | `Hash` |
| `contract_alias` | `ContractAlias` |
| `lease_expiry_ms` | `Option<u64>` |
| `expected_previous_contract_address` | `Option<ContractAddress>` |

**Smart-contract notes:**

- Atomically creates a fresh account-owned lifecycle and rotates the stable alias under compare-and-swap deployment guards.
- Raw deployment is rejected for protected namespaces; those addresses are created only by the certified Parliament deployment corridor.

## `iroha.instruction.v1::smart_contract_code::DeactivateContractInstance`

> Schema summary: struct fields: contract_address: ContractAddress, expected_revision: u64, reason: Option<String>.

- Rust type: `iroha_data_model::isi::smart_contract_code::DeactivateContractInstance`
- Schema hash: `6667e876e3d9c279d0d2fe4fbdba34bf`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `expected_revision` | `u64` |
| `reason` | `Option<String>` |

**Smart-contract notes:**

- Requires an active retained lifecycle record and its exact non-zero `expected_revision`; successful deactivation advances the revision but preserves origin and ownership.
- Only the current account owner may submit this raw instruction; Parliament deactivation uses the certified governance lifecycle corridor.

## `iroha.instruction.v1::smart_contract_code::FinalizeSmartContractCodeUpload`

> Schema summary: struct fields: code_hash: Hash, total_size: u64, chunk_count: u32.

- Rust type: `iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload`
- Schema hash: `0406dbcf58c0c157bdc2c690d3faba54`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `code_hash` | `Hash` |
| `total_size` | `u64` |
| `chunk_count` | `u32` |

## `iroha.instruction.v1::smart_contract_code::OfferContractOwnership`

> Schema summary: struct fields: contract_address: ContractAddress, expected_revision: u64, new_owner: ContractLifecycleOwnerV1.

- Rust type: `iroha_data_model::isi::smart_contract_code::OfferContractOwnership`
- Schema hash: `39bed4c5f25cefd4e01cc123e376667d`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `expected_revision` | `u64` |
| `new_owner` | `ContractLifecycleOwnerV1` |

**Smart-contract notes:**

- The current account owner records a revision-guarded pending owner; ownership does not move until a separate acceptance.

## `iroha.instruction.v1::smart_contract_code::RegisterSmartContractBytes`

> Schema summary: struct fields: code_hash: Hash, code: Vec<u8>.

- Rust type: `iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes`
- Schema hash: `a78be8fe926a797ea6c73e651427118c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `code_hash` | `Hash` |
| `code` | `Vec<u8>` |

**Smart-contract notes:**

- `code_hash` must equal the domain-separated canonical hash of the complete deployable `.to` artifact; duplicate uploads re-use the stored bytes.
- Use the hashes in `specs/sdk/android/generated/fixtures/smart_contract_code_executor_hashes.json` to verify `.to` parsing logic in automation.

## `iroha.instruction.v1::smart_contract_code::RegisterSmartContractCode`

> Schema summary: struct fields: manifest: ContractManifest.

- Rust type: `iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode`
- Schema hash: `fa62c9f0a5a3f8b756eef62b689e2a32`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `manifest` | `ContractManifest` |

### Manifest field details

#### ContractManifest fields

Optional metadata attached to smart-contract deployments; hash fields must match the canonical host-computed values before admission.

| Field | Type | Description |
|-------|------|-------------|
| `code_hash` | `Option<Hash>` | Domain-separated canonical hash of the complete deployable `.to` artifact, including its execution header, `CNTR`, literals, and code. |
| `abi_hash` | `Option<Hash>` | Hash of the syscall/pointer ABI surface for the supplied `abi_version` (see `specs/ivm_header.md`). |
| `compiler_fingerprint` | `Option<String>` | Compiler + toolchain note recorded for provenance. |
| `features_bitmap` | `Option<u64>` | Compiler-derived, hash-covered V1 execution capabilities (ZK and VECTOR); never host SIMD, Metal, or CUDA availability. |
| `access_set_hints` | `Option<AccessSetHints>` | Advisory read/write key hints for the scheduler. |
| `entrypoints` | `Option<Vec<EntrypointDescriptor>>` | Optional entrypoint descriptors advertised by the compiler. |

#### AccessSetHints fields

Declarative read/write key hints stored inside smart-contract manifests.

| Field | Type | Description |
|-------|------|-------------|
| `read_keys` | `Vec<String>` | Canonical keys (e.g., `account:sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D`) the contract expects to read. |
| `write_keys` | `Vec<String>` | Keys that the contract expects to write during execution. |

#### EntrypointDescriptor fields

Metadata emitted per Kotodama entrypoint.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Symbol name declared in Kotodama source. |
| `kind` | `EntryPointKind` | Role of the entrypoint (`Kotoage`, `View`, `Hajimari`, or `Kaizen`). |
| `permission` | `Option<String>` | Optional dispatcher permission required before invocation. |
| `read_keys` | `Vec<String>` | Advisory read set scoped to the entrypoint. |
| `write_keys` | `Vec<String>` | Advisory write set scoped to the entrypoint. |
| `access_hints_complete` | `Option<bool>` | Whether access-set hints are complete or explicitly provided. |
| `access_hints_skipped` | `Vec<String>` | Reasons access hints were skipped for this entrypoint. |
| `triggers` | `Vec<TriggerDescriptor>` | Trigger declarations that call this entrypoint. |

#### TriggerDescriptor fields

Declarative trigger metadata attached to an entrypoint.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `TriggerId` | Trigger identifier. |
| `repeats` | `Repeats` | Repeat policy for the trigger action. |
| `filter` | `EventFilterBox` | Event filter that drives execution. |
| `authority` | `Option<AccountId>` | Optional explicit authority override. |
| `metadata` | `Metadata` | Trigger metadata payload (JSON map). |
| `callback` | `TriggerCallback` | Callback target for this trigger. |

#### TriggerCallback fields

Entrypoint callback target referenced by a trigger declaration.

| Field | Type | Description |
|-------|------|-------------|
| `namespace` | `Option<String>` | Optional contract namespace for cross-contract callbacks. |
| `entrypoint` | `String` | Entrypoint name to invoke. |

**Smart-contract notes:**

- Nodes recompute `manifest.code_hash` from the `.to` artifact and reject mismatches; `manifest.abi_hash` must equal the canonical ABI digest for the declared version.
- Sample hash pair derived from `defaults/executor.to` lives in `specs/sdk/android/generated/fixtures/smart_contract_code_executor_hashes.json` for deterministic builder tests.

## `iroha.instruction.v1::smart_contract_code::RemoveSmartContractBytes`

> Schema summary: struct fields: code_hash: Hash, reason: Option<String>.

- Rust type: `iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes`
- Schema hash: `86da3d62bcefa84711d95e3fea332689`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `code_hash` | `Hash` |
| `reason` | `Option<String>` |

**Smart-contract notes:**

- Removal succeeds only when no manifest or active instance references the target `code_hash`; provide an audit reason when automating removals.

## `iroha.instruction.v1::smart_contract_code::SetContractParliamentDelegation`

> Schema summary: struct fields: contract_address: ContractAddress, expected_revision: u64, delegated: bool.

- Rust type: `iroha_data_model::isi::smart_contract_code::SetContractParliamentDelegation`
- Schema hash: `752c5605dec44d771d4f63c41e57b2ed`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `expected_revision` | `u64` |
| `delegated` | `bool` |

**Smart-contract notes:**

- Only the current account owner may change delegation, guarded by the exact lifecycle `expected_revision`.
- Delegated Parliament may activate or deactivate through certified governance, but cannot transfer ownership or change delegation.

## `iroha.instruction.v1::smart_contract_code::UploadSmartContractCodeChunk`

> Schema summary: struct fields: code_hash: Hash, total_size: u64, chunk_index: u32, chunk_count: u32, chunk: Vec<u8>.

- Rust type: `iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk`
- Schema hash: `41ca98d8d78d9d8113909941490f8612`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `code_hash` | `Hash` |
| `total_size` | `u64` |
| `chunk_index` | `u32` |
| `chunk_count` | `u32` |
| `chunk` | `Vec<u8>` |

## `iroha.instruction.v1::sorafs::ApprovePinManifest`

> Schema summary: struct fields: digest: ManifestDigest, council_envelope: Option<Vec<u8>>, council_envelope_digest: Option<Array<u8, 32>>.

- Rust type: `iroha_data_model::isi::sorafs::ApprovePinManifest`
- Schema hash: `1583c5673581a22cad86e51ca49aa514`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `digest` | `ManifestDigest` |
| `council_envelope` | `Option<Vec<u8>>` |
| `council_envelope_digest` | `Option<Array<u8, 32>>` |

## `iroha.instruction.v1::sorafs::BindManifestAlias`

> Schema summary: struct fields: digest: ManifestDigest, binding: ManifestAliasBinding, bound_epoch: u64, expiry_epoch: u64.

- Rust type: `iroha_data_model::isi::sorafs::BindManifestAlias`
- Schema hash: `6baf2f7a3e4df7bf5dab7231c7690fe0`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `digest` | `ManifestDigest` |
| `binding` | `ManifestAliasBinding` |
| `bound_epoch` | `u64` |
| `expiry_epoch` | `u64` |

### Manifest field details

#### ManifestAliasBinding fields

Alias binding payload approved alongside a manifest.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Canonical ASCII alias label matching `[a-z0-9._-]{1,128}`. |
| `namespace` | `String` | Canonical ASCII alias namespace matching `[a-z0-9._-]{1,128}`. |
| `proof` | `Vec<u8>` | Non-empty canonical Norito alias proof bytes (canonical padded base64 in JSON; decoded size at most 1 MiB). |

## `iroha.instruction.v1::sorafs::CompleteReplicationOrder`

> Schema summary: struct fields: order_id: ReplicationOrderId, provider_id: ProviderId, completion_epoch: u64, expected_authority: ProviderIngestCompletionAuthorityV1, expected_assignment_revision: u64, finalized_anchor: ProviderIngestFinalizedAnchorV1.

- Rust type: `iroha_data_model::isi::sorafs::CompleteReplicationOrder`
- Schema hash: `b12e141f6fa82d6538e77613bc8f848c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `order_id` | `ReplicationOrderId` |
| `provider_id` | `ProviderId` |
| `completion_epoch` | `u64` |
| `expected_authority` | `ProviderIngestCompletionAuthorityV1` |
| `expected_assignment_revision` | `u64` |
| `finalized_anchor` | `ProviderIngestFinalizedAnchorV1` |

## `iroha.instruction.v1::sorafs::ExpireReplicationOrder`

> Schema summary: struct fields: order_id: ReplicationOrderId, expiration_epoch: u64.

- Rust type: `iroha_data_model::isi::sorafs::ExpireReplicationOrder`
- Schema hash: `c3c08d50e73972a3f5136d7031c1a309`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `order_id` | `ReplicationOrderId` |
| `expiration_epoch` | `u64` |

## `iroha.instruction.v1::sorafs::IssueReplicationOrder`

> Schema summary: struct fields: order_id: ReplicationOrderId, order_payload: Vec<u8>, issued_epoch: u64, deadline_epoch: u64, musubi_archive: Option<ArchiveId>.

- Rust type: `iroha_data_model::isi::sorafs::IssueReplicationOrder`
- Schema hash: `c4b340f0b6d646e6865d4a23087f5e2c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `order_id` | `ReplicationOrderId` |
| `order_payload` | `Vec<u8>` |
| `issued_epoch` | `u64` |
| `deadline_epoch` | `u64` |
| `musubi_archive` | `Option<ArchiveId>` |

## `iroha.instruction.v1::sorafs::RecordCapacityTelemetry`

> Schema summary: struct fields: record: CapacityTelemetryRecord.

- Rust type: `iroha_data_model::isi::sorafs::RecordCapacityTelemetry`
- Schema hash: `7378859e6a3f4607c1246f65cc6c896b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `record` | `CapacityTelemetryRecord` |

## `iroha.instruction.v1::sorafs::RegisterCapacityDeclaration`

> Schema summary: struct fields: record: CapacityDeclarationRecord.

- Rust type: `iroha_data_model::isi::sorafs::RegisterCapacityDeclaration`
- Schema hash: `9c77b1011c33673c919ac9d0a4ba0808`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `record` | `CapacityDeclarationRecord` |

## `iroha.instruction.v1::sorafs::RegisterCapacityDispute`

> Schema summary: struct fields: record: CapacityDisputeRecord.

- Rust type: `iroha_data_model::isi::sorafs::RegisterCapacityDispute`
- Schema hash: `7940e0ccdc6836d8e62b8b0cd27117f7`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `record` | `CapacityDisputeRecord` |

## `iroha.instruction.v1::sorafs::RegisterPinManifest`

> Schema summary: struct fields: manifest_payload: Vec<u8>, alias: Option<ManifestAliasBinding>, successor_of: Option<ManifestDigest>.

- Rust type: `iroha_data_model::isi::sorafs::RegisterPinManifest`
- Schema hash: `61eb8eda15dad63ec8e3b35b58cfaa36`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `manifest_payload` | `Vec<u8>` |
| `alias` | `Option<ManifestAliasBinding>` |
| `successor_of` | `Option<ManifestDigest>` |

### Manifest field details

#### ManifestAliasBinding fields

Alias binding payload approved alongside a manifest.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Canonical ASCII alias label matching `[a-z0-9._-]{1,128}`. |
| `namespace` | `String` | Canonical ASCII alias namespace matching `[a-z0-9._-]{1,128}`. |
| `proof` | `Vec<u8>` | Non-empty canonical Norito alias proof bytes (canonical padded base64 in JSON; decoded size at most 1 MiB). |

## `iroha.instruction.v1::sorafs::RegisterProviderOwner`

> Schema summary: struct fields: provider_id: ProviderId, owner: AccountId.

- Rust type: `iroha_data_model::isi::sorafs::RegisterProviderOwner`
- Schema hash: `6226ce0ecda49b712bb0bb166d2ae864`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `provider_id` | `ProviderId` |
| `owner` | `AccountId` |

## `iroha.instruction.v1::sorafs::RetirePinManifest`

> Schema summary: struct fields: digest: ManifestDigest, reason: Option<String>.

- Rust type: `iroha_data_model::isi::sorafs::RetirePinManifest`
- Schema hash: `6da0ff52a6d999ecbd39aef38443f1a3`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `digest` | `ManifestDigest` |
| `reason` | `Option<String>` |

## `iroha.instruction.v1::sorafs::ReviseReplicationOrderAssignments`

> Schema summary: struct fields: order_id: ReplicationOrderId, expected_assignment_revision: u64, next_assignment_revision: u64, assignments: Vec<ReplicationAssignmentV1>.

- Rust type: `iroha_data_model::isi::sorafs::ReviseReplicationOrderAssignments`
- Schema hash: `6e875bf7dae8680dd05c61ab4f98c6fc`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `order_id` | `ReplicationOrderId` |
| `expected_assignment_revision` | `u64` |
| `next_assignment_revision` | `u64` |
| `assignments` | `Vec<ReplicationAssignmentV1>` |

## `iroha.instruction.v1::sorafs::RevokeProviderIngestCompletionAuthority`

> Schema summary: struct fields: provider_id: ProviderId, expected_current: ProviderIngestCompletionAuthorityV1.

- Rust type: `iroha_data_model::isi::sorafs::RevokeProviderIngestCompletionAuthority`
- Schema hash: `e61fa4476187f68a02d9a6d987eb36a1`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `provider_id` | `ProviderId` |
| `expected_current` | `ProviderIngestCompletionAuthorityV1` |

## `iroha.instruction.v1::sorafs::SetProviderIngestCompletionAuthority`

> Schema summary: struct fields: provider_id: ProviderId, expected_current: Option<ProviderIngestCompletionAuthorityV1>, next: ProviderIngestCompletionAuthorityV1.

- Rust type: `iroha_data_model::isi::sorafs::SetProviderIngestCompletionAuthority`
- Schema hash: `21eee4c57c81e23ba5aef896cedc1c86`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `provider_id` | `ProviderId` |
| `expected_current` | `Option<ProviderIngestCompletionAuthorityV1>` |
| `next` | `ProviderIngestCompletionAuthorityV1` |

## `iroha.instruction.v1::sorafs::UnregisterProviderOwner`

> Schema summary: struct fields: provider_id: ProviderId.

- Rust type: `iroha_data_model::isi::sorafs::UnregisterProviderOwner`
- Schema hash: `eda3efbef86870e64d3cd07ea2d60ab9`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `provider_id` | `ProviderId` |

## `iroha.instruction.v1::transparent::RemoveAssetKeyValue`

> Schema summary: struct fields: asset: AssetId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveAssetKeyValue`
- Schema hash: `8f0008f715ed9794ded9ae3a990243bf`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetId` |
| `key` | `Name` |

## `iroha.instruction.v1::transparent::SetAssetKeyValue`

> Schema summary: struct fields: asset: AssetId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetAssetKeyValue`
- Schema hash: `5955e7b3e0166d3234997cb5738f5973`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha.instruction.v1::verifying_keys::RegisterVerifyingKey`

> Schema summary: struct fields: id: VerifyingKeyId, record: VerifyingKeyRecord.

- Rust type: `iroha_data_model::isi::verifying_keys::RegisterVerifyingKey`
- Schema hash: `61c13e70ede9a90bacef2fcfb6457446`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `VerifyingKeyId` |
| `record` | `VerifyingKeyRecord` |

## `iroha.instruction.v1::verifying_keys::UpdateVerifyingKey`

> Schema summary: struct fields: id: VerifyingKeyId, record: VerifyingKeyRecord.

- Rust type: `iroha_data_model::isi::verifying_keys::UpdateVerifyingKey`
- Schema hash: `8b6f2a4b41a57ca1e852170e0e984e85`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `VerifyingKeyId` |
| `record` | `VerifyingKeyRecord` |

## `iroha.instruction.v1::zk::CreateElection`

> Schema summary: struct fields: election_id: String, options: u32, eligible_root: Array<u8, 32>, start_ts: u64, end_ts: u64, vk_ballot: VerifyingKeyId, vk_tally: VerifyingKeyId, domain_tag: String.

- Rust type: `iroha_data_model::isi::zk::CreateElection`
- Schema hash: `443beea278f19d1d954f0c02b5f1d8cc`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `election_id` | `String` |
| `options` | `u32` |
| `eligible_root` | `Array<u8, 32>` |
| `start_ts` | `u64` |
| `end_ts` | `u64` |
| `vk_ballot` | `VerifyingKeyId` |
| `vk_tally` | `VerifyingKeyId` |
| `domain_tag` | `String` |

## `iroha.instruction.v1::zk::FinalizeElection`

> Schema summary: struct fields: election_id: String, tally: Vec<u128>, tally_proof: ProofAttachment.

- Rust type: `iroha_data_model::isi::zk::FinalizeElection`
- Schema hash: `7382acea8661a36bc48f571339b0a6c4`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `election_id` | `String` |
| `tally` | `Vec<u128>` |
| `tally_proof` | `ProofAttachment` |

## `iroha.instruction.v1::zk::RegisterZkAsset`

> Schema summary: struct fields: asset: AssetDefinitionId, vk_unshield: Option<VerifyingKeyId>.

- Rust type: `iroha_data_model::isi::zk::RegisterZkAsset`
- Schema hash: `5fc0b16cf5cb3dd02292dc01fdcb8179`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `vk_unshield` | `Option<VerifyingKeyId>` |

## `iroha.instruction.v1::zk::SubmitBallot`

> Schema summary: struct fields: election_id: String, ciphertext: Vec<u8>, ballot_proof: ProofAttachment, nullifier: Array<u8, 32>.

- Rust type: `iroha_data_model::isi::zk::SubmitBallot`
- Schema hash: `0b16f818b658db7b9f4383a76eeb5545`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `election_id` | `String` |
| `ciphertext` | `Vec<u8>` |
| `ballot_proof` | `ProofAttachment` |
| `nullifier` | `Array<u8, 32>` |

## `iroha.instruction.v1::zk::VerifyProof`

> Schema summary: struct fields: attachment: ProofAttachment.

- Rust type: `iroha_data_model::isi::zk::VerifyProof`
- Schema hash: `0b5d0ae55f342299f394799e85f61ae8`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `attachment` | `ProofAttachment` |

## `iroha.log`

> Schema summary: struct fields: level: Level, msg: String.

- Rust type: `iroha_data_model::isi::transparent::Log`
- Schema hash: `8e55c03b421e22131dbca44b0bdeb957`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `level` | `Level` |
| `msg` | `String` |

## `iroha.mint`

> Schema summary: enum variants: Asset (Mint<Quantity, Asset>), TriggerRepetitions (Mint<u32, Trigger>).

- Rust type: `iroha_data_model::isi::mint_burn::MintBox`
- Schema hash: `ec0b538ed0e5b46ed163e0aedb335e73`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Asset` | 0 | `Mint<Quantity, Asset>` |
| `TriggerRepetitions` | 1 | `Mint<u32, Trigger>` |

## `iroha.register`

> Schema summary: enum variants: Peer (RegisterPeerWithPop), Domain (Register<Domain>), Account (Register<Account>), AssetDefinition (Register<AssetDefinition>), Nft (Register<Nft>), Role (Register<Role>), Trigger (Register<Trigger>).

- Rust type: `iroha_data_model::isi::register::RegisterBox`
- Schema hash: `2e9fa44b44ac5295a0b34e05edcb4133`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Peer` | 0 | `RegisterPeerWithPop` |
| `Domain` | 1 | `Register<Domain>` |
| `Account` | 2 | `Register<Account>` |
| `AssetDefinition` | 3 | `Register<AssetDefinition>` |
| `Nft` | 4 | `Register<Nft>` |
| `Role` | 5 | `Register<Role>` |
| `Trigger` | 6 | `Register<Trigger>` |

## `iroha.remove_key_value`

> Schema summary: enum variants: Domain (RemoveKeyValue<Domain>), Account (RemoveKeyValue<Account>), AssetDefinition (RemoveKeyValue<AssetDefinition>), Nft (RemoveKeyValue<Nft>), Trigger (RemoveKeyValue<Trigger>).

- Rust type: `iroha_data_model::isi::RemoveKeyValueBox`
- Schema hash: `c2940a83246a650a774cc48c8294f754`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Domain` | 0 | `RemoveKeyValue<Domain>` |
| `Account` | 1 | `RemoveKeyValue<Account>` |
| `AssetDefinition` | 2 | `RemoveKeyValue<AssetDefinition>` |
| `Nft` | 3 | `RemoveKeyValue<Nft>` |
| `Trigger` | 4 | `RemoveKeyValue<Trigger>` |

## `iroha.repo`

> Schema summary: enum variants: Initiate (RepoIsi), Reverse (ReverseRepoIsi), MarginCall (RepoMarginCallIsi).

- Rust type: `iroha_data_model::isi::repo::RepoInstructionBox`
- Schema hash: `f98148ca4133dadc0b9046058646c979`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Initiate` | 0 | `RepoIsi` |
| `Reverse` | 1 | `ReverseRepoIsi` |
| `MarginCall` | 2 | `RepoMarginCallIsi` |

## `iroha.revoke`

> Schema summary: enum variants: Permission (Revoke<Permission, Account>), Role (Revoke<RoleId, Account>), RolePermission (Revoke<Permission, Role>).

- Rust type: `iroha_data_model::isi::RevokeBox`
- Schema hash: `3bca4b895d20bf1081e15d823ad0cff9`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Permission` | 0 | `Revoke<Permission, Account>` |
| `Role` | 1 | `Revoke<RoleId, Account>` |
| `RolePermission` | 2 | `Revoke<Permission, Role>` |

## `iroha.runtime_upgrade.activate`

> Schema summary: struct fields: id: RuntimeUpgradeId.

- Rust type: `iroha_data_model::isi::runtime_upgrade::ActivateRuntimeUpgrade`
- Schema hash: `dd0f2fac36ae80eba91c5a521bd012db`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `RuntimeUpgradeId` |

## `iroha.runtime_upgrade.cancel`

> Schema summary: struct fields: id: RuntimeUpgradeId.

- Rust type: `iroha_data_model::isi::runtime_upgrade::CancelRuntimeUpgrade`
- Schema hash: `d563aa37d8f4b53d9de7e5330ff76f94`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `RuntimeUpgradeId` |

## `iroha.runtime_upgrade.propose`

> Schema summary: struct fields: manifest_bytes: Vec<u8>.

- Rust type: `iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade`
- Schema hash: `d3f95f8f392d31da0c3b1528c13b2d08`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `manifest_bytes` | `Vec<u8>` |

## `iroha.set_key_value`

> Schema summary: enum variants: Domain (SetKeyValue<Domain>), Account (SetKeyValue<Account>), AssetDefinition (SetKeyValue<AssetDefinition>), Nft (SetKeyValue<Nft>), Trigger (SetKeyValue<Trigger>).

- Rust type: `iroha_data_model::isi::SetKeyValueBox`
- Schema hash: `7f532bc72c105d3cd63dda90e00df899`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Domain` | 0 | `SetKeyValue<Domain>` |
| `Account` | 1 | `SetKeyValue<Account>` |
| `AssetDefinition` | 2 | `SetKeyValue<AssetDefinition>` |
| `Nft` | 3 | `SetKeyValue<Nft>` |
| `Trigger` | 4 | `SetKeyValue<Trigger>` |

## `iroha.set_parameter`

> Schema summary: tuple fields: _0: Parameter.

- Rust type: `iroha_data_model::isi::transparent::SetParameter`
- Schema hash: `e0fff3487fdca11cf277d9bdd4338343`

**Layout:** `tuple`

| Field | Type |
|-------|------|
| `0` | `Parameter` |

## `iroha.settlement`

> Schema summary: enum variants: Dvp (DvpIsi), Pvp (PvpIsi), SetFxCorridorPolicy (SetFxCorridorPolicy), FundFxCorridorEscrow (FundFxCorridorEscrow), RefundFxCorridorEscrow (RefundFxCorridorEscrow), SettleFxCorridor (SettleFxCorridor), Atomic (SettleAtomic).

- Rust type: `iroha_data_model::isi::settlement::SettlementInstructionBox`
- Schema hash: `a1f5f5f5e7b87acd6bcc319e8635a3a3`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Dvp` | 0 | `DvpIsi` |
| `Pvp` | 1 | `PvpIsi` |
| `SetFxCorridorPolicy` | 2 | `SetFxCorridorPolicy` |
| `FundFxCorridorEscrow` | 3 | `FundFxCorridorEscrow` |
| `RefundFxCorridorEscrow` | 4 | `RefundFxCorridorEscrow` |
| `SettleFxCorridor` | 5 | `SettleFxCorridor` |
| `Atomic` | 6 | `SettleAtomic` |

## `iroha.transfer`

> Schema summary: enum variants: Domain (Transfer<Account, DomainId, Account>), AssetDefinition (Transfer<Account, AssetDefinitionId, Account>), Asset (Transfer<Asset, Quantity, Account>), Nft (Transfer<Account, NftId, Account>).

- Rust type: `iroha_data_model::isi::transfer::TransferBox`
- Schema hash: `a4174c78d6341f8f98fc2adae8ed67b9`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Domain` | 0 | `Transfer<Account, DomainId, Account>` |
| `AssetDefinition` | 1 | `Transfer<Account, AssetDefinitionId, Account>` |
| `Asset` | 2 | `Transfer<Asset, Quantity, Account>` |
| `Nft` | 3 | `Transfer<Account, NftId, Account>` |

## `iroha.transfer_batch`

> Schema summary: struct fields: mode: BatchMode, entries: Vec<TransferAssetBatchEntry>.

- Rust type: `iroha_data_model::isi::transfer::TransferAssetBatch`
- Schema hash: `d76a8b607909812061b62dff5922a7cc`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `mode` | `BatchMode` |
| `entries` | `Vec<TransferAssetBatchEntry>` |

## `iroha.unregister`

> Schema summary: enum variants: Peer (Unregister<Peer>), Domain (Unregister<Domain>), Account (Unregister<Account>), AssetDefinition (Unregister<AssetDefinition>), Nft (Unregister<Nft>), Role (Unregister<Role>), Trigger (Unregister<Trigger>).

- Rust type: `iroha_data_model::isi::register::UnregisterBox`
- Schema hash: `42c6839dfa39c0ac8218a781820d6eae`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Peer` | 0 | `Unregister<Peer>` |
| `Domain` | 1 | `Unregister<Domain>` |
| `Account` | 2 | `Unregister<Account>` |
| `AssetDefinition` | 3 | `Unregister<AssetDefinition>` |
| `Nft` | 4 | `Unregister<Nft>` |
| `Role` | 5 | `Unregister<Role>` |
| `Trigger` | 6 | `Unregister<Trigger>` |

## `iroha.upgrade`

> Schema summary: struct fields: executor: Executor.

- Rust type: `iroha_data_model::isi::transparent::Upgrade`
- Schema hash: `78c95dde0cb1ef2399178b15fbaed21f`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `executor` | `Executor` |

## `zk::CancelConfidentialPolicyTransition`

> Schema summary: struct fields: asset: AssetDefinitionId, transition_id: Hash.

- Rust type: `iroha_data_model::isi::zk::CancelConfidentialPolicyTransition`
- Schema hash: `e5fc69bf877b653b726e5330d56df3db`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `transition_id` | `Hash` |

## `zk::ScheduleConfidentialPolicyTransition`

> Schema summary: struct fields: asset: AssetDefinitionId, new_mode: ConfidentialPolicyMode, effective_height: u64, transition_id: Hash, conversion_window: Option<u64>.

- Rust type: `iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition`
- Schema hash: `d8441660d1f34a2d89f567969956a495`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `new_mode` | `ConfidentialPolicyMode` |
| `effective_height` | `u64` |
| `transition_id` | `Hash` |
| `conversion_window` | `Option<u64>` |
