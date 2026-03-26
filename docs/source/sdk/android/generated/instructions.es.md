---
lang: es
direction: ltr
source: docs/source/sdk/android/generated/instructions.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 301d6d415aa3471d8df56e49e7b608ad8d27b4a5f9087ff7a8a6d555007f5e37
source_last_modified: "2026-01-22T06:58:49.756391+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated via scripts/android_codegen_docs.py -->
# Android Instruction Reference

This file is generated from `instruction_manifest.json`. Do not edit manually.

## `iroha.burn`

> Schema summary: enum variants: Asset (Burn<Numeric, Asset>), TriggerRepetitions (Burn<u32, Trigger>).

- Rust type: `iroha_data_model::isi::mint_burn::BurnBox`
- Schema hash: `b8a981b5d11aaa54b8a981b5d11aaa54`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Asset` | 0 | `Burn<Numeric, Asset>` |
| `TriggerRepetitions` | 1 | `Burn<u32, Trigger>` |

## `iroha.custom`

> Schema summary: struct fields: payload: Json.

- Rust type: `iroha_data_model::isi::transparent::CustomInstruction`
- Schema hash: `89e9c7d440cf16b689e9c7d440cf16b6`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `payload` | `Json` |

## `iroha.execute_trigger`

> Schema summary: struct fields: trigger: TriggerId, args: Json.

- Rust type: `iroha_data_model::isi::transparent::ExecuteTrigger`
- Schema hash: `f92cdee1227ffa12f92cdee1227ffa12`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `trigger` | `TriggerId` |
| `args` | `Json` |

## `iroha.grant`

> Schema summary: enum variants: Permission (Grant<Permission, Account>), Role (Grant<RoleId, Account>), RolePermission (Grant<Permission, Role>).

- Rust type: `iroha_data_model::isi::GrantBox`
- Schema hash: `21d628458e22661321d628458e226613`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Permission` | 0 | `Grant<Permission, Account>` |
| `Role` | 1 | `Grant<RoleId, Account>` |
| `RolePermission` | 2 | `Grant<Permission, Role>` |

## `iroha.log`

> Schema summary: struct fields: level: Level, msg: String.

- Rust type: `iroha_data_model::isi::transparent::Log`
- Schema hash: `9a26c3b474fedb119a26c3b474fedb11`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `level` | `Level` |
| `msg` | `String` |

## `iroha.mint`

> Schema summary: enum variants: Asset (Mint<Numeric, Asset>), TriggerRepetitions (Mint<u32, Trigger>).

- Rust type: `iroha_data_model::isi::mint_burn::MintBox`
- Schema hash: `a77313c153163964a77313c153163964`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Asset` | 0 | `Mint<Numeric, Asset>` |
| `TriggerRepetitions` | 1 | `Mint<u32, Trigger>` |

## `iroha.register`

> Schema summary: enum variants: Peer (RegisterPeerWithPop), Domain (Register<Domain>), Account (Register<Account>), AssetDefinition (Register<AssetDefinition>), Nft (Register<Nft>), Role (Register<Role>), Trigger (Register<Trigger>).

- Rust type: `iroha_data_model::isi::register::RegisterBox`
- Schema hash: `5321ba5d245cb6f65321ba5d245cb6f6`

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
- Schema hash: `17a266cdeca147f517a266cdeca147f5`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Domain` | 0 | `RemoveKeyValue<Domain>` |
| `Account` | 1 | `RemoveKeyValue<Account>` |
| `AssetDefinition` | 2 | `RemoveKeyValue<AssetDefinition>` |
| `Nft` | 3 | `RemoveKeyValue<Nft>` |
| `Trigger` | 4 | `RemoveKeyValue<Trigger>` |

## `iroha.repo.initiate`

> Schema summary: struct fields: agreement_id: RepoAgreementId, initiator: AccountId, counterparty: AccountId, custodian: Option<AccountId>, cash_leg: RepoCashLeg, collateral_leg: RepoCollateralLeg, rate_bps: u16, maturity_timestamp_ms: u64, governance: RepoGovernance.

- Rust type: `iroha_data_model::isi::repo::RepoIsi`
- Schema hash: `699ebd9e4662c508699ebd9e4662c508`

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

## `iroha.repo.reverse`

> Schema summary: struct fields: agreement_id: RepoAgreementId, initiator: AccountId, counterparty: AccountId, cash_leg: RepoCashLeg, collateral_leg: RepoCollateralLeg, settlement_timestamp_ms: u64.

- Rust type: `iroha_data_model::isi::repo::ReverseRepoIsi`
- Schema hash: `db8564a6098daa97db8564a6098daa97`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `agreement_id` | `RepoAgreementId` |
| `initiator` | `AccountId` |
| `counterparty` | `AccountId` |
| `cash_leg` | `RepoCashLeg` |
| `collateral_leg` | `RepoCollateralLeg` |
| `settlement_timestamp_ms` | `u64` |

## `iroha.revoke`

> Schema summary: enum variants: Permission (Revoke<Permission, Account>), Role (Revoke<RoleId, Account>), RolePermission (Revoke<Permission, Role>).

