---
lang: ba
direction: ltr
source: docs/source/sdk/android/generated/instructions.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 88e6789ab71164c460f570e82656cc7ecc2f8007e5d0996a72d81f9d3bbb773a
source_last_modified: "2026-01-22T16:26:46.585235+00:00"
translation_last_reviewed: 2026-02-07
---

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

> Schema summary: enum variants: Dvp (DvpIsi), Pvp (PvpIsi), SetFxCorridorPolicy (SetFxCorridorPolicy), SettleFxCorridor (SettleFxCorridor).

- Rust type: `iroha_data_model::isi::settlement::SettlementInstructionBox`
- Schema hash: `a1f5f5f5e7b87acd6bcc319e8635a3a3`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Dvp` | 0 | `DvpIsi` |
| `Pvp` | 1 | `PvpIsi` |
| `SetFxCorridorPolicy` | 2 | `SetFxCorridorPolicy` |
| `SettleFxCorridor` | 3 | `SettleFxCorridor` |

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

> Schema summary: struct fields: entries: Vec<TransferAssetBatchEntry>.

- Rust type: `iroha_data_model::isi::transfer::TransferAssetBatch`
- Schema hash: `d76a8b607909812061b62dff5922a7cc`

**Layout:** `struct`

| Field | Type |
|-------|------|
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

## `iroha_data_model::isi::governance::CastPlainBallot`

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

## `iroha_data_model::isi::governance::CastZkBallot`

> Schema summary: struct fields: election_id: String, proof_b64: String, public_inputs_json: String.

- Rust type: `iroha_data_model::isi::governance::CastZkBallot`
- Schema hash: `abae0adf4d6ffedaa63d36522d4684c2`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `election_id` | `String` |
| `proof_b64` | `String` |
| `public_inputs_json` | `String` |

## `iroha_data_model::isi::governance::EnactReferendum`

> Schema summary: struct fields: referendum_id: Array<u8, 32>, preimage_hash: Array<u8, 32>, at_window: AtWindow.

- Rust type: `iroha_data_model::isi::governance::EnactReferendum`
- Schema hash: `ceaa89089eba40dcb61010a1e395a259`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `referendum_id` | `Array<u8, 32>` |
| `preimage_hash` | `Array<u8, 32>` |
| `at_window` | `AtWindow` |

## `iroha_data_model::isi::governance::FinalizeReferendum`

> Schema summary: struct fields: referendum_id: String, proposal_id: Array<u8, 32>.

- Rust type: `iroha_data_model::isi::governance::FinalizeReferendum`
- Schema hash: `9e8394eeb97d215a830a600e16d4c1aa`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `referendum_id` | `String` |
| `proposal_id` | `Array<u8, 32>` |

## `iroha_data_model::isi::governance::PersistCouncilForEpoch`

> Schema summary: struct fields: epoch: u64, members: Vec<AccountId>, alternates: Vec<AccountId>, verified: u32, candidates_count: u32, derived_by: CouncilDerivationKind.

- Rust type: `iroha_data_model::isi::governance::PersistCouncilForEpoch`
- Schema hash: `e883e2ba76ced91134fc5d7faab8caa2`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `epoch` | `u64` |
| `members` | `Vec<AccountId>` |
| `alternates` | `Vec<AccountId>` |
| `verified` | `u32` |
| `candidates_count` | `u32` |
| `derived_by` | `CouncilDerivationKind` |

## `iroha_data_model::isi::governance::ProposeDeployContract`

> Schema summary: struct fields: contract_address: ContractAddress, code_hash_hex: String, abi_hash_hex: String, abi_version: String, window: Option<AtWindow>, mode: Option<VotingMode>, manifest_provenance: Option<ManifestProvenance>.

- Rust type: `iroha_data_model::isi::governance::ProposeDeployContract`
- Schema hash: `926530a822dece971cc0fb5ab36850c0`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `code_hash_hex` | `String` |
| `abi_hash_hex` | `String` |
| `abi_version` | `String` |
| `window` | `Option<AtWindow>` |
| `mode` | `Option<VotingMode>` |
| `manifest_provenance` | `Option<ManifestProvenance>` |

## `iroha_data_model::isi::kaigi::CreateKaigi`

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

## `iroha_data_model::isi::kaigi::EndKaigi`

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

## `iroha_data_model::isi::kaigi::JoinKaigi`

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

## `iroha_data_model::isi::kaigi::LeaveKaigi`

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

## `iroha_data_model::isi::kaigi::RecordKaigiUsage`

> Schema summary: struct fields: call_id: KaigiId, duration_ms: u64, billed_gas: u64, usage_commitment: Option<Hash>, proof: Option<Vec<u8>>.

- Rust type: `iroha_data_model::isi::kaigi::RecordKaigiUsage`
- Schema hash: `af1e75920a73e5cbca28e61607f591ff`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `duration_ms` | `u64` |
| `billed_gas` | `u64` |
| `usage_commitment` | `Option<Hash>` |
| `proof` | `Option<Vec<u8>>` |

## `iroha_data_model::isi::kaigi::RegisterKaigiRelay`

> Schema summary: struct fields: relay: KaigiRelayRegistration.

- Rust type: `iroha_data_model::isi::kaigi::RegisterKaigiRelay`
- Schema hash: `b2a46ddb766ca24558c44bd0e7d07660`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `relay` | `KaigiRelayRegistration` |

## `iroha_data_model::isi::kaigi::SetKaigiRelayManifest`

> Schema summary: struct fields: call_id: KaigiId, relay_manifest: Option<KaigiRelayManifest>.

- Rust type: `iroha_data_model::isi::kaigi::SetKaigiRelayManifest`
- Schema hash: `18892cb3a3e8da2e425239969e583cd6`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `relay_manifest` | `Option<KaigiRelayManifest>` |

## `iroha_data_model::isi::ministry::SubmitAgendaProposal`

> Schema summary: struct fields: proposal: AgendaProposalV1.

- Rust type: `iroha_data_model::isi::ministry::SubmitAgendaProposal`
- Schema hash: `fea837e878a1a962db096737ec821aaf`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `proposal` | `AgendaProposalV1` |

## `iroha_data_model::isi::mint_burn::Burn<iroha_primitives::numeric::Quantity, iroha_data_model::asset::value::model::Asset>`

> Schema summary: struct fields: object: Quantity, destination: AssetId.

- Rust type: `iroha_data_model::isi::mint_burn::Burn<iroha_primitives::numeric::Quantity, iroha_data_model::asset::value::model::Asset>`
- Schema hash: `def28b8ac0a7011dabf5d6aca44fd42f`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Quantity` |
| `destination` | `AssetId` |