- Rust type: `iroha_data_model::isi::RevokeBox`
- Schema hash: `8103f6c9994ea6308103f6c9994ea630`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Permission` | 0 | `Revoke<Permission, Account>` |
| `Role` | 1 | `Revoke<RoleId, Account>` |
| `RolePermission` | 2 | `Revoke<Permission, Role>` |

## `iroha.runtime_upgrade.activate`

> Schema summary: struct fields: id: RuntimeUpgradeId.

- Rust type: `iroha_data_model::isi::runtime_upgrade::ActivateRuntimeUpgrade`
- Schema hash: `c8dbd3041daa7149c8dbd3041daa7149`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `RuntimeUpgradeId` |

## `iroha.runtime_upgrade.cancel`

> Schema summary: struct fields: id: RuntimeUpgradeId.

- Rust type: `iroha_data_model::isi::runtime_upgrade::CancelRuntimeUpgrade`
- Schema hash: `dffda7aec9caf96ddffda7aec9caf96d`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `RuntimeUpgradeId` |

## `iroha.runtime_upgrade.propose`

> Schema summary: struct fields: manifest_bytes: Vec<u8>.

- Rust type: `iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade`
- Schema hash: `21d69a2ac8ea67f021d69a2ac8ea67f0`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `manifest_bytes` | `Vec<u8>` |

## `iroha.set_key_value`

> Schema summary: enum variants: Domain (SetKeyValue<Domain>), Account (SetKeyValue<Account>), AssetDefinition (SetKeyValue<AssetDefinition>), Nft (SetKeyValue<Nft>), Trigger (SetKeyValue<Trigger>).

- Rust type: `iroha_data_model::isi::SetKeyValueBox`
- Schema hash: `9b074cea7d8371cf9b074cea7d8371cf`

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
- Schema hash: `77acf9687667a5f177acf9687667a5f1`

**Layout:** `tuple`

| Field | Type |
|-------|------|
| `0` | `Parameter` |

## `iroha.settlement.dvp`

> Schema summary: struct fields: settlement_id: SettlementId, delivery_leg: SettlementLeg, payment_leg: SettlementLeg, plan: SettlementPlan, metadata: Metadata.

- Rust type: `iroha_data_model::isi::settlement::DvpIsi`
- Schema hash: `e44952e42c0ea191e44952e42c0ea191`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `settlement_id` | `SettlementId` |
| `delivery_leg` | `SettlementLeg` |
| `payment_leg` | `SettlementLeg` |
| `plan` | `SettlementPlan` |
| `metadata` | `Metadata` |

## `iroha.settlement.pvp`

> Schema summary: struct fields: settlement_id: SettlementId, primary_leg: SettlementLeg, counter_leg: SettlementLeg, plan: SettlementPlan, metadata: Metadata.

- Rust type: `iroha_data_model::isi::settlement::PvpIsi`
- Schema hash: `000d52b7bd46a0e1000d52b7bd46a0e1`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `settlement_id` | `SettlementId` |
| `primary_leg` | `SettlementLeg` |
| `counter_leg` | `SettlementLeg` |
| `plan` | `SettlementPlan` |
| `metadata` | `Metadata` |

## `iroha.transfer`

> Schema summary: enum variants: Domain (Transfer<Account, DomainId, Account>), AssetDefinition (Transfer<Account, AssetDefinitionId, Account>), Asset (Transfer<Asset, Numeric, Account>), Nft (Transfer<Account, NftId, Account>).

- Rust type: `iroha_data_model::isi::transfer::TransferBox`
- Schema hash: `ffb7ba8dc1b0c984ffb7ba8dc1b0c984`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Domain` | 0 | `Transfer<Account, DomainId, Account>` |
| `AssetDefinition` | 1 | `Transfer<Account, AssetDefinitionId, Account>` |
| `Asset` | 2 | `Transfer<Asset, Numeric, Account>` |
| `Nft` | 3 | `Transfer<Account, NftId, Account>` |

## `iroha.transfer_batch`

> Schema summary: struct fields: entries: Vec<TransferAssetBatchEntry>.

- Rust type: `iroha_data_model::isi::transfer::TransferAssetBatch`
- Schema hash: `b267543a7f746edfb267543a7f746edf`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `entries` | `Vec<TransferAssetBatchEntry>` |

## `iroha.unregister`

> Schema summary: enum variants: Peer (Unregister<Peer>), Domain (Unregister<Domain>), Account (Unregister<Account>), AssetDefinition (Unregister<AssetDefinition>), Nft (Unregister<Nft>), Role (Unregister<Role>), Trigger (Unregister<Trigger>).

- Rust type: `iroha_data_model::isi::register::UnregisterBox`
- Schema hash: `fa527ae77f909394fa527ae77f909394`

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
- Schema hash: `84e4bd4fa6cb5b1e84e4bd4fa6cb5b1e`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `executor` | `Executor` |

## `iroha_data_model::isi::governance::CastPlainBallot`

> Schema summary: struct fields: referendum_id: String, owner: AccountId, amount: u128, duration_blocks: u64, direction: u8.

- Rust type: `iroha_data_model::isi::governance::CastPlainBallot`
- Schema hash: `9969f69b4a99a0749969f69b4a99a074`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `referendum_id` | `String` |
| `owner` | `AccountId` |
| `amount` | `u128` |
| `duration_blocks` | `u64` |
| `direction` | `u8` |

## `iroha_data_model::isi::governance::CastZkBallot`

> Schema summary: struct fields: election_id: String, proof_b64: String, public_inputs_json: String.

- Rust type: `iroha_data_model::isi::governance::CastZkBallot`
- Schema hash: `58d9049c2c73912958d9049c2c739129`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `election_id` | `String` |
| `proof_b64` | `String` |
| `public_inputs_json` | `String` |

## `iroha_data_model::isi::governance::EnactReferendum`

> Schema summary: struct fields: referendum_id: Array<u8, 32>, preimage_hash: Array<u8, 32>, at_window: AtWindow.

- Rust type: `iroha_data_model::isi::governance::EnactReferendum`
- Schema hash: `564da81425d228de564da81425d228de`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `referendum_id` | `Array<u8, 32>` |
| `preimage_hash` | `Array<u8, 32>` |
| `at_window` | `AtWindow` |

## `iroha_data_model::isi::governance::FinalizeReferendum`

> Schema summary: struct fields: referendum_id: String, proposal_id: Array<u8, 32>.

- Rust type: `iroha_data_model::isi::governance::FinalizeReferendum`
- Schema hash: `316f68c14913465e316f68c14913465e`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `referendum_id` | `String` |
| `proposal_id` | `Array<u8, 32>` |

## `iroha_data_model::isi::governance::PersistCouncilForEpoch`

> Schema summary: struct fields: epoch: u64, members: Vec<AccountId>, candidates_count: u32, derived_by: CouncilDerivationKind.

- Rust type: `iroha_data_model::isi::governance::PersistCouncilForEpoch`
- Schema hash: `25f004fc72a647fa25f004fc72a647fa`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `epoch` | `u64` |
| `members` | `Vec<AccountId>` |
| `candidates_count` | `u32` |
| `derived_by` | `CouncilDerivationKind` |

## `iroha_data_model::isi::governance::ProposeDeployContract`

> Schema summary: struct fields: namespace: String, contract_id: String, code_hash_hex: String, abi_hash_hex: String, abi_version: String, window: Option<AtWindow>, mode: Option<VotingMode>.

- Rust type: `iroha_data_model::isi::governance::ProposeDeployContract`
- Schema hash: `d92fab6392e8299fd92fab6392e8299f`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `namespace` | `String` |
| `contract_id` | `String` |
| `code_hash_hex` | `String` |
| `abi_hash_hex` | `String` |
| `abi_version` | `String` |
| `window` | `Option<AtWindow>` |
| `mode` | `Option<VotingMode>` |

## `iroha_data_model::isi::kaigi::CreateKaigi`

> Schema summary: struct fields: call: NewKaigi.

- Rust type: `iroha_data_model::isi::kaigi::CreateKaigi`
- Schema hash: `24ee2ad1d6a56d3524ee2ad1d6a56d35`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call` | `NewKaigi` |

## `iroha_data_model::isi::kaigi::EndKaigi`

> Schema summary: struct fields: call_id: KaigiId, ended_at_ms: Option<u64>.

- Rust type: `iroha_data_model::isi::kaigi::EndKaigi`
- Schema hash: `85befda0409d3c0485befda0409d3c04`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `ended_at_ms` | `Option<u64>` |

## `iroha_data_model::isi::kaigi::JoinKaigi`

> Schema summary: struct fields: call_id: KaigiId, participant: AccountId, commitment: Option<KaigiParticipantCommitment>, nullifier: Option<KaigiParticipantNullifier>, roster_root: Option<Hash>, proof: Option<Vec<u8>>.

- Rust type: `iroha_data_model::isi::kaigi::JoinKaigi`
- Schema hash: `5077ea3be6f706825077ea3be6f70682`

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
- Schema hash: `d74b8812a0a2681cd74b8812a0a2681c`

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
- Schema hash: `e20fb919a4056c21e20fb919a4056c21`

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
- Schema hash: `b40e80079720b8a2b40e80079720b8a2`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `relay` | `KaigiRelayRegistration` |

## `iroha_data_model::isi::kaigi::SetKaigiRelayManifest`

> Schema summary: struct fields: call_id: KaigiId, relay_manifest: Option<KaigiRelayManifest>.

- Rust type: `iroha_data_model::isi::kaigi::SetKaigiRelayManifest`
- Schema hash: `726dd6413d1d2b01726dd6413d1d2b01`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `call_id` | `KaigiId` |
| `relay_manifest` | `Option<KaigiRelayManifest>` |

## `iroha_data_model::isi::mint_burn::Burn<iroha_primitives::numeric::Numeric, iroha_data_model::asset::value::model::Asset>`

> Schema summary: struct fields: object: Numeric, destination: AssetId.

- Rust type: `iroha_data_model::isi::mint_burn::Burn<iroha_primitives::numeric::Numeric, iroha_data_model::asset::value::model::Asset>`
- Schema hash: `9534672edb4e0a2b9534672edb4e0a2b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Numeric` |
| `destination` | `AssetId` |

## `iroha_data_model::isi::mint_burn::Burn<u32, iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: u32, destination: TriggerId.

- Rust type: `iroha_data_model::isi::mint_burn::Burn<u32, iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `4169ed08d250db844169ed08d250db84`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `u32` |
| `destination` | `TriggerId` |

## `iroha_data_model::isi::mint_burn::Mint<iroha_primitives::numeric::Numeric, iroha_data_model::asset::value::model::Asset>`

> Schema summary: struct fields: object: Numeric, destination: AssetId.

- Rust type: `iroha_data_model::isi::mint_burn::Mint<iroha_primitives::numeric::Numeric, iroha_data_model::asset::value::model::Asset>`
- Schema hash: `44d10ee70609795a44d10ee70609795a`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Numeric` |
| `destination` | `AssetId` |

## `iroha_data_model::isi::mint_burn::Mint<u32, iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: u32, destination: TriggerId.