## `iroha_data_model::isi::mint_burn::Burn<u32, iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: u32, destination: TriggerId.

- Rust type: `iroha_data_model::isi::mint_burn::Burn<u32, iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `b072a05868b3a8513fbf72578cbd6791`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `u32` |
| `destination` | `TriggerId` |

## `iroha_data_model::isi::mint_burn::Mint<iroha_primitives::numeric::Quantity, iroha_data_model::asset::value::model::Asset>`

> Schema summary: struct fields: object: Quantity, destination: AssetId.

- Rust type: `iroha_data_model::isi::mint_burn::Mint<iroha_primitives::numeric::Quantity, iroha_data_model::asset::value::model::Asset>`
- Schema hash: `e1ae8c60db986034448368a72482b9a8`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Quantity` |
| `destination` | `AssetId` |

## `iroha_data_model::isi::mint_burn::Mint<u32, iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: u32, destination: TriggerId.

- Rust type: `iroha_data_model::isi::mint_burn::Mint<u32, iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `9dc6d32a057256dc62129bf7117c825e`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `u32` |
| `destination` | `TriggerId` |

## `iroha_data_model::isi::register::Register<iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: NewAccount.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::account::model::Account>`
- Schema hash: `3f3af7606739421205efb1c7c7f30949`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewAccount` |

## `iroha_data_model::isi::register::Register<iroha_data_model::asset::definition::model::AssetDefinition>`

> Schema summary: struct fields: object: NewAssetDefinition.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::asset::definition::model::AssetDefinition>`
- Schema hash: `564a1394c20ccf1d02a9ba2147287a5f`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewAssetDefinition` |

## `iroha_data_model::isi::register::Register<iroha_data_model::domain::model::Domain>`

> Schema summary: struct fields: object: NewDomain.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::domain::model::Domain>`
- Schema hash: `22930817cc1f8d4ac41f62017085de82`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewDomain` |

## `iroha_data_model::isi::register::Register<iroha_data_model::nft::model::Nft>`