- Rust type: `iroha_data_model::isi::mint_burn::Mint<u32, iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `3a9c22c89cf530303a9c22c89cf53030`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `u32` |
| `destination` | `TriggerId` |

## `iroha_data_model::isi::register::Register<iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: NewAccount.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::account::model::Account>`
- Schema hash: `6b56657dd89f07e76b56657dd89f07e7`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewAccount` |

## `iroha_data_model::isi::register::Register<iroha_data_model::asset::definition::model::AssetDefinition>`

> Schema summary: struct fields: object: NewAssetDefinition.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::asset::definition::model::AssetDefinition>`
- Schema hash: `2f8ffa740d701cd52f8ffa740d701cd5`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewAssetDefinition` |

## `iroha_data_model::isi::register::Register<iroha_data_model::domain::model::Domain>`

> Schema summary: struct fields: object: NewDomain.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::domain::model::Domain>`
- Schema hash: `cd47fd557a3668b5cd47fd557a3668b5`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewDomain` |

## `iroha_data_model::isi::register::Register<iroha_data_model::nft::model::Nft>`

> Schema summary: struct fields: object: NewNft.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::nft::model::Nft>`
- Schema hash: `2be70a0e3acb18442be70a0e3acb1844`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewNft` |

## `iroha_data_model::isi::register::Register<iroha_data_model::role::model::Role>`

> Schema summary: struct fields: object: NewRole.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::role::model::Role>`
- Schema hash: `d1e6da9e715220c3d1e6da9e715220c3`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NewRole` |

## `iroha_data_model::isi::register::Register<iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: Trigger.

- Rust type: `iroha_data_model::isi::register::Register<iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `b858d433d86c147fb858d433d86c147f`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Trigger` |

## `iroha_data_model::isi::register::RegisterPeerWithPop`

> Schema summary: struct fields: peer: PeerId, pop: Vec<u8>.

- Rust type: `iroha_data_model::isi::register::RegisterPeerWithPop`
- Schema hash: `a13876e96b8c55ada13876e96b8c55ad`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `peer` | `PeerId` |
| `pop` | `Vec<u8>` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: AccountId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::account::model::Account>`
- Schema hash: `cad12f4762d46f3bcad12f4762d46f3b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AccountId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::asset::definition::model::AssetDefinition>`

> Schema summary: struct fields: object: AssetDefinitionId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::asset::definition::model::AssetDefinition>`
- Schema hash: `2289f3cda79c46c52289f3cda79c46c5`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AssetDefinitionId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::domain::model::Domain>`

> Schema summary: struct fields: object: DomainId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::domain::model::Domain>`
- Schema hash: `04be33ece13b5ab104be33ece13b5ab1`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `DomainId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::nft::model::Nft>`

> Schema summary: struct fields: object: NftId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::nft::model::Nft>`
- Schema hash: `aafcaf361e054c16aafcaf361e054c16`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NftId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::peer::model::Peer>`

> Schema summary: struct fields: object: PeerId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::peer::model::Peer>`
- Schema hash: `68e6d48351e58d7168e6d48351e58d71`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `PeerId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::role::model::Role>`

> Schema summary: struct fields: object: RoleId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::role::model::Role>`
- Schema hash: `a8a58d646177b9aaa8a58d646177b9aa`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `RoleId` |

## `iroha_data_model::isi::register::Unregister<iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: TriggerId.

- Rust type: `iroha_data_model::isi::register::Unregister<iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `d791bbaad100b369d791bbaad100b369`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `TriggerId` |

## `iroha_data_model::isi::repo::RepoInstructionBox`

> Schema summary: enum variants: Initiate (RepoIsi), Reverse (ReverseRepoIsi), MarginCall (RepoMarginCallIsi).

- Rust type: `iroha_data_model::isi::repo::RepoInstructionBox`
- Schema hash: `97c88af835840b2d97c88af835840b2d`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Initiate` | 0 | `RepoIsi` |
| `Reverse` | 1 | `ReverseRepoIsi` |
| `MarginCall` | 2 | `RepoMarginCallIsi` |

## `iroha_data_model::isi::settlement::SettlementInstructionBox`

> Schema summary: enum variants: Dvp (DvpIsi), Pvp (PvpIsi).

- Rust type: `iroha_data_model::isi::settlement::SettlementInstructionBox`
- Schema hash: `0bf625b40d139d700bf625b40d139d70`

**Layout:** `enum`

| Tag | Discriminant | Payload |
|-----|--------------|---------|
| `Dvp` | 0 | `DvpIsi` |
| `Pvp` | 1 | `PvpIsi` |

## `iroha_data_model::isi::smart_contract_code::ActivateContractInstance`

> Schema summary: struct fields: namespace: String, contract_id: String, code_hash: Hash.

- Rust type: `iroha_data_model::isi::smart_contract_code::ActivateContractInstance`
- Schema hash: `829e0d2a934213bf829e0d2a934213bf`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `namespace` | `String` |
| `contract_id` | `String` |
| `code_hash` | `Hash` |

**Smart-contract notes:**

- Requires manifests and bytecode to exist for the supplied `code_hash`; activation binds `(namespace, contract_id)` to that digest.
- Protected namespaces continue to enforce governance approval, so Android SDKs should surface deterministic errors when admission fails.

## `iroha_data_model::isi::smart_contract_code::DeactivateContractInstance`

> Schema summary: struct fields: namespace: String, contract_id: String, reason: Option<String>.

- Rust type: `iroha_data_model::isi::smart_contract_code::DeactivateContractInstance`
- Schema hash: `351293113eec3144351293113eec3144`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `namespace` | `String` |
| `contract_id` | `String` |
| `reason` | `Option<String>` |

## `iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes`

> Schema summary: struct fields: code_hash: Hash, code: Vec<u8>.

- Rust type: `iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes`
- Schema hash: `458b53cef6502236458b53cef6502236`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `code_hash` | `Hash` |
| `code` | `Vec<u8>` |

**Smart-contract notes:**

- `code_hash` must equal the Blake2b-32 digest of the program body (bytes after the IVM header); duplicate uploads re-use the stored bytes.
- Use the hashes in `docs/source/sdk/android/generated/fixtures/smart_contract_code_executor_hashes.json` to verify `.to` parsing logic in automation.

## `iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode`

> Schema summary: struct fields: manifest: ContractManifest.

- Rust type: `iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode`
- Schema hash: `63eec8b1a5dfcb1263eec8b1a5dfcb12`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `manifest` | `ContractManifest` |

### Manifest field details

#### ContractManifest fields

Optional metadata attached to smart-contract deployments; hash fields must match the canonical host-computed values before admission.

| Field | Type | Description |
|-------|------|-------------|
| `code_hash` | `Option<Hash>` | Blake2b-32 digest of the `.to` program body (bytes after the IVM header). |
| `abi_hash` | `Option<Hash>` | Hash of the syscall/pointer ABI surface for the supplied `abi_version` (see `docs/source/ivm_header.md`). |
| `compiler_fingerprint` | `Option<String>` | Compiler + toolchain note recorded for provenance. |
| `features_bitmap` | `Option<u64>` | Bitmask of build features (SIMD, CUDA, etc.). |
| `access_set_hints` | `Option<AccessSetHints>` | Advisory read/write key hints for the scheduler. |
| `entrypoints` | `Option<Vec<EntrypointDescriptor>>` | Optional entrypoint descriptors advertised by the compiler. |

#### AccessSetHints fields

Declarative read/write key hints stored inside smart-contract manifests.

| Field | Type | Description |
|-------|------|-------------|
| `read_keys` | `Vec<String>` | Canonical keys (e.g., `account:<katakana-i105-account-id>`) the contract expects to read. |
| `write_keys` | `Vec<String>` | Keys that the contract expects to write during execution. |

#### EntrypointDescriptor fields

Metadata emitted per Kotodama entrypoint.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Symbol name declared in Kotodama source. |
| `kind` | `EntryPointKind` | Role of the entrypoint (`Public`, `Hajimari`, or `Kaizen`). |
| `permission` | `Option<String>` | Optional dispatcher permission required before invocation. |
| `read_keys` | `Vec<String>` | Advisory read set scoped to the entrypoint. |
| `write_keys` | `Vec<String>` | Advisory write set scoped to the entrypoint. |
| `access_hints_complete` | `Option<bool>` | Whether access-set hints were emitted (wildcards are used when coverage is conservative). |
| `access_hints_skipped` | `Vec<String>` | Reserved; currently empty because opaque access emits wildcard hints instead of skipping. |
| `triggers` | `Vec<TriggerDescriptor>` | Trigger declarations that call this entrypoint. |

#### TriggerCallback fields

Entrypoint callback target referenced by a trigger declaration.

| Field | Type | Description |
|-------|------|-------------|
| `namespace` | `Option<String>` | Optional contract namespace for cross-contract callbacks. |
| `entrypoint` | `String` | Entrypoint name to invoke. |

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

**Smart-contract notes:**

- Nodes recompute `manifest.code_hash` from the `.to` artifact and reject mismatches; `manifest.abi_hash` must equal the canonical ABI digest for the declared version.
- Sample hash pair derived from `defaults/executor.to` lives in `docs/source/sdk/android/generated/fixtures/smart_contract_code_executor_hashes.json` for deterministic builder tests.

## `iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes`

> Schema summary: struct fields: code_hash: Hash, reason: Option<String>.

- Rust type: `iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes`
- Schema hash: `645fa1f41c603c82645fa1f41c603c82`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `code_hash` | `Hash` |
| `reason` | `Option<String>` |

**Smart-contract notes:**

- Removal succeeds only when no manifest or active instance references the target `code_hash`; provide an audit reason when automating removals.

## `iroha_data_model::isi::sorafs::ApprovePinManifest`

> Schema summary: struct fields: digest: ManifestDigest, approved_epoch: u64, council_envelope: Option<Vec<u8>>, council_envelope_digest: Option<Array<u8, 32>>.

- Rust type: `iroha_data_model::isi::sorafs::ApprovePinManifest`
- Schema hash: `1f1c7d3046f69a091f1c7d3046f69a09`

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
- Schema hash: `10689fa9cfd6fa3c10689fa9cfd6fa3c`

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
| `name` | `String` | Alias label (e.g., `docs`). |
| `namespace` | `String` | Alias namespace (e.g., `sora`). |
| `proof` | `Vec<u8>` | Norito-encoded alias proof bytes (base64 in JSON). |

## `iroha_data_model::isi::sorafs::CompleteReplicationOrder`

> Schema summary: struct fields: order_id: ReplicationOrderId, completion_epoch: u64.

- Rust type: `iroha_data_model::isi::sorafs::CompleteReplicationOrder`
- Schema hash: `2349156305687aa92349156305687aa9`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `order_id` | `ReplicationOrderId` |
| `completion_epoch` | `u64` |