> Schema summary: struct fields: object: NewNft.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::nft::model::Nft>`
- Schema hash: `18d9bb39182910b1d20bfa59acc491d2`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewNft` |

## `iroha_data_model::isi::register::Register<iroha_data_model::role::model::Role>`

> Schema summary: struct fields: object: NewRole.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::role::model::Role>`
- Schema hash: `c6f709139c4be58789b348def3cb86ac`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewRole` |

## `iroha_data_model::isi::register::Register<iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: Trigger.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `14c9dce57703112aa22be88fca5c7647`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Trigger` |

## `iroha_data_model::isi::register::RegisterPeerWithPop`

> Schema summary: struct fields: peer: PeerId, pop: Vec<u8>, activation_at: Option<u64>, expiry_at: Option<u64>, hsm: Option<HsmBinding>.

- Rust type: `iroha_data_model::isi::register::RegisterPeerWithPop`
- Schema hash: `5bce06b486a498769cabef7046aa4b60`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `peer` | `PeerId` |
| `pop` | `Vec<u8>` |
| `activation_at` | `Option<u64>` |
| `expiry_at` | `Option<u64>` |
| `hsm` | `Option<HsmBinding>` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: AccountId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::account::model::Account>`
- Schema hash: `a7833e55477579c8f9ab1b632fb8c67c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AccountId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::asset::definition::model::AssetDefinition>`

> Schema summary: struct fields: object: AssetDefinitionId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::asset::definition::model::AssetDefinition>`
- Schema hash: `1867ecb14d321c44e6258e80d9386095`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AssetDefinitionId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::domain::model::Domain>`

> Schema summary: struct fields: object: DomainId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::domain::model::Domain>`
- Schema hash: `415a9559710c3f9a750fef2e1597b548`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `DomainId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::nft::model::Nft>`

> Schema summary: struct fields: object: NftId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::nft::model::Nft>`
- Schema hash: `1f6141e9b60b61347b01feb27afe3fc3`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NftId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::peer::model::Peer>`

> Schema summary: struct fields: object: PeerId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::peer::model::Peer>`
- Schema hash: `db66b397a842db8e8bf242f5d5975d3d`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `PeerId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::role::model::Role>`

> Schema summary: struct fields: object: RoleId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::role::model::Role>`
- Schema hash: `7deed2a68ad7badf726a78a41960818b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `RoleId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: TriggerId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `304c6094698804326e24ccd5ae83b4de`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `TriggerId` |

## `iroha_data_model::isi::repo::RepoIsi`

> Schema summary: struct fields: agreement_id: RepoAgreementId, initiator: AccountId, counterparty: AccountId, custodian: Option<AccountId>, cash_leg: RepoCashLeg, collateral_leg: RepoCollateralLeg, rate_bps: u16, maturity_timestamp_ms: u64, governance: RepoGovernance.

- Rust type: `iroha_data_model::isi::repo::RepoIsi`
- Schema hash: `c41ed8a16bddb247d065e7c02cded0a6`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `agreement_id` | `RepoAgreementId` |
| `initiator` | `AccountId` |
| `counterparty` | `AccountId` |
| `custodian` | `Option<AccountId>` |
| `cash_leg` | `RepoCashLeg` |
| `collateral_leg` | `RepoCollateralLeg` |
| `rate_bps` | `u16` |
| `maturity_timestamp_ms` | `u64` |
| `governance` | `RepoGovernance` |

## `iroha_data_model::isi::repo::ReverseRepoIsi`

> Schema summary: struct fields: agreement_id: RepoAgreementId, initiator: AccountId, counterparty: AccountId, cash_leg: RepoCashLeg, collateral_leg: RepoCollateralLeg, settlement_timestamp_ms: u64.

- Rust type: `iroha_data_model::isi::repo::ReverseRepoIsi`
- Schema hash: `eaaee02ec8bf5e55318885b656065fbb`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `agreement_id` | `RepoAgreementId` |
| `initiator` | `AccountId` |
| `counterparty` | `AccountId` |
| `cash_leg` | `RepoCashLeg` |
| `collateral_leg` | `RepoCollateralLeg` |
| `settlement_timestamp_ms` | `u64` |

## `iroha_data_model::isi::settlement::DvpIsi`

> Schema summary: struct fields: settlement_id: SettlementId, delivery_leg: SettlementLeg, payment_leg: SettlementLeg, plan: SettlementPlan, metadata: Metadata.

- Rust type: `iroha_data_model::isi::settlement::DvpIsi`
- Schema hash: `4f22f5d2f0d3e17b8768416ea8a918e2`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `settlement_id` | `SettlementId` |
| `delivery_leg` | `SettlementLeg` |
| `payment_leg` | `SettlementLeg` |
| `plan` | `SettlementPlan` |
| `metadata` | `Metadata` |

## `iroha_data_model::isi::settlement::PvpIsi`

> Schema summary: struct fields: settlement_id: SettlementId, primary_leg: SettlementLeg, counter_leg: SettlementLeg, plan: SettlementPlan, metadata: Metadata.

- Rust type: `iroha_data_model::isi::settlement::PvpIsi`
- Schema hash: `d7b3745676b57f9567790e5bd669a56e`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `settlement_id` | `SettlementId` |
| `primary_leg` | `SettlementLeg` |
| `counter_leg` | `SettlementLeg` |
| `plan` | `SettlementPlan` |
| `metadata` | `Metadata` |

## `iroha_data_model::isi::smart_contract_code::ActivateContractInstance`

> Schema summary: struct fields: contract_address: ContractAddress, code_hash: Hash.

- Rust type: `iroha_data_model::isi::smart_contract_code::ActivateContractInstance`
- Schema hash: `8ec0cf8ad0470dd7d021321f1cec8d47`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `code_hash` | `Hash` |

**Smart-contract notes:**

- Requires manifests and bytecode to exist for the supplied `code_hash`; activation binds `(namespace, contract_id)` to that digest.
- Protected namespaces continue to enforce governance approval, so Android SDKs should surface deterministic errors when admission fails.

## `iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload`

> Schema summary: struct fields: code_hash: Hash.

- Rust type: `iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload`
- Schema hash: `ea496a080ec700168bae4fae3e679d2b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `code_hash` | `Hash` |

## `iroha_data_model::isi::smart_contract_code::DeactivateContractInstance`

> Schema summary: struct fields: contract_address: ContractAddress, reason: Option<String>.

- Rust type: `iroha_data_model::isi::smart_contract_code::DeactivateContractInstance`
- Schema hash: `6667e876e3d9c279d0d2fe4fbdba34bf`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `contract_address` | `ContractAddress` |
| `reason` | `Option<String>` |

## `iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload`

> Schema summary: struct fields: code_hash: Hash, total_size: u64, chunk_count: u32.

- Rust type: `iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload`
- Schema hash: `0406dbcf58c0c157bdc2c690d3faba54`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `code_hash` | `Hash` |
| `total_size` | `u64` |
| `chunk_count` | `u32` |

## `iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes`

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
- Use the hashes in `docs/source/sdk/android/generated/fixtures/smart_contract_code_executor_hashes.json` to verify `.to` parsing logic in automation.

## `iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode`

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
| `abi_hash` | `Option<Hash>` | Hash of the syscall/pointer ABI surface for the supplied `abi_version` (see `docs/source/ivm_header.md`). |
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
- Sample hash pair derived from `defaults/executor.to` lives in `docs/source/sdk/android/generated/fixtures/smart_contract_code_executor_hashes.json` for deterministic builder tests.

## `iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes`

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

## `iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk`

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

## `iroha_data_model::isi::sorafs::ApprovePinManifest`

> Schema summary: struct fields: digest: ManifestDigest, approved_epoch: u64, council_envelope: Option<Vec<u8>>, council_envelope_digest: Option<Array<u8, 32>>.

- Rust type: `iroha_data_model::isi::sorafs::ApprovePinManifest`
- Schema hash: `1583c5673581a22cad86e51ca49aa514`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `digest` | `ManifestDigest` |
| `approved_epoch` | `u64` |
| `council_envelope` | `Option<Vec<u8>>` |
| `council_envelope_digest` | `Option<Array<u8, 32>>` |

## `iroha_data_model::isi::sorafs::BindManifestAlias`

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

## `iroha_data_model::isi::sorafs::CompleteReplicationOrder`

> Schema summary: struct fields: order_id: ReplicationOrderId, completion_epoch: u64.

- Rust type: `iroha_data_model::isi::sorafs::CompleteReplicationOrder`
- Schema hash: `b12e141f6fa82d6538e77613bc8f848c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `order_id` | `ReplicationOrderId` |
| `completion_epoch` | `u64` |