## `iroha_data_model::isi::sorafs::IssueReplicationOrder`

> Schema summary: struct fields: order_id: ReplicationOrderId, order_payload: Vec<u8>, issued_epoch: u64, deadline_epoch: u64.

- Rust type: `iroha_data_model::isi::sorafs::IssueReplicationOrder`
- Schema hash: `833ba5f3cc34fac9833ba5f3cc34fac9`

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
- Schema hash: `9ad5ce8c8a5d93339ad5ce8c8a5d9333`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `record` | `CapacityTelemetryRecord` |

## `iroha_data_model::isi::sorafs::RegisterCapacityDeclaration`

> Schema summary: struct fields: record: CapacityDeclarationRecord.

- Rust type: `iroha_data_model::isi::sorafs::RegisterCapacityDeclaration`
- Schema hash: `ada4966279095dc6ada4966279095dc6`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `record` | `CapacityDeclarationRecord` |

## `iroha_data_model::isi::sorafs::RegisterCapacityDispute`

> Schema summary: struct fields: record: CapacityDisputeRecord.

- Rust type: `iroha_data_model::isi::sorafs::RegisterCapacityDispute`
- Schema hash: `29c330490cf2e64c29c330490cf2e64c`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `record` | `CapacityDisputeRecord` |

## `iroha_data_model::isi::sorafs::RegisterPinManifest`

> Schema summary: struct fields: digest: ManifestDigest, chunker: ChunkerProfileHandle, chunk_digest_sha3_256: Array<u8, 32>, policy: PinPolicy, submitted_epoch: u64, alias: Option<ManifestAliasBinding>, successor_of: Option<ManifestDigest>.

- Rust type: `iroha_data_model::isi::sorafs::RegisterPinManifest`
- Schema hash: `7fa6a5b7e48dcdaa7fa6a5b7e48dcdaa`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `digest` | `ManifestDigest` |
| `chunker` | `ChunkerProfileHandle` |
| `chunk_digest_sha3_256` | `Array<u8, 32>` |
| `policy` | `PinPolicy` |
| `submitted_epoch` | `u64` |
| `alias` | `Option<ManifestAliasBinding>` |
| `successor_of` | `Option<ManifestDigest>` |

### Manifest field details

#### ChunkerProfileHandle fields

Registry descriptor describing the chunking profile used to build the CAR.

| Field | Type | Description |
|-------|------|-------------|
| `profile_id` | `u32` | Numeric profile identifier (`ProfileId`). |
| `namespace` | `String` | Registry namespace (typically `sorafs`). |
| `name` | `String` | Human-readable profile name (for example `sf1`). |
| `semver` | `String` | Semantic version string of the parameter set. |
| `multihash_code` | `u64` | Multihash code used when deriving chunk digests. |

#### PinPolicy fields

Storage replication policy negotiated with the pin registry.

| Field | Type | Description |
|-------|------|-------------|
| `min_replicas` | `u16` | Minimum replica count required by governance. |
| `storage_class` | `StorageClass` | Requested storage tier (`Hot`, `Warm`, `Cold`). |
| `retention_epoch` | `u64` | Inclusive epoch through which the manifest must remain pinned. |

#### StorageClass variants

Storage tier classification for `SoraFS` replicas.

| Variant | Description |
|---------|-------------|
| `Hot` | Low-latency replicas for developer workflows. |
| `Warm` | Cost-optimised replicas with relaxed latency. |
| `Cold` | Archival replicas retained for compliance. |

#### ManifestAliasBinding fields

Alias binding payload approved alongside a manifest.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Alias label (e.g., `docs`). |
| `namespace` | `String` | Alias namespace (e.g., `sora`). |
| `proof` | `Vec<u8>` | Norito-encoded alias proof bytes (base64 in JSON). |

## `iroha_data_model::isi::sorafs::RetirePinManifest`

> Schema summary: struct fields: digest: ManifestDigest, retired_epoch: u64, reason: Option<String>.

- Rust type: `iroha_data_model::isi::sorafs::RetirePinManifest`
- Schema hash: `191ef654cefea496191ef654cefea496`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `digest` | `ManifestDigest` |
| `retired_epoch` | `u64` |
| `reason` | `Option<String>` |

## `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::asset::id::model::AssetDefinitionId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: source: AccountId, object: AssetDefinitionId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::asset::id::model::AssetDefinitionId, iroha_data_model::account::model::Account>`
- Schema hash: `f80edc9082e78aa0f80edc9082e78aa0`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `source` | `AccountId` |
| `object` | `AssetDefinitionId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::domain::model::DomainId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: source: AccountId, object: DomainId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::domain::model::DomainId, iroha_data_model::account::model::Account>`
- Schema hash: `469a5e8db6b4add3469a5e8db6b4add3`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `source` | `AccountId` |
| `object` | `DomainId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::nft::model::NftId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: source: AccountId, object: NftId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transfer::Transfer<iroha_data_model::account::model::Account, iroha_data_model::nft::model::NftId, iroha_data_model::account::model::Account>`
- Schema hash: `0054af4882eabfde0054af4882eabfde`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `source` | `AccountId` |
| `object` | `NftId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transfer::Transfer<iroha_data_model::asset::value::model::Asset, iroha_primitives::numeric::Numeric, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: source: AssetId, object: Numeric, destination: AccountId.

- Rust type: `iroha_data_model::isi::transfer::Transfer<iroha_data_model::asset::value::model::Asset, iroha_primitives::numeric::Numeric, iroha_data_model::account::model::Account>`
- Schema hash: `536fd0ff0010e42e536fd0ff0010e42e`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `source` | `AssetId` |
| `object` | `Numeric` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::Grant<iroha_data_model::permission::model::Permission, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: Permission, destination: AccountId.

- Rust type: `iroha_data_model::isi::transparent::Grant<iroha_data_model::permission::model::Permission, iroha_data_model::account::model::Account>`
- Schema hash: `e24e6ea7302d74c6e24e6ea7302d74c6`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Permission` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::Grant<iroha_data_model::permission::model::Permission, iroha_data_model::role::model::Role>`

> Schema summary: struct fields: object: Permission, destination: RoleId.

- Rust type: `iroha_data_model::isi::transparent::Grant<iroha_data_model::permission::model::Permission, iroha_data_model::role::model::Role>`
- Schema hash: `00171a937a9fbab900171a937a9fbab9`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Permission` |
| `destination` | `RoleId` |

## `iroha_data_model::isi::transparent::Grant<iroha_data_model::role::model::RoleId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: RoleId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transparent::Grant<iroha_data_model::role::model::RoleId, iroha_data_model::account::model::Account>`
- Schema hash: `dd43425d69ea0f4bdd43425d69ea0f4b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `RoleId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::RemoveAssetKeyValue`

> Schema summary: struct fields: asset: AssetId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveAssetKeyValue`
- Schema hash: `54d0c35a5448897054d0c35a54488970`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: AccountId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::account::model::Account>`
- Schema hash: `41b4f18a5c13aac741b4f18a5c13aac7`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AccountId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::asset::definition::model::AssetDefinition>`

> Schema summary: struct fields: object: AssetDefinitionId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::asset::definition::model::AssetDefinition>`
- Schema hash: `2d603db6b8906a512d603db6b8906a51`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AssetDefinitionId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::domain::model::Domain>`

> Schema summary: struct fields: object: DomainId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::domain::model::Domain>`
- Schema hash: `8b39ae185f2ddfd58b39ae185f2ddfd5`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `DomainId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::nft::model::Nft>`

> Schema summary: struct fields: object: NftId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::nft::model::Nft>`
- Schema hash: `b165d2b1afc3d451b165d2b1afc3d451`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NftId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: TriggerId, key: Name.

- Rust type: `iroha_data_model::isi::transparent::RemoveKeyValue<iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `76cc1312388ff5f476cc1312388ff5f4`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `TriggerId` |
| `key` | `Name` |

## `iroha_data_model::isi::transparent::Revoke<iroha_data_model::permission::model::Permission, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: Permission, destination: AccountId.

- Rust type: `iroha_data_model::isi::transparent::Revoke<iroha_data_model::permission::model::Permission, iroha_data_model::account::model::Account>`
- Schema hash: `d287e2bae269216fd287e2bae269216f`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Permission` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::Revoke<iroha_data_model::permission::model::Permission, iroha_data_model::role::model::Role>`

> Schema summary: struct fields: object: Permission, destination: RoleId.

- Rust type: `iroha_data_model::isi::transparent::Revoke<iroha_data_model::permission::model::Permission, iroha_data_model::role::model::Role>`
- Schema hash: `f0f39a5e39833ec4f0f39a5e39833ec4`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `Permission` |
| `destination` | `RoleId` |

## `iroha_data_model::isi::transparent::Revoke<iroha_data_model::role::model::RoleId, iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: RoleId, destination: AccountId.

- Rust type: `iroha_data_model::isi::transparent::Revoke<iroha_data_model::role::model::RoleId, iroha_data_model::account::model::Account>`
- Schema hash: `0dd7682c101736a70dd7682c101736a7`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `RoleId` |
| `destination` | `AccountId` |

## `iroha_data_model::isi::transparent::SetAssetKeyValue`

> Schema summary: struct fields: asset: AssetId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetAssetKeyValue`
- Schema hash: `0c83c004f3f0d49e0c83c004f3f0d49e`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::account::model::Account>`

> Schema summary: struct fields: object: AccountId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::account::model::Account>`
- Schema hash: `c98da31b994c99ebc98da31b994c99eb`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AccountId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::asset::definition::model::AssetDefinition>`

> Schema summary: struct fields: object: AssetDefinitionId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::asset::definition::model::AssetDefinition>`
- Schema hash: `d5cb0dd28822d32dd5cb0dd28822d32d`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `AssetDefinitionId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::domain::model::Domain>`

> Schema summary: struct fields: object: DomainId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::domain::model::Domain>`
- Schema hash: `73a94c454ce16ea973a94c454ce16ea9`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `DomainId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::nft::model::Nft>`