## `iroha_data_model::isi::sorafs::IssueReplicationOrder`

> Schema summary: struct fields: order_id: ReplicationOrderId, order_payload: Vec<u8>, issued_epoch: u64, deadline_epoch: u64.

- Rust type: `iroha_data_model::isi::sorafs::IssueReplicationOrder`
- Schema hash: `c4b340f0b6d646e6865d4a23087f5e2c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `order_id` | `ReplicationOrderId` |
| `order_payload` | `Vec<u8>` |
| `issued_epoch` | `u64` |
| `deadline_epoch` | `u64` |

## `iroha_data_model::isi::sorafs::RecordCapacityTelemetry`

> Schema summary: struct fields: record: CapacityTelemetryRecord.

- Rust type: `iroha_data_model::isi::sorafs::RecordCapacityTelemetry`
- Schema hash: `7378859e6a3f4607c1246f65cc6c896b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `record` | `CapacityTelemetryRecord` |

## `iroha_data_model::isi::sorafs::RegisterCapacityDeclaration`

> Schema summary: struct fields: record: CapacityDeclarationRecord.

- Rust type: `iroha_data_model::isi::sorafs::RegisterCapacityDeclaration`
- Schema hash: `9c77b1011c33673c919ac9d0a4ba0808`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `record` | `CapacityDeclarationRecord` |

## `iroha_data_model::isi::sorafs::RegisterCapacityDispute`

> Schema summary: struct fields: record: CapacityDisputeRecord.

- Rust type: `iroha_data_model::isi::sorafs::RegisterCapacityDispute`
- Schema hash: `7940e0ccdc6836d8e62b8b0cd27117f7`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `record` | `CapacityDisputeRecord` |

## `iroha_data_model::isi::sorafs::RegisterPinManifest`

> Schema summary: struct fields: manifest_payload: Vec<u8>, submitted_epoch: u64, alias: Option<ManifestAliasBinding>, successor_of: Option<ManifestDigest>.

- Rust type: `iroha_data_model::isi::sorafs::RegisterPinManifest`
- Schema hash: `61eb8eda15dad63ec8e3b35b58cfaa36`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `manifest_payload` | `Vec<u8>` |
| `submitted_epoch` | `u64` |
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

## `iroha_data_model::isi::sorafs::RetirePinManifest`

> Schema summary: struct fields: digest: ManifestDigest, retired_epoch: u64, reason: Option<String>.

- Rust type: `iroha_data_model::isi::sorafs::RetirePinManifest`
- Schema hash: `6da0ff52a6d999ecbd39aef38443f1a3`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `digest` | `ManifestDigest` |
| `retired_epoch` | `u64` |
| `reason` | `Option<String>` |

## `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::asset::id::model::AssetDefinitionId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: source: AccountId, object: AssetDefinitionId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::asset::id::model::AssetDefinitionId, iroha_data_model::account::model::Account>`
- Schema hash: `ddb2872f7c8834f2284a164bb5fcbab5`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `source` | `AccountId` |
| `object` | `AssetDefinitionId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::domain::model::DomainId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: source: AccountId, object: DomainId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::domain::model::DomainId, iroha_data_model::account::model::Account>`
- Schema hash: `6b65c37dafee48c7e94110bba6c8e64a`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `source` | `AccountId` |
| `object` | `DomainId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::nft::model::NftId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: source: AccountId, object: NftId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::nft::model::NftId, iroha_data_model::account::model::Account>`
- Schema hash: `309e39e79cd4ec83cea16f0b7553976e`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `source` | `AccountId` |
| `object` | `NftId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transfer::Transfer<iroha_data_model::asset::value::model::Asset, iroha_primitives::numeric::Quantity, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: source: AssetId, object: Quantity, destination: AccountId.

- Rust type: `iroha_data_model::isi::transfer::Transfer<iroha_data_model::asset::value::model::Asset, iroha_primitives::numeric::Quantity, iroha_data_model::account::model::Account>`
- Schema hash: `5e9403daad4734a27229e906de7b98e5`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `source` | `AssetId` |
| `object` | `Quantity` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::Grant<iroha_data_model::permission::model::Permission, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: Permission, destination: AccountId.

- Rust type: `iroha_data_model::isi::transparent::Grant<iroha_data_model::permission::model::Permission, iroha_data_model::account::model::Account>`
- Schema hash: `f478f05aab0d807e8728b4f2e88313f7`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Permission` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::Grant<iroha_data_model::permission::model::Permission, iroha_data_model::role::model::Role>`

> Schema summary: struct fields: object: Permission, destination: RoleId.

- Rust type: `iroha_data_model::isi::transparent::Grant<iroha_data_model::permission::model::Permission, iroha_data_model::role::model::Role>`
- Schema hash: `15a05cbd5e571e90e9241aa013dc5bb8`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Permission` |
| `destination` | `RoleId` |