> Schema summary: struct fields: object: NftId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::nft::model::Nft>`
- Schema hash: `f9f1ae25ec03f114f9f1ae25ec03f114`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `NftId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::trigger::model::model::Trigger>`

> Schema summary: struct fields: object: TriggerId, key: Name, value: Json.

- Rust type: `iroha_data_model::isi::transparent::SetKeyValue<iroha_data_model::trigger::model::model::Trigger>`
- Schema hash: `9ecea8a7710a9f179ecea8a7710a9f17`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `object` | `TriggerId` |
| `key` | `Name` |
| `value` | `Json` |

## `iroha_data_model::isi::verifying_keys::DeprecateVerifyingKey`

> Schema summary: struct fields: id: VerifyingKeyId.

- Rust type: `iroha_data_model::isi::verifying_keys::DeprecateVerifyingKey`
- Schema hash: `0f5e19a35e92629a0f5e19a35e92629a`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `VerifyingKeyId` |

## `iroha_data_model::isi::verifying_keys::RegisterVerifyingKey`

> Schema summary: struct fields: id: VerifyingKeyId, record: VerifyingKeyRecord.

- Rust type: `iroha_data_model::isi::verifying_keys::RegisterVerifyingKey`
- Schema hash: `f5a10c0b9ecc0d38f5a10c0b9ecc0d38`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `VerifyingKeyId` |
| `record` | `VerifyingKeyRecord` |

## `iroha_data_model::isi::verifying_keys::UpdateVerifyingKey`

> Schema summary: struct fields: id: VerifyingKeyId, record: VerifyingKeyRecord.

- Rust type: `iroha_data_model::isi::verifying_keys::UpdateVerifyingKey`
- Schema hash: `d577b404ffad6d50d577b404ffad6d50`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `id` | `VerifyingKeyId` |
| `record` | `VerifyingKeyRecord` |

## `iroha_data_model::isi::zk::CreateElection`

> Schema summary: struct fields: election_id: String, options: u32, eligible_root: Array<u8, 32>, start_ts: u64, end_ts: u64, vk_ballot: VerifyingKeyId, vk_tally: VerifyingKeyId, domain_tag: String.

- Rust type: `iroha_data_model::isi::zk::CreateElection`
- Schema hash: `6612c94b6f84c9cb6612c94b6f84c9cb`

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
- Schema hash: `9cd931a79ced1cb69cd931a79ced1cb6`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `election_id` | `String` |
| `tally` | `Vec<u64>` |
| `tally_proof` | `ProofAttachment` |

## `iroha_data_model::isi::zk::RegisterZkAsset`

> Schema summary: struct fields: asset: AssetDefinitionId, mode: ZkAssetMode, allow_shield: bool, allow_unshield: bool, vk_transfer: Option<VerifyingKeyId>, vk_unshield: Option<VerifyingKeyId>, vk_shield: Option<VerifyingKeyId>.

- Rust type: `iroha_data_model::isi::zk::RegisterZkAsset`
- Schema hash: `5d14a5ea7a6d1c255d14a5ea7a6d1c25`

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

> Schema summary: struct fields: asset: AssetDefinitionId, from: AccountId, amount: u128, note_commitment: Array<u8, 32>, enc_payload: ConfidentialEncryptedPayload.

- Rust type: `iroha_data_model::isi::zk::Shield`
- Schema hash: `644a69b3e27c574b644a69b3e27c574b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `from` | `AccountId` |
| `amount` | `u128` |
| `note_commitment` | `Array<u8, 32>` |
| `enc_payload` | `ConfidentialEncryptedPayload` |

## `iroha_data_model::isi::zk::SubmitBallot`

> Schema summary: struct fields: election_id: String, ciphertext: Vec<u8>, ballot_proof: ProofAttachment, nullifier: Array<u8, 32>.

- Rust type: `iroha_data_model::isi::zk::SubmitBallot`
- Schema hash: `4319232398af7d414319232398af7d41`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `election_id` | `String` |
| `ciphertext` | `Vec<u8>` |
| `ballot_proof` | `ProofAttachment` |
| `nullifier` | `Array<u8, 32>` |

## `iroha_data_model::isi::zk::Unshield`

> Schema summary: struct fields: asset: AssetDefinitionId, to: AccountId, public_amount: u128, inputs: Vec<Array<u8, 32>>, proof: ProofAttachment, root_hint: Option<Array<u8, 32>>.

- Rust type: `iroha_data_model::isi::zk::Unshield`
- Schema hash: `eb6a8611ac89d632eb6a8611ac89d632`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `to` | `AccountId` |
| `public_amount` | `u128` |
| `inputs` | `Vec<Array<u8, 32>>` |
| `proof` | `ProofAttachment` |
| `root_hint` | `Option<Array<u8, 32>>` |

## `iroha_data_model::isi::zk::VerifyProof`

> Schema summary: struct fields: attachment: ProofAttachment.

- Rust type: `iroha_data_model::isi::zk::VerifyProof`
- Schema hash: `861294eecde91c3b861294eecde91c3b`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `attachment` | `ProofAttachment` |

## `iroha_data_model::isi::zk::ZkTransfer`

> Schema summary: struct fields: asset: AssetDefinitionId, inputs: Vec<Array<u8, 32>>, outputs: Vec<Array<u8, 32>>, proof: ProofAttachment, root_hint: Option<Array<u8, 32>>.

- Rust type: `iroha_data_model::isi::zk::ZkTransfer`
- Schema hash: `a54e2391aea3a8b6a54e2391aea3a8b6`

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
- Schema hash: `c8b4798fe99aba33c8b4798fe99aba33`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `transition_id` | `Hash` |

## `zk::ScheduleConfidentialPolicyTransition`

> Schema summary: struct fields: asset: AssetDefinitionId, new_mode: ConfidentialPolicyMode, effective_height: u64, transition_id: Hash, conversion_window: Option<u64>.

- Rust type: `iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition`
- Schema hash: `836fd710eab04142836fd710eab04142`

**Layout:** `struct`

| Field | Type |
|-------|------|
| `asset` | `AssetDefinitionId` |
| `new_mode` | `ConfidentialPolicyMode` |
| `effective_height` | `u64` |
| `transition_id` | `Hash` |
| `conversion_window` | `Option<u64>` |