## `iroha_data_model::isi::transparent::Grant<iroha_data_model::role::model::RoleId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: RoleId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transparent::Grant<iroha_data_model::role::model::RoleId, iroha_data_model::account::model::Account>`
- Schema hash: `1ec104a10801ceaaec02b66ffc0e426f`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `RoleId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::RemoveAssetKeyValue`

> Schema summary: struct fields: asset: AssetId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveAssetKeyValue`
- Schema hash: `8f0008f715ed9794ded9ae3a990243bf`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: AccountId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::account::model::Account>`
- Schema hash: `a48cc024ac906c0825f083042b2ab2d2`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AccountId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::asset::definition::model::AssetDefinition>`

> Schema summary: struct fields: object: AssetDefinitionId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::asset::definition::model::AssetDefinition>`
- Schema hash: `af4faf5b2ed7882e4ef8694fc36f0a2d`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AssetDefinitionId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::domain::model::Domain>`

> Schema summary: struct fields: object: DomainId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::domain::model::Domain>`
- Schema hash: `9d27745fa683d3c5bcd199c1cfc02a92`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `DomainId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::nft::model::Nft>`

> Schema summary: struct fields: object: NftId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::nft::model::Nft>`
- Schema hash: `54ef4ccf56060a7150f795cf5555de0a`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NftId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: TriggerId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `6185028dbdb06fb81f400eb1c4ccd7af`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `TriggerId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::Revoke<iroha_data_model::permission::model::Permission, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: Permission, destination: AccountId.

- Rust type: `iroha_data_model::isi::transparent::Revoke<iroha_data_model::permission::model::Permission, iroha_data_model::account::model::Account>`
- Schema hash: `b4e8890d3eb58a26f9198f2a4a145613`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Permission` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::Revoke<iroha_data_model::permission::model::Permission, iroha_data_model::role::model::Role>`

> Schema summary: struct fields: object: Permission, destination: RoleId.

- Rust type: `iroha_data_model::isi::transparent::Revoke<iroha_data_model::permission::model::Permission, iroha_data_model::role::model::Role>`
- Schema hash: `86966505240988489c9478d4a28a510c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Permission` |
| `destination` | `RoleId` |

## `iroha_data_model::isi::transparent::Revoke<iroha_data_model::role::model::RoleId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: RoleId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transparent::Revoke<iroha_data_model::role::model::RoleId, iroha_data_model::account::model::Account>`
- Schema hash: `e01c253454d61fd40263c86368206826`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `RoleId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::SetAssetKeyValue`

> Schema summary: struct fields: asset: AssetId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetAssetKeyValue`
- Schema hash: `5955e7b3e0166d3234997cb5738f5973`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: AccountId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::account::model::Account>`
- Schema hash: `ea9055ae2139aeef47c0133bff86fb17`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AccountId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::asset::definition::model::AssetDefinition>`

> Schema summary: struct fields: object: AssetDefinitionId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::asset::definition::model::AssetDefinition>`
- Schema hash: `2d6525268e249afd12d66568c6f58e1c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AssetDefinitionId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::domain::model::Domain>`

> Schema summary: struct fields: object: DomainId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::domain::model::Domain>`
- Schema hash: `230bb03ff6dae83646a190708c57aae3`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `DomainId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::nft::model::Nft>`

> Schema summary: struct fields: object: NftId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::nft::model::Nft>`
- Schema hash: `eff1db23399ed115ee6c4eeb1e22a4ef`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NftId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: TriggerId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `0b4c7498c135d5a9770cd3232c5294be`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `TriggerId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::verifying_keys::RegisterVerifyingKey`

> Schema summary: struct fields: id: VerifyingKeyId, record: VerifyingKeyRecord.

- Rust type: `iroha_data_model::isi::verifying_keys::RegisterVerifyingKey`
- Schema hash: `61c13e70ede9a90bacef2fcfb6457446`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `VerifyingKeyId` |
| `record` | `VerifyingKeyRecord` |

## `iroha_data_model::isi::verifying_keys::UpdateVerifyingKey`

> Schema summary: struct fields: id: VerifyingKeyId, record: VerifyingKeyRecord.

- Rust type: `iroha_data_model::isi::verifying_keys::UpdateVerifyingKey`
- Schema hash: `8b6f2a4b41a57ca1e852170e0e984e85`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `VerifyingKeyId` |
| `record` | `VerifyingKeyRecord` |

## `iroha_data_model::isi::zk::CreateElection`

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

## `iroha_data_model::isi::zk::FinalizeElection`

> Schema summary: struct fields: election_id: String, tally: Vec<u64>, tally_proof: ProofAttachment.

- Rust type: `iroha_data_model::isi::zk::FinalizeElection`
- Schema hash: `7382acea8661a36bc48f571339b0a6c4`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `election_id` | `String` |
| `tally` | `Vec<u64>` |
| `tally_proof` | `ProofAttachment` |

## `iroha_data_model::isi::zk::RegisterZkAsset`

> Schema summary: struct fields: asset: AssetDefinitionId, mode: ZkAssetMode, allow_shield: bool, allow_unshield: bool, vk_transfer: Option<VerifyingKeyId>, vk_unshield: Option<VerifyingKeyId>, vk_shield: Option<VerifyingKeyId>.

- Rust type: `iroha_data_model::isi::zk::RegisterZkAsset`
- Schema hash: `5fc0b16cf5cb3dd02292dc01fdcb8179`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `mode` | `ZkAssetMode` |
| `allow_shield` | `bool` |
| `allow_unshield` | `bool` |
| `vk_transfer` | `Option<VerifyingKeyId>` |
| `vk_unshield` | `Option<VerifyingKeyId>` |
| `vk_shield` | `Option<VerifyingKeyId>` |

## `iroha_data_model::isi::zk::Shield`

> Schema summary: struct fields: asset: AssetDefinitionId, from: AccountId, amount: Quantity, note_commitment: Array<u8, 32>, enc_payload: ConfidentialEncryptedPayload.

- Rust type: `iroha_data_model::isi::zk::Shield`
- Schema hash: `f6640dce7cdf2a695403dd2f3f71d93e`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `from` | `AccountId` |
| `amount` | `Quantity` |
| `note_commitment` | `Array<u8, 32>` |
| `enc_payload` | `ConfidentialEncryptedPayload` |

## `iroha_data_model::isi::zk::SubmitBallot`

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

## `iroha_data_model::isi::zk::Unshield`

> Schema summary: struct fields: asset: AssetDefinitionId, to: AccountId, public_amount: Quantity, inputs: Vec<Array<u8, 32>>, outputs: Vec<Array<u8, 32>>, proof: ProofAttachment, root_hint: Option<Array<u8, 32>>.

- Rust type: `iroha_data_model::isi::zk::Unshield`
- Schema hash: `1cb55ecc7fd92625b2bee33e491a4a0c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `to` | `AccountId` |
| `public_amount` | `Quantity` |
| `inputs` | `Vec<Array<u8, 32>>` |
| `outputs` | `Vec<Array<u8, 32>>` |
| `proof` | `ProofAttachment` |
| `root_hint` | `Option<Array<u8, 32>>` |

## `iroha_data_model::isi::zk::VerifyProof`

> Schema summary: struct fields: attachment: ProofAttachment.

- Rust type: `iroha_data_model::isi::zk::VerifyProof`
- Schema hash: `0b5d0ae55f342299f394799e85f61ae8`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `attachment` | `ProofAttachment` |

## `iroha_data_model::isi::zk::ZkTransfer`

> Schema summary: struct fields: asset: AssetDefinitionId, inputs: Vec<Array<u8, 32>>, outputs: Vec<Array<u8, 32>>, proof: ProofAttachment, root_hint: Option<Array<u8, 32>>.

- Rust type: `iroha_data_model::isi::zk::ZkTransfer`
- Schema hash: `47144daf134fc01da511d50f913c7b80`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `inputs` | `Vec<Array<u8, 32>>` |
| `outputs` | `Vec<Array<u8, 32>>` |
| `proof` | `ProofAttachment` |
| `root_hint` | `Option<Array<u8, 32>>` |

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
