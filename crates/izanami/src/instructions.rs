//! Workload generation utilities for Izanami, covering diverse ISI mixes and trigger flavours.

#[cfg(test)]
use std::sync::Mutex as StdMutex;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use color_eyre::{Result, eyre::eyre};
use iroha_config::parameters::defaults as config_defaults;
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::NewAccount,
    account::ScopedAccountId,
    account::rekey::AccountLabel,
    events::{
        EventFilterBox,
        execute_trigger::ExecuteTriggerEventFilter,
        pipeline::{PipelineEventFilterBox, TransactionEventFilter},
        prelude::{AccountEventFilter, AccountEventSet, DataEventFilter, TimeEventFilter},
        time::{ExecutionTime, Schedule},
    },
    isi::{
        RemoveAssetKeyValue, SetAssetKeyValue,
        settlement::{
            DvpIsi, SettlementId, SettlementInstructionBox, SettlementLeg, SettlementPlan,
        },
        sorafs::{
            ApprovePinManifest, CompleteReplicationOrder, IssueReplicationOrder,
            RegisterPinManifest, RegisterProviderOwner,
        },
        space_directory::{
            ExpireSpaceDirectoryManifest, PublishSpaceDirectoryManifest,
            RevokeSpaceDirectoryManifest,
        },
        staking::{
            ActivatePublicLaneValidator, BondPublicLaneStake, FinalizePublicLaneUnbond,
            RecordPublicLaneRewards, RegisterPublicLaneValidator, SchedulePublicLaneUnbond,
            SlashPublicLaneValidator,
        },
    },
    metadata::Metadata,
    nexus::{
        Allowance, AllowanceWindow, AssetPermissionManifest, CapabilityScope, DataSpaceId, LaneId,
        ManifestEffect, ManifestEntry, ManifestVersion, PublicLaneRewardRole,
        PublicLaneRewardShare, UniversalAccountId,
    },
    parameter::system::SumeragiNposParameters,
    prelude::*,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ChunkerProfileHandle, ManifestDigest, PinPolicy, ReplicationOrderId},
    },
    trigger::{
        Trigger,
        action::{Action, Repeats},
    },
};
use iroha_executor_data_model::permission::{
    account::{CanModifyAccountMetadata, CanRegisterAccount},
    asset::{CanMintAssetWithDefinition, CanModifyAssetMetadataWithDefinition},
    asset_definition::{CanModifyAssetDefinitionMetadata, CanRegisterAssetDefinition},
    domain::{CanModifyDomainMetadata, CanRegisterDomain},
    nexus::CanPublishSpaceDirectoryManifest,
    nft::CanRegisterNft,
    role::CanManageRoles,
    sorafs::{
        CanApproveSorafsPin, CanCompleteSorafsReplicationOrder, CanIssueSorafsReplicationOrder,
        CanRegisterSorafsPin, CanRegisterSorafsProviderOwner,
    },
    trigger::CanRegisterTrigger,
};
use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;
use norito::{
    codec::Encode as NoritoEncode,
    json::{Map as JsonMap, Value as JsonValue},
};
use rand::{Rng, RngCore, rngs::StdRng, seq::IndexedRandom};
use sorafs_manifest::{
    capacity::{
        REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
        ReplicationOrderV1,
    },
    chunker_registry,
};

/// Base seed used for deterministic peer identity derivation in Izanami networks.
pub const IZANAMI_BASE_SEED: &str = "izanami-chaos";
use tokio::sync::Mutex;

use crate::config::WorkloadProfile;
use crate::smart_contracts;

/// Record describing an account and its signing material.
#[derive(Clone, Debug)]
pub struct AccountRecord {
    pub id: ScopedAccountId,
    pub key_pair: KeyPair,
    pub uaid: Option<UniversalAccountId>,
}

/// Transaction plan produced by the workload generator.
#[derive(Clone, Debug)]
pub struct TransactionPlan {
    pub label: &'static str,
    pub instructions: Vec<InstructionBox>,
    pub signer: AccountRecord,
    pub expect_success: bool,
    pub(crate) state_updates: Vec<PlanUpdate>,
}

#[derive(Clone, Debug)]
pub(crate) enum PlanUpdate {
    RegisterTrigger(TriggerId),
    RegisterCallTrigger(TriggerId),
    TrackRepeatableTrigger(TriggerId),
    MintTriggerRepetitions { trigger_id: TriggerId, amount: u32 },
    BurnTriggerRepetitions { trigger_id: TriggerId, amount: u32 },
    ReleaseTriggerRepetitionsReservation { trigger_id: TriggerId, amount: u32 },
    SetTriggerMetadata { trigger_id: TriggerId, key: Name },
    ClearTriggerMetadata(TriggerId),
}

impl PlanUpdate {
    fn apply(&self, state: &mut ChaosState, succeeded: bool) {
        match self {
            PlanUpdate::RegisterTrigger(trigger_id) if succeeded => {
                if !state.registered_triggers.contains(trigger_id) {
                    state.registered_triggers.push(trigger_id.clone());
                }
            }
            PlanUpdate::RegisterCallTrigger(trigger_id) if succeeded => {
                if !state.call_triggers.contains(trigger_id) {
                    state.call_triggers.push(trigger_id.clone());
                }
            }
            PlanUpdate::TrackRepeatableTrigger(trigger_id) if succeeded => {
                state.track_repeatable_trigger(trigger_id.clone());
            }
            PlanUpdate::MintTriggerRepetitions { trigger_id, amount } if succeeded => {
                state.track_repeatable_trigger(trigger_id.clone());
                let entry = state
                    .trigger_repetitions
                    .entry(trigger_id.clone())
                    .or_default();
                *entry = entry.saturating_add(*amount);
            }
            PlanUpdate::BurnTriggerRepetitions { trigger_id, amount } if succeeded => {
                state.release_trigger_repetitions_reservation(trigger_id, *amount);
                let Some(entry) = state.trigger_repetitions.get_mut(trigger_id) else {
                    return;
                };
                *entry = entry.saturating_sub(*amount);
                if *entry == 0 {
                    state.trigger_repetitions.remove(trigger_id);
                    state.repeatable_triggers.retain(|id| id != trigger_id);
                }
            }
            PlanUpdate::ReleaseTriggerRepetitionsReservation { trigger_id, amount }
                if !succeeded =>
            {
                state.release_trigger_repetitions_reservation(trigger_id, *amount);
            }
            PlanUpdate::SetTriggerMetadata { trigger_id, key } if succeeded => {
                state
                    .trigger_metadata
                    .entry(trigger_id.clone())
                    .or_default()
                    .insert(key.clone());
            }
            PlanUpdate::ClearTriggerMetadata(trigger_id) if succeeded => {
                state.trigger_metadata.remove(trigger_id);
            }
            _ => {}
        }
    }
}

impl TransactionPlan {
    fn apply_updates(&self, state: &mut ChaosState, succeeded: bool) {
        for update in &self.state_updates {
            update.apply(state, succeeded);
        }
    }

    pub fn burn_trigger_repetitions(&self) -> Option<(TriggerId, u32)> {
        self.instructions.iter().find_map(|instruction| {
            let burn = instruction.as_any().downcast_ref::<BurnBox>()?;
            match burn {
                BurnBox::TriggerRepetitions(burn) => Some((burn.destination.clone(), burn.object)),
                BurnBox::Asset(_) => None,
            }
        })
    }

    pub fn mint_trigger_repetitions(&self) -> Option<(TriggerId, u32)> {
        self.instructions.iter().find_map(|instruction| {
            let mint = instruction.as_any().downcast_ref::<MintBox>()?;
            match mint {
                MintBox::TriggerRepetitions(mint) => Some((mint.destination.clone(), mint.object)),
                MintBox::Asset(_) => None,
            }
        })
    }
}

fn json_pair<K, V>(key: K, value: V) -> JsonValue
where
    K: Into<String>,
    V: Into<JsonValue>,
{
    let mut map = JsonMap::new();
    map.insert(key.into(), value.into());
    JsonValue::Object(map)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn account_from_record(record: &AccountRecord) -> NewAccount {
    let builder = Account::new(record.id.clone());
    if let Some(uaid) = record.uaid {
        builder.with_uaid(Some(uaid))
    } else {
        builder
    }
}

fn peer_keypair(index: usize) -> KeyPair {
    let seed = format!("{IZANAMI_BASE_SEED}-peer-{index}");
    let mut seed_bytes = seed.into_bytes();
    seed_bytes.extend_from_slice(b":bls");
    KeyPair::from_seed(seed_bytes, Algorithm::BlsNormal)
}

/// Prepared chaos state with pre-built genesis instructions.
#[derive(Debug)]
pub struct PreparedChaos {
    pub state: ChaosState,
    pub genesis: Vec<Vec<InstructionBox>>,
    pub recipes: Vec<RecipeKind>,
}

/// Build post-topology NPoS bootstrap instructions for Izanami peers.
pub fn npos_post_topology_instructions(peer_count: usize) -> Vec<InstructionBox> {
    let effective_peers = peer_count.max(1);
    let nexus_domain: DomainId = "nexus".parse().expect("nexus domain");
    let stake_amount: Numeric = SumeragiNposParameters::default().min_self_bond().into();
    let mut instructions = Vec::new();
    for index in 0..effective_peers {
        let key_pair = peer_keypair(index);
        let validator_id =
            ScopedAccountId::new(nexus_domain.clone(), key_pair.public_key().clone());
        instructions.push(InstructionBox::from(RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id.clone(),
            stake_account: validator_id.clone(),
            initial_stake: stake_amount.clone(),
            metadata: Metadata::default(),
        }));
        instructions.push(InstructionBox::from(ActivatePublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id,
        }));
    }
    instructions
}

/// Create the baseline chaos state and its associated genesis block.
pub fn prepare_state(
    account_count: usize,
    peer_count: Option<usize>,
    nexus: Option<&crate::config::NexusProfile>,
    workload_profile: WorkloadProfile,
    allow_contract_deploy_in_stable: bool,
) -> Result<PreparedChaos> {
    let effective_accounts = account_count.max(3);
    let base_domain: DomainId = "chaosnet"
        .parse()
        .map_err(|_| eyre!("invalid base domain"))?;
    let domain_name = base_domain.name.to_string();

    let treasury_key = KeyPair::random();
    let treasury_id = ScopedAccountId::new(base_domain.clone(), treasury_key.public_key().clone());
    let treasury = AccountRecord {
        id: treasury_id,
        key_pair: treasury_key,
        uaid: None,
    };

    let mut users = Vec::with_capacity(effective_accounts);
    for _ in 0..effective_accounts {
        let key = KeyPair::random();
        let account_id = ScopedAccountId::new(base_domain.clone(), key.public_key().clone());
        users.push(AccountRecord {
            id: account_id,
            key_pair: key,
            uaid: None,
        });
    }

    let asset_numeric_id: AssetDefinitionId = format!("chaos_coin#{domain_name}")
        .parse()
        .map_err(|_| eyre!("failed to form numeric asset id"))?;
    let asset_nft_id: AssetDefinitionId = format!("chaos_collectible#{domain_name}")
        .parse()
        .map_err(|_| eyre!("failed to form nft asset id"))?;

    let dataspaces: Vec<DataSpaceId> = nexus
        .map(|profile| {
            let ids: Vec<DataSpaceId> = profile
                .dataspace_catalog
                .entries()
                .iter()
                .map(|entry| entry.id)
                .collect();
            if ids.is_empty() {
                vec![DataSpaceId::GLOBAL]
            } else {
                ids
            }
        })
        .unwrap_or_else(|| vec![DataSpaceId::GLOBAL]);
    let lanes: Vec<LaneId> = nexus
        .map(|profile| {
            let ids: Vec<LaneId> = profile
                .lane_catalog
                .lanes()
                .iter()
                .map(|lane| lane.id)
                .collect();
            if ids.is_empty() {
                vec![LaneId::SINGLE]
            } else {
                ids
            }
        })
        .unwrap_or_else(|| vec![LaneId::SINGLE]);

    let sorafs_replication = if nexus.is_some() {
        let manifest_digest = ManifestDigest::new(*Hash::new(b"izanami-sorafs-manifest").as_ref());
        let descriptor = chunker_registry::default_descriptor();
        let chunker = ChunkerProfileHandle {
            profile_id: descriptor.id.0,
            namespace: descriptor.namespace.to_string(),
            name: descriptor.name.to_string(),
            semver: descriptor.semver.to_string(),
            multihash_code: descriptor.multihash_code,
        };
        let provider_id = ProviderId::new(*Hash::new(b"izanami-sorafs-provider").as_ref());
        Some(SorafsReplicationSeed {
            manifest_digest,
            chunker,
            provider_id,
        })
    } else {
        None
    };

    let effective_peers = peer_count.unwrap_or(account_count.max(1)).max(1);
    let mut nexus_genesis = Vec::new();
    let mut nexus_staking = None;
    let mut npos_bootstrap_stake = None;
    if nexus.is_some() {
        let nexus_domain: DomainId = "nexus"
            .parse()
            .map_err(|_| eyre!("failed to parse nexus domain id"))?;
        let ivm_domain: DomainId = "ivm"
            .parse()
            .map_err(|_| eyre!("failed to parse ivm domain id"))?;
        let gas_account_id = ScopedAccountId::new(
            ivm_domain.clone(),
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
        );
        let gas_label: Name = "gas"
            .parse()
            .map_err(|_| eyre!("failed to parse gas account label"))?;
        let gas_account = Account::new(gas_account_id.clone())
            .with_label(Some(AccountLabel::new(ivm_domain.clone(), gas_label)));

        let stake_asset: AssetDefinitionId = config_defaults::nexus::staking::stake_asset_id()
            .parse()
            .map_err(|_| eyre!("failed to parse nexus stake asset id"))?;
        let fee_asset: AssetDefinitionId = config_defaults::nexus::fees::FEE_ASSET_ID
            .parse()
            .map_err(|_| eyre!("failed to parse nexus fee asset id"))?;
        let stake_amount_value = SumeragiNposParameters::default().min_self_bond();
        let stake_amount: Numeric = stake_amount_value.into();
        npos_bootstrap_stake = Some(stake_amount_value);

        nexus_genesis.push(InstructionBox::from(Register::domain(Domain::new(
            nexus_domain.clone(),
        ))));
        nexus_genesis.push(InstructionBox::from(Register::domain(Domain::new(
            ivm_domain.clone(),
        ))));
        nexus_genesis.push(InstructionBox::from(Register::account(gas_account)));
        nexus_genesis.push(InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(stake_asset.clone()),
        )));
        if fee_asset != stake_asset {
            nexus_genesis.push(InstructionBox::from(Register::asset_definition(
                AssetDefinition::numeric(fee_asset.clone()),
            )));
        }

        let mut validator_accounts = Vec::new();
        for index in 0..effective_peers {
            let key_pair = peer_keypair(index);
            let account_id =
                ScopedAccountId::new(nexus_domain.clone(), key_pair.public_key().clone());
            validator_accounts.push(AccountRecord {
                id: account_id.clone(),
                key_pair,
                uaid: None,
            });
            nexus_genesis.push(InstructionBox::from(Register::account(Account::new(
                account_id,
            ))));
        }

        for validator in &validator_accounts {
            nexus_genesis.push(InstructionBox::from(Mint::asset_numeric(
                stake_amount.clone(),
                AssetId::new(stake_asset.clone(), validator.id.clone()),
            )));
        }

        nexus_genesis.push(InstructionBox::from(Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: stake_asset.clone(),
            },
            treasury.id.clone(),
        )));
        if fee_asset != stake_asset {
            nexus_genesis.push(InstructionBox::from(Grant::account_permission(
                CanMintAssetWithDefinition {
                    asset_definition: fee_asset.clone(),
                },
                treasury.id.clone(),
            )));
        }

        nexus_staking = Some(NexusStakingSetup {
            stake_asset,
            fee_asset,
            fee_sink: gas_account_id.clone(),
            stake_escrow: gas_account_id.clone(),
            slash_sink: gas_account_id,
            validator_accounts,
        });
    }

    let mut genesis_tx = Vec::new();
    genesis_tx.push(InstructionBox::from(Register::domain(Domain::new(
        base_domain.clone(),
    ))));
    genesis_tx.push(InstructionBox::from(Register::asset_definition(
        AssetDefinition::numeric(asset_numeric_id.clone()),
    )));
    genesis_tx.push(InstructionBox::from(Register::asset_definition(
        AssetDefinition::numeric(asset_nft_id.clone()).mintable_once(),
    )));

    genesis_tx.push(InstructionBox::from(Register::account(
        account_from_record(&treasury),
    )));
    for account in &users {
        genesis_tx.push(InstructionBox::from(Register::account(
            account_from_record(account),
        )));
    }
    genesis_tx.extend(nexus_genesis);
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanRegisterDomain,
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanModifyDomainMetadata {
            domain: base_domain.clone(),
        },
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanRegisterAccount {
            domain: base_domain.clone(),
        },
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanRegisterAssetDefinition {
            domain: base_domain.clone(),
        },
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanMintAssetWithDefinition {
            asset_definition: asset_numeric_id.clone(),
        },
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanRegisterNft {
            domain: base_domain.clone(),
        },
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanModifyAssetDefinitionMetadata {
            asset_definition: asset_numeric_id.clone(),
        },
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanModifyAssetDefinitionMetadata {
            asset_definition: asset_nft_id.clone(),
        },
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanModifyAssetMetadataWithDefinition {
            asset_definition: asset_numeric_id.clone(),
        },
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanManageRoles,
        treasury.id.clone(),
    )));
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanRegisterTrigger {
            authority: treasury.id.clone(),
        },
        treasury.id.clone(),
    )));
    for dataspace in &dataspaces {
        genesis_tx.push(InstructionBox::from(Grant::account_permission(
            CanPublishSpaceDirectoryManifest {
                dataspace: *dataspace,
            },
            treasury.id.clone(),
        )));
    }
    if sorafs_replication.is_some() {
        genesis_tx.push(InstructionBox::from(Grant::account_permission(
            CanRegisterSorafsPin,
            treasury.id.clone(),
        )));
        genesis_tx.push(InstructionBox::from(Grant::account_permission(
            CanApproveSorafsPin,
            treasury.id.clone(),
        )));
        genesis_tx.push(InstructionBox::from(Grant::account_permission(
            CanRegisterSorafsProviderOwner,
            treasury.id.clone(),
        )));
        genesis_tx.push(InstructionBox::from(Grant::account_permission(
            CanIssueSorafsReplicationOrder,
            treasury.id.clone(),
        )));
        genesis_tx.push(InstructionBox::from(Grant::account_permission(
            CanCompleteSorafsReplicationOrder,
            treasury.id.clone(),
        )));
    }
    let initial_float: Numeric = 1_000_000_000_u64.into();
    let treasury_asset_id = AssetId::new(asset_numeric_id.clone(), treasury.id.clone());
    genesis_tx.push(InstructionBox::from(Mint::asset_numeric(
        initial_float,
        treasury_asset_id.clone(),
    )));

    let mut state = ChaosState::new(
        base_domain.clone(),
        treasury,
        users,
        asset_numeric_id,
        asset_nft_id,
        dataspaces,
        lanes,
        sorafs_replication,
        nexus_staking,
    );
    state.asset_instances.insert(treasury_asset_id);
    if let (Some(stake_amount), Some(setup)) = (npos_bootstrap_stake, state.nexus_staking.as_ref())
    {
        let lane = LaneId::SINGLE;
        let validator_ids: Vec<ScopedAccountId> = setup
            .validator_accounts
            .iter()
            .map(|record| record.id.clone())
            .collect();
        if !validator_ids.is_empty() {
            for validator_id in &validator_ids {
                state.add_public_lane_stake_share(lane, validator_id, validator_id, stake_amount);
            }
            state
                .public_lane_validators
                .insert(lane, validator_ids.into_iter().collect());
        }
    }
    let mut recipes = match workload_profile {
        WorkloadProfile::Stable => BASE_RECIPES_STABLE.to_vec(),
        WorkloadProfile::Chaos => BASE_RECIPES_CHAOS.to_vec(),
    };
    if matches!(workload_profile, WorkloadProfile::Stable) && !allow_contract_deploy_in_stable {
        recipes.retain(|kind| {
            !matches!(
                kind,
                RecipeKind::DeployIvmContract | RecipeKind::DeployKotodamaContract
            )
        });
    }
    if nexus.is_some() {
        let extra = match workload_profile {
            WorkloadProfile::Stable => NEXUS_RECIPES_STABLE,
            WorkloadProfile::Chaos => NEXUS_RECIPES_CHAOS,
        };
        recipes.extend_from_slice(extra);
    }
    Ok(PreparedChaos {
        state,
        genesis: vec![genesis_tx],
        recipes,
    })
}

/// Workload engine that produces stochastic transaction plans.
#[derive(Debug)]
pub struct WorkloadEngine {
    state: Mutex<ChaosState>,
    recipes: Vec<RecipeKind>,
    #[cfg(test)]
    recipe_override: StdMutex<Option<RecipeKind>>,
}

impl WorkloadEngine {
    pub fn new(state: ChaosState, recipes: Vec<RecipeKind>) -> Self {
        Self {
            state: Mutex::new(state),
            recipes,
            #[cfg(test)]
            recipe_override: StdMutex::new(None),
        }
    }

    pub async fn next_plan(&self, rng: &mut StdRng) -> Result<TransactionPlan> {
        #[cfg(test)]
        if let Some(kind) = {
            let guard = self
                .recipe_override
                .lock()
                .expect("override mutex poisoned");
            *guard
        } {
            let mut guard = self.state.lock().await;
            return guard.produce_plan(kind, rng);
        }

        let kind = *self
            .recipes
            .choose(rng)
            .ok_or_else(|| eyre!("no recipes configured"))?;
        let mut guard = self.state.lock().await;
        guard.produce_plan(kind, rng)
    }

    pub async fn record_result(&self, plan: &TransactionPlan, succeeded: bool) {
        if plan.state_updates.is_empty() {
            return;
        }
        let mut guard = self.state.lock().await;
        plan.apply_updates(&mut guard, succeeded);
    }

    pub async fn sync_trigger_repetitions(&self, trigger_id: &TriggerId, repeats: Option<u32>) {
        let mut guard = self.state.lock().await;
        match repeats {
            Some(count) if count > 0 => {
                guard.trigger_repetitions.insert(trigger_id.clone(), count);
                if !guard.repeatable_triggers.contains(trigger_id) {
                    guard.repeatable_triggers.push(trigger_id.clone());
                }
            }
            _ => {
                guard.trigger_repetitions.remove(trigger_id);
                guard.repeatable_triggers.retain(|id| id != trigger_id);
            }
        }
    }

    #[cfg(test)]
    fn set_recipe_override(&self, recipe: Option<RecipeKind>) {
        *self
            .recipe_override
            .lock()
            .expect("override mutex poisoned") = recipe;
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RecipeKind {
    RegisterDomain,
    DuplicateDomain,
    RegisterAccount,
    DuplicateAccount,
    RegisterUaidAccount,
    RegisterAssetDefinition,
    UnregisterAssetDefinition,
    MintAsset,
    TransferAsset,
    BurnAsset,
    SetAccountKeyValue,
    RemoveAccountKeyValue,
    SetDomainKeyValue,
    RemoveDomainKeyValue,
    SetAssetDefinitionKeyValue,
    RemoveAssetDefinitionKeyValue,
    SetAssetInstanceKeyValue,
    RemoveAssetInstanceKeyValue,
    RegisterNft,
    TransferNft,
    RegisterRole,
    GrantRole,
    RevokeRole,
    RegisterTimeTrigger,
    RegisterDataTrigger,
    RegisterPipelineTrigger,
    SetTriggerKeyValue,
    RemoveTriggerKeyValue,
    MintTriggerRepetitions,
    BurnTriggerRepetitions,
    ExecuteTrigger,
    ExecuteMissingTrigger,
    DeployIvmContract,
    DeployKotodamaContract,
    PublishSpaceDirectoryManifest,
    RevokeSpaceDirectoryManifest,
    ExpireSpaceDirectoryManifest,
    RegisterPublicLaneValidator,
    BondPublicLaneStake,
    SchedulePublicLaneUnbond,
    FinalizePublicLaneUnbond,
    SlashPublicLaneValidator,
    RecordPublicLaneRewards,
    DvpSettlement,
    IssueReplicationOrder,
    CompleteReplicationOrder,
}

// Stable runs favor deterministic recipes; contract deployment can be enabled via
// `allow_contract_deploy_in_stable`.
const BASE_RECIPES_STABLE: &[RecipeKind] = &[
    RecipeKind::RegisterDomain,
    RecipeKind::RegisterNft,
    RecipeKind::MintAsset,
    RecipeKind::TransferAsset,
    RecipeKind::TransferNft,
    RecipeKind::BurnAsset,
    RecipeKind::SetAccountKeyValue,
    RecipeKind::RemoveAccountKeyValue,
    RecipeKind::SetDomainKeyValue,
    RecipeKind::RemoveDomainKeyValue,
    RecipeKind::SetAssetDefinitionKeyValue,
    RecipeKind::RemoveAssetDefinitionKeyValue,
    RecipeKind::SetAssetInstanceKeyValue,
    RecipeKind::RemoveAssetInstanceKeyValue,
    RecipeKind::RegisterRole,
    RecipeKind::MintTriggerRepetitions,
    RecipeKind::BurnTriggerRepetitions,
    RecipeKind::DeployIvmContract,
    RecipeKind::DeployKotodamaContract,
];

const BASE_RECIPES_CHAOS: &[RecipeKind] = &[
    RecipeKind::RegisterDomain,
    RecipeKind::RegisterAssetDefinition,
    RecipeKind::RegisterAccount,
    RecipeKind::RegisterUaidAccount,
    RecipeKind::RegisterNft,
    RecipeKind::MintAsset,
    RecipeKind::TransferAsset,
    RecipeKind::TransferNft,
    RecipeKind::BurnAsset,
    RecipeKind::SetAccountKeyValue,
    RecipeKind::RemoveAccountKeyValue,
    RecipeKind::SetDomainKeyValue,
    RecipeKind::RemoveDomainKeyValue,
    RecipeKind::SetAssetDefinitionKeyValue,
    RecipeKind::RemoveAssetDefinitionKeyValue,
    RecipeKind::SetAssetInstanceKeyValue,
    RecipeKind::RemoveAssetInstanceKeyValue,
    RecipeKind::RegisterRole,
    RecipeKind::GrantRole,
    RecipeKind::RevokeRole,
    RecipeKind::RegisterTimeTrigger,
    RecipeKind::RegisterDataTrigger,
    RecipeKind::RegisterPipelineTrigger,
    RecipeKind::SetTriggerKeyValue,
    RecipeKind::RemoveTriggerKeyValue,
    RecipeKind::MintTriggerRepetitions,
    RecipeKind::BurnTriggerRepetitions,
    RecipeKind::ExecuteTrigger,
    RecipeKind::ExecuteMissingTrigger,
    RecipeKind::DuplicateDomain,
    RecipeKind::DuplicateAccount,
    RecipeKind::UnregisterAssetDefinition,
    RecipeKind::DeployIvmContract,
    RecipeKind::DeployKotodamaContract,
    RecipeKind::PublishSpaceDirectoryManifest,
    RecipeKind::RevokeSpaceDirectoryManifest,
    RecipeKind::ExpireSpaceDirectoryManifest,
];

const NEXUS_RECIPES_STABLE: &[RecipeKind] = &[];

const NEXUS_RECIPES_CHAOS: &[RecipeKind] = &[
    RecipeKind::RegisterPublicLaneValidator,
    RecipeKind::BondPublicLaneStake,
    RecipeKind::SchedulePublicLaneUnbond,
    RecipeKind::FinalizePublicLaneUnbond,
    RecipeKind::SlashPublicLaneValidator,
    RecipeKind::RecordPublicLaneRewards,
    RecipeKind::DvpSettlement,
    RecipeKind::IssueReplicationOrder,
    RecipeKind::CompleteReplicationOrder,
];

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct NexusStakingSetup {
    stake_asset: AssetDefinitionId,
    fee_asset: AssetDefinitionId,
    fee_sink: ScopedAccountId,
    stake_escrow: ScopedAccountId,
    slash_sink: ScopedAccountId,
    validator_accounts: Vec<AccountRecord>,
}

#[derive(Debug, Clone)]
pub struct ChaosState {
    base_domain: DomainId,
    treasury: AccountRecord,
    users: Vec<AccountRecord>,
    uaid_accounts: HashMap<UniversalAccountId, AccountRecord>,
    asset_numeric: AssetDefinitionId,
    dataspaces: Vec<DataSpaceId>,
    lanes: Vec<LaneId>,
    created_domains: HashSet<DomainId>,
    registered_roles: Vec<RoleId>,
    role_memberships: HashMap<RoleId, HashSet<ScopedAccountId>>,
    registered_triggers: Vec<TriggerId>,
    repeatable_triggers: Vec<TriggerId>,
    call_triggers: Vec<TriggerId>,
    asset_definitions: HashSet<AssetDefinitionId>,
    asset_definitions_unclaimed: HashSet<AssetDefinitionId>,
    asset_instances: HashSet<AssetId>,
    nft_holdings: HashMap<NftId, ScopedAccountId>,
    account_metadata: HashMap<ScopedAccountId, HashSet<Name>>,
    domain_metadata: HashMap<DomainId, HashSet<Name>>,
    asset_definition_metadata: HashMap<AssetDefinitionId, HashSet<Name>>,
    asset_metadata: HashMap<AssetId, HashSet<Name>>,
    trigger_metadata: HashMap<TriggerId, HashSet<Name>>,
    trigger_repetitions: HashMap<TriggerId, u32>,
    pending_trigger_repetitions: HashMap<TriggerId, u32>,
    space_directory_manifests: HashMap<UniversalAccountId, HashSet<DataSpaceId>>,
    public_lane_validators: HashMap<LaneId, HashSet<ScopedAccountId>>,
    public_lane_stakes: HashMap<(LaneId, ScopedAccountId, ScopedAccountId), u64>,
    pending_unbonds: Vec<PendingUnbond>,
    pending_replication_orders: Vec<ReplicationOrderId>,
    sorafs_replication: Option<SorafsReplicationSeed>,
    sorafs_replication_ready: bool,
    nexus_staking: Option<NexusStakingSetup>,
    counters: ChaosCounters,
}

#[derive(Clone, Debug)]
struct PendingUnbond {
    lane: LaneId,
    validator: ScopedAccountId,
    staker: ScopedAccountId,
    request_id: Hash,
}

#[derive(Clone, Debug)]
struct SorafsReplicationSeed {
    manifest_digest: ManifestDigest,
    chunker: ChunkerProfileHandle,
    provider_id: ProviderId,
}

#[derive(Debug, Default, Clone)]
struct ChaosCounters {
    domain: u64,
    account: u64,
    uaid: u64,
    trigger: u64,
    role: u64,
    asset_definition: u64,
    nft: u64,
    metadata: u64,
    invalid: u64,
    staking: u64,
    settlement: u64,
    replication: u64,
}

impl ChaosState {
    pub fn base_domain(&self) -> &DomainId {
        &self.base_domain
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        base_domain: DomainId,
        treasury: AccountRecord,
        users: Vec<AccountRecord>,
        asset_numeric: AssetDefinitionId,
        asset_nft: AssetDefinitionId,
        dataspaces: Vec<DataSpaceId>,
        lanes: Vec<LaneId>,
        sorafs_replication: Option<SorafsReplicationSeed>,
        nexus_staking: Option<NexusStakingSetup>,
    ) -> Self {
        let mut asset_definitions = HashSet::new();
        asset_definitions.insert(asset_numeric.clone());
        asset_definitions.insert(asset_nft);
        let mut uaid_accounts = HashMap::new();
        if let Some(uaid) = treasury.uaid {
            uaid_accounts.insert(uaid, treasury.clone());
        }
        for account in &users {
            if let Some(uaid) = account.uaid {
                uaid_accounts.insert(uaid, account.clone());
            }
        }
        Self {
            base_domain,
            treasury,
            users,
            uaid_accounts,
            asset_numeric,
            dataspaces,
            lanes,
            created_domains: HashSet::new(),
            registered_roles: Vec::new(),
            role_memberships: HashMap::new(),
            registered_triggers: Vec::new(),
            repeatable_triggers: Vec::new(),
            call_triggers: Vec::new(),
            asset_definitions,
            asset_definitions_unclaimed: HashSet::new(),
            asset_instances: HashSet::new(),
            nft_holdings: HashMap::new(),
            account_metadata: HashMap::new(),
            domain_metadata: HashMap::new(),
            asset_definition_metadata: HashMap::new(),
            asset_metadata: HashMap::new(),
            trigger_metadata: HashMap::new(),
            trigger_repetitions: HashMap::new(),
            pending_trigger_repetitions: HashMap::new(),
            space_directory_manifests: HashMap::new(),
            public_lane_validators: HashMap::new(),
            public_lane_stakes: HashMap::new(),
            pending_unbonds: Vec::new(),
            pending_replication_orders: Vec::new(),
            sorafs_replication,
            sorafs_replication_ready: false,
            nexus_staking,
            counters: ChaosCounters::default(),
        }
    }

    fn track_account(&mut self, record: AccountRecord) {
        if let Some(uaid) = record.uaid {
            self.uaid_accounts.insert(uaid, record.clone());
        }
        self.users.push(record);
    }

    fn nexus_staking_expect_success(&self) -> bool {
        self.nexus_staking.is_some()
    }

    fn allocate_uaid_record(&mut self) -> AccountRecord {
        let _ = self.bump_account();
        let key = KeyPair::random();
        let uaid = self.next_uaid();
        let account_id = ScopedAccountId::new(self.base_domain.clone(), key.public_key().clone());
        AccountRecord {
            id: account_id,
            key_pair: key,
            uaid: Some(uaid),
        }
    }

    fn next_uaid(&mut self) -> UniversalAccountId {
        let suffix = self.bump_uaid();
        let seed = format!("izanami-uaid-{suffix}");
        UniversalAccountId::from_hash(Hash::new(seed.as_bytes()))
    }

    fn pick_uaid_without_manifest(
        &self,
        dataspace: DataSpaceId,
        rng: &mut StdRng,
    ) -> Option<UniversalAccountId> {
        let eligible: Vec<_> = self
            .uaid_accounts
            .keys()
            .filter(|uaid| {
                !self
                    .space_directory_manifests
                    .get(*uaid)
                    .is_some_and(|spaces| spaces.contains(&dataspace))
            })
            .cloned()
            .collect();
        eligible.choose(rng).cloned()
    }

    fn pick_manifest_for_dataspace(
        &self,
        dataspace: DataSpaceId,
        rng: &mut StdRng,
    ) -> Option<UniversalAccountId> {
        let candidates: Vec<_> = self
            .space_directory_manifests
            .iter()
            .filter(|(_, spaces)| spaces.contains(&dataspace))
            .map(|(uaid, _)| *uaid)
            .collect();
        candidates.choose(rng).copied()
    }

    fn random_dataspace(&self, rng: &mut StdRng) -> DataSpaceId {
        *self.dataspaces.choose(rng).unwrap_or(&DataSpaceId::GLOBAL)
    }

    fn random_lane(&self, rng: &mut StdRng) -> LaneId {
        *self.lanes.choose(rng).unwrap_or(&LaneId::SINGLE)
    }

    fn pick_registered_validator(&self, rng: &mut StdRng) -> Option<(LaneId, AccountRecord)> {
        let mut candidates = Vec::new();
        for (lane, accounts) in &self.public_lane_validators {
            for account_id in accounts {
                if let Some(record) = self.account_by_id(account_id) {
                    candidates.push((*lane, record));
                }
            }
        }
        candidates.choose(rng).cloned()
    }

    fn mark_manifest_removed(&mut self, uaid: UniversalAccountId, dataspace: DataSpaceId) {
        if let Some(spaces) = self.space_directory_manifests.get_mut(&uaid) {
            spaces.remove(&dataspace);
            if spaces.is_empty() {
                self.space_directory_manifests.remove(&uaid);
            }
        }
    }

    fn produce_plan(&mut self, kind: RecipeKind, rng: &mut StdRng) -> Result<TransactionPlan> {
        match kind {
            RecipeKind::RegisterDomain => self.plan_register_domain(rng),
            RecipeKind::DuplicateDomain => Ok(self.plan_duplicate_domain()),
            RecipeKind::RegisterAccount => Ok(self.plan_register_account()),
            RecipeKind::DuplicateAccount => self.plan_duplicate_account(rng),
            RecipeKind::RegisterUaidAccount => Ok(self.plan_register_uaid_account()),
            RecipeKind::RegisterAssetDefinition => self.plan_register_asset_definition(),
            RecipeKind::UnregisterAssetDefinition => Ok(self.plan_unregister_asset_definition(rng)),
            RecipeKind::RegisterNft => self.plan_register_nft(rng),
            RecipeKind::TransferNft => self.plan_transfer_nft(rng),
            RecipeKind::MintAsset => self.plan_mint_asset(rng),
            RecipeKind::TransferAsset => self.plan_transfer_asset(rng),
            RecipeKind::BurnAsset => Ok(self.plan_burn_asset(rng)),
            RecipeKind::SetAccountKeyValue => self.plan_set_key(rng),
            RecipeKind::RemoveAccountKeyValue => self.plan_remove_key(rng),
            RecipeKind::SetDomainKeyValue => self.plan_set_domain_key(),
            RecipeKind::RemoveDomainKeyValue => self.plan_remove_domain_key(),
            RecipeKind::SetAssetDefinitionKeyValue => self.plan_set_asset_definition_key(rng),
            RecipeKind::RemoveAssetDefinitionKeyValue => self.plan_remove_asset_definition_key(rng),
            RecipeKind::SetAssetInstanceKeyValue => self.plan_set_asset_metadata(rng),
            RecipeKind::RemoveAssetInstanceKeyValue => self.plan_remove_asset_metadata(rng),
            RecipeKind::RegisterRole => self.plan_register_role(),
            RecipeKind::GrantRole => self.plan_grant_role(rng),
            RecipeKind::RevokeRole => self.plan_revoke_role(rng),
            RecipeKind::RegisterTimeTrigger => self.plan_time_trigger(rng),
            RecipeKind::RegisterDataTrigger => self.plan_data_trigger(rng),
            RecipeKind::RegisterPipelineTrigger => self.plan_pipeline_trigger(rng),
            RecipeKind::SetTriggerKeyValue => self.plan_set_trigger_key(rng),
            RecipeKind::RemoveTriggerKeyValue => self.plan_remove_trigger_key(rng),
            RecipeKind::MintTriggerRepetitions => self.plan_mint_trigger_repetitions(rng),
            RecipeKind::BurnTriggerRepetitions => self.plan_burn_trigger_repetitions(rng),
            RecipeKind::ExecuteTrigger => self.plan_execute_trigger(rng),
            RecipeKind::ExecuteMissingTrigger => self.plan_execute_missing_trigger(),
            RecipeKind::DeployIvmContract => self.plan_deploy_ivm(rng),
            RecipeKind::DeployKotodamaContract => self.plan_deploy_kotodama(rng),
            RecipeKind::PublishSpaceDirectoryManifest => self.plan_publish_space_manifest(rng),
            RecipeKind::RevokeSpaceDirectoryManifest => self.plan_revoke_space_manifest(rng),
            RecipeKind::ExpireSpaceDirectoryManifest => self.plan_expire_space_manifest(rng),
            RecipeKind::RegisterPublicLaneValidator => self.plan_register_public_validator(rng),
            RecipeKind::BondPublicLaneStake => self.plan_bond_public_stake(rng),
            RecipeKind::SchedulePublicLaneUnbond => self.plan_schedule_public_unbond(rng),
            RecipeKind::FinalizePublicLaneUnbond => self.plan_finalize_public_unbond(rng),
            RecipeKind::SlashPublicLaneValidator => self.plan_slash_public_validator(rng),
            RecipeKind::RecordPublicLaneRewards => self.plan_record_public_rewards(rng),
            RecipeKind::DvpSettlement => self.plan_dvp_settlement(rng),
            RecipeKind::IssueReplicationOrder => self.plan_issue_replication_order(rng),
            RecipeKind::CompleteReplicationOrder => self.plan_complete_replication_order(rng),
        }
    }

    fn plan_register_domain(&mut self, _rng: &mut StdRng) -> Result<TransactionPlan> {
        let suffix = self.bump_domain();
        let domain_id: DomainId = format!("chaos_child_{suffix}")
            .parse()
            .map_err(|_| eyre!("failed to build new domain id"))?;
        self.created_domains.insert(domain_id.clone());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "register_domain",
            instructions: vec![InstructionBox::from(Register::domain(Domain::new(
                domain_id,
            )))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_duplicate_domain(&mut self) -> TransactionPlan {
        let target = self
            .created_domains
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| self.base_domain.clone());
        TransactionPlan {
            state_updates: Vec::new(),
            label: "duplicate_domain",
            instructions: vec![InstructionBox::from(Register::domain(Domain::new(target)))],
            signer: self.treasury.clone(),
            expect_success: false,
        }
    }

    fn plan_register_account(&mut self) -> TransactionPlan {
        let _suffix = self.bump_account();
        let key = KeyPair::random();
        let account_id = ScopedAccountId::new(self.base_domain.clone(), key.public_key().clone());
        let record = AccountRecord {
            id: account_id.clone(),
            key_pair: key,
            uaid: None,
        };
        self.track_account(record.clone());
        TransactionPlan {
            state_updates: Vec::new(),
            label: "register_account",
            instructions: vec![InstructionBox::from(Register::account(Account::new(
                account_id,
            )))],
            signer: self.treasury.clone(),
            expect_success: true,
        }
    }

    fn plan_duplicate_account(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let candidate = self.random_user(rng)?.clone();
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "duplicate_account",
            instructions: vec![InstructionBox::from(Register::account(Account::new(
                candidate.id.clone(),
            )))],
            signer: self.treasury.clone(),
            expect_success: false,
        })
    }

    fn plan_register_uaid_account(&mut self) -> TransactionPlan {
        let record = self.allocate_uaid_record();
        let account = account_from_record(&record);
        self.track_account(record.clone());
        TransactionPlan {
            state_updates: Vec::new(),
            label: "register_uaid_account",
            instructions: vec![InstructionBox::from(Register::account(account))],
            signer: self.treasury.clone(),
            expect_success: true,
        }
    }

    fn plan_mint_asset(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let beneficiary = self.random_user(rng)?.clone();
        let amount: Numeric = rng.random_range(1_u32..=100_u32).into();
        let asset_id = AssetId::new(self.asset_numeric.clone(), beneficiary.id.clone());
        self.asset_instances.insert(asset_id.clone());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "mint_asset",
            instructions: vec![InstructionBox::from(Mint::asset_numeric(amount, asset_id))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_transfer_asset(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let receiver = self.random_user(rng)?.clone();
        let amount: Numeric = rng.random_range(1_u32..=50_u32).into();
        let treasury_asset = AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone());
        let receiver_asset = AssetId::new(self.asset_numeric.clone(), receiver.id.clone());
        self.asset_instances.insert(treasury_asset.clone());
        self.asset_instances.insert(receiver_asset);
        let instructions = vec![
            InstructionBox::from(Mint::asset_numeric(amount.clone(), treasury_asset.clone())),
            InstructionBox::from(Transfer::asset_numeric(
                treasury_asset,
                amount,
                receiver.id.clone(),
            )),
        ];
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "transfer_asset",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_burn_asset(&mut self, rng: &mut StdRng) -> TransactionPlan {
        let amount: Numeric = rng.random_range(1_u32..=20_u32).into();
        let treasury_asset = AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone());
        self.asset_instances.insert(treasury_asset.clone());
        let instructions = vec![
            InstructionBox::from(Mint::asset_numeric(amount.clone(), treasury_asset.clone())),
            InstructionBox::from(Burn::asset_numeric(amount, treasury_asset)),
        ];
        TransactionPlan {
            state_updates: Vec::new(),
            label: "burn_asset",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        }
    }

    fn plan_set_key(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let target = self.random_user(rng)?.clone();
        let key: Name = format!("flag_{}", rng.random_range(0_u32..=999_u32))
            .parse()
            .map_err(|_| eyre!("failed to build metadata key"))?;
        let value = json_pair("chaos", true);
        self.account_metadata
            .entry(target.id.clone())
            .or_default()
            .insert(key.clone());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "set_account_kv",
            instructions: vec![InstructionBox::from(SetKeyValue::account(
                target.id.clone(),
                key,
                value,
            ))],
            signer: target,
            expect_success: true,
        })
    }

    fn plan_remove_key(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let target = self.random_user(rng)?.clone();
        if let Some(keys) = self.account_metadata.get_mut(&target.id) {
            if let Some(existing) = keys.iter().next().cloned() {
                keys.remove(&existing);
            }
            if keys.is_empty() {
                self.account_metadata.remove(&target.id);
            }
        }
        let key: Name = format!("ephemeral_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse key name"))?;
        let instructions = vec![
            InstructionBox::from(SetKeyValue::account(
                target.id.clone(),
                key.clone(),
                json_pair("temp", self.bump_invalid()),
            )),
            InstructionBox::from(RemoveKeyValue::account(target.id.clone(), key)),
        ];
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "remove_account_kv",
            instructions,
            signer: target,
            expect_success: true,
        })
    }

    fn plan_register_asset_definition(&mut self) -> Result<TransactionPlan> {
        let suffix = self.bump_asset_definition();
        let domain_name = self.base_domain.name().to_string();
        let definition_id: AssetDefinitionId = format!("chaos_asset_{suffix}#{domain_name}")
            .parse()
            .map_err(|_| eyre!("failed to build asset definition id"))?;
        let asset_definition = AssetDefinition::numeric(definition_id.clone());
        self.asset_definitions.insert(definition_id.clone());
        self.asset_definitions_unclaimed
            .insert(definition_id.clone());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "register_asset_definition",
            instructions: vec![InstructionBox::from(Register::asset_definition(
                asset_definition,
            ))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_unregister_asset_definition(&mut self, _rng: &mut StdRng) -> TransactionPlan {
        if let Some(candidate) = self.asset_definitions_unclaimed.iter().next().cloned() {
            self.asset_definitions_unclaimed.remove(&candidate);
            self.asset_definitions.remove(&candidate);
            self.asset_definition_metadata.remove(&candidate);
            return TransactionPlan {
                state_updates: Vec::new(),
                label: "unregister_asset_definition",
                instructions: vec![InstructionBox::from(Unregister::asset_definition(
                    candidate,
                ))],
                signer: self.treasury.clone(),
                expect_success: true,
            };
        }
        let domain_name = self.base_domain.name().to_string();
        let fallback: AssetDefinitionId =
            format!("ghost_asset_{}#{domain_name}", self.bump_invalid())
                .parse()
                .expect("ghost asset definition id should parse");
        TransactionPlan {
            state_updates: Vec::new(),
            label: "unregister_asset_definition",
            instructions: vec![InstructionBox::from(Unregister::asset_definition(fallback))],
            signer: self.treasury.clone(),
            expect_success: false,
        }
    }

    fn plan_set_domain_key(&mut self) -> Result<TransactionPlan> {
        let key: Name = format!("domain_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse domain metadata key"))?;
        let domain = self.base_domain.clone();
        self.domain_metadata
            .entry(domain.clone())
            .or_default()
            .insert(key.clone());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "set_domain_kv",
            instructions: vec![InstructionBox::from(SetKeyValue::domain(
                domain,
                key,
                json_pair("domain", "chaos"),
            ))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_remove_domain_key(&mut self) -> Result<TransactionPlan> {
        let domain = self.base_domain.clone();
        if let Some(keys) = self.domain_metadata.get_mut(&domain) {
            if let Some(existing) = keys.iter().next().cloned() {
                keys.remove(&existing);
            }
            if keys.is_empty() {
                self.domain_metadata.remove(&domain);
            }
        }
        let key: Name = format!("domain_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse fallback domain key"))?;
        let instructions = vec![
            InstructionBox::from(SetKeyValue::domain(
                domain.clone(),
                key.clone(),
                json_pair("ephemeral", true),
            )),
            InstructionBox::from(RemoveKeyValue::domain(domain, key)),
        ];
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "remove_domain_kv",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_set_asset_definition_key(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let definition = self.random_asset_definition(rng)?;
        let key: Name = format!("asset_def_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse asset definition metadata key"))?;
        self.asset_definition_metadata
            .entry(definition.clone())
            .or_default()
            .insert(key.clone());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "set_asset_definition_kv",
            instructions: vec![InstructionBox::from(SetKeyValue::asset_definition(
                definition,
                key,
                json_pair("definition", "chaos"),
            ))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_remove_asset_definition_key(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let definition = self.random_asset_definition(rng)?;
        if let Some(keys) = self.asset_definition_metadata.get_mut(&definition) {
            if let Some(existing) = keys.iter().next().cloned() {
                keys.remove(&existing);
            }
            if keys.is_empty() {
                self.asset_definition_metadata.remove(&definition);
            }
        }
        let key: Name = format!("asset_def_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse fallback asset definition key"))?;
        let instructions = vec![
            InstructionBox::from(SetKeyValue::asset_definition(
                definition.clone(),
                key.clone(),
                json_pair("ephemeral", true),
            )),
            InstructionBox::from(RemoveKeyValue::asset_definition(definition, key)),
        ];
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "remove_asset_definition_kv",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_set_asset_metadata(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let asset = self
            .random_asset_instance(rng)
            .unwrap_or_else(|_| AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone()));
        let key: Name = format!("asset_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse asset metadata key"))?;
        self.asset_metadata
            .entry(asset.clone())
            .or_default()
            .insert(key.clone());
        let amount: Numeric = 1_u32.into();
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "set_asset_kv",
            instructions: vec![
                InstructionBox::from(Mint::asset_numeric(amount, asset.clone())),
                InstructionBox::from(SetAssetKeyValue::new(
                    asset,
                    key,
                    json_pair("asset", "chaos"),
                )),
            ],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_remove_asset_metadata(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let asset = self
            .random_asset_instance(rng)
            .unwrap_or_else(|_| AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone()));
        if let Some(keys) = self.asset_metadata.get_mut(&asset) {
            if let Some(existing) = keys.iter().next().cloned() {
                keys.remove(&existing);
            }
            if keys.is_empty() {
                self.asset_metadata.remove(&asset);
            }
        }
        let key: Name = format!("asset_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse fallback asset key"))?;
        let amount: Numeric = 1_u32.into();
        let instructions = vec![
            InstructionBox::from(Mint::asset_numeric(amount, asset.clone())),
            InstructionBox::from(SetAssetKeyValue::new(
                asset.clone(),
                key.clone(),
                json_pair("ephemeral", true),
            )),
            InstructionBox::from(RemoveAssetKeyValue::new(asset, key)),
        ];
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "remove_asset_kv",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_register_nft(&mut self, _rng: &mut StdRng) -> Result<TransactionPlan> {
        let suffix = self.bump_nft();
        let domain_name = self.base_domain.name().to_string();
        let nft_id: NftId = format!("chaos_nft_{suffix}${domain_name}")
            .parse()
            .map_err(|_| eyre!("failed to parse nft id"))?;
        let nft = Nft::new(nft_id.clone(), Metadata::default());
        self.nft_holdings
            .insert(nft_id.clone(), self.treasury.id.clone());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "register_nft",
            instructions: vec![InstructionBox::from(Register::nft(nft))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_transfer_nft(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let suffix = self.bump_nft();
        let domain_name = self.base_domain.name().to_string();
        let nft_id: NftId = format!("chaos_nft_{suffix}${domain_name}")
            .parse()
            .map_err(|_| eyre!("failed to parse nft id"))?;
        let receiver = self.random_user_except(rng, &self.treasury.id)?;
        let nft = Nft::new(nft_id.clone(), Metadata::default());
        let instructions = vec![
            InstructionBox::from(Register::nft(nft)),
            InstructionBox::from(Transfer::nft(
                self.treasury.id.clone(),
                nft_id.clone(),
                receiver.id.clone(),
            )),
        ];
        self.nft_holdings.insert(nft_id, receiver.id.clone());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "transfer_nft",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_set_trigger_key(&mut self, _rng: &mut StdRng) -> Result<TransactionPlan> {
        let mut instructions = Vec::new();
        let trigger_id: TriggerId = format!("metadata_trigger_{}", self.bump_trigger())
            .parse()
            .map_err(|_| eyre!("failed to parse metadata trigger id"))?;
        let log_instruction = Log::new(Level::INFO, "trigger metadata bootstrap".to_string());
        let action = Action::new(
            vec![InstructionBox::from(log_instruction)],
            Repeats::Indefinitely,
            self.treasury.id.clone(),
            EventFilterBox::ExecuteTrigger(
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
            ),
        );
        instructions.push(InstructionBox::from(Register::trigger(Trigger::new(
            trigger_id.clone(),
            action,
        ))));

        let key: Name = format!("trigger_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse trigger metadata key"))?;
        instructions.push(InstructionBox::from(SetKeyValue::trigger(
            trigger_id.clone(),
            key.clone(),
            json_pair("trigger", "chaos"),
        )));
        let state_updates = vec![
            PlanUpdate::RegisterTrigger(trigger_id.clone()),
            PlanUpdate::SetTriggerMetadata {
                trigger_id: trigger_id.clone(),
                key,
            },
        ];
        Ok(TransactionPlan {
            state_updates,
            label: "set_trigger_kv",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_remove_trigger_key(&mut self, _rng: &mut StdRng) -> Result<TransactionPlan> {
        let mut instructions = Vec::new();
        let trigger_id: TriggerId = format!("metadata_trigger_{}", self.bump_trigger())
            .parse()
            .map_err(|_| eyre!("failed to parse metadata trigger id"))?;
        let log_instruction = Log::new(Level::INFO, "trigger metadata bootstrap".to_string());
        let action = Action::new(
            vec![InstructionBox::from(log_instruction)],
            Repeats::Indefinitely,
            self.treasury.id.clone(),
            EventFilterBox::ExecuteTrigger(
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
            ),
        );
        instructions.push(InstructionBox::from(Register::trigger(Trigger::new(
            trigger_id.clone(),
            action,
        ))));

        let key: Name = format!("trigger_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse fallback trigger key"))?;
        instructions.push(InstructionBox::from(SetKeyValue::trigger(
            trigger_id.clone(),
            key.clone(),
            json_pair("ephemeral", true),
        )));
        instructions.push(InstructionBox::from(RemoveKeyValue::trigger(
            trigger_id.clone(),
            key,
        )));
        let state_updates = vec![
            PlanUpdate::RegisterTrigger(trigger_id.clone()),
            PlanUpdate::ClearTriggerMetadata(trigger_id.clone()),
        ];
        Ok(TransactionPlan {
            state_updates,
            label: "remove_trigger_kv",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_mint_trigger_repetitions(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let mut instructions = Vec::new();
        let trigger_id = if let Some(existing) = self.random_repeatable_trigger(rng) {
            existing
        } else {
            let trigger_id: TriggerId = format!("repeat_trigger_{}", self.bump_trigger())
                .parse()
                .map_err(|_| eyre!("failed to parse repeat trigger id"))?;
            let action = Action::new(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "repetition bootstrap".to_string(),
                ))],
                Repeats::Exactly(1),
                self.treasury.id.clone(),
                EventFilterBox::ExecuteTrigger(
                    ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
                ),
            );
            instructions.push(InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id.clone(),
                action,
            ))));
            let updates = vec![PlanUpdate::TrackRepeatableTrigger(trigger_id.clone())];
            return Ok(TransactionPlan {
                state_updates: updates,
                label: "mint_trigger_repetitions",
                instructions,
                signer: self.treasury.clone(),
                expect_success: true,
            });
        };
        let amount = rng.random_range(1_u32..=3_u32);
        instructions.push(InstructionBox::from(Mint::trigger_repetitions(
            amount,
            trigger_id.clone(),
        )));
        let state_updates = vec![PlanUpdate::MintTriggerRepetitions { trigger_id, amount }];
        Ok(TransactionPlan {
            state_updates,
            label: "mint_trigger_repetitions",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_burn_trigger_repetitions(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let mut instructions = Vec::new();
        let trigger_id = if let Some(existing) = self.random_repeatable_trigger(rng) {
            existing
        } else {
            return self.plan_mint_trigger_repetitions(rng);
        };
        let tracked = *self.trigger_repetitions.get(&trigger_id).unwrap_or(&0);
        let pending = self.pending_trigger_repetitions(&trigger_id);
        let available = tracked.saturating_sub(pending);
        if available <= 1 {
            return self.plan_mint_trigger_repetitions(rng);
        }
        let burn_amount = rng.random_range(1..=available.saturating_sub(1));
        self.reserve_trigger_repetitions(&trigger_id, burn_amount);
        instructions.push(InstructionBox::from(Burn::trigger_repetitions(
            burn_amount,
            trigger_id.clone(),
        )));
        let state_updates = vec![
            PlanUpdate::BurnTriggerRepetitions {
                trigger_id: trigger_id.clone(),
                amount: burn_amount,
            },
            PlanUpdate::ReleaseTriggerRepetitionsReservation {
                trigger_id,
                amount: burn_amount,
            },
        ];
        Ok(TransactionPlan {
            state_updates,
            label: "burn_trigger_repetitions",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_register_role(&mut self) -> Result<TransactionPlan> {
        let role_id: RoleId = format!("CHAOS_ROLE_{}", self.bump_role())
            .parse()
            .map_err(|_| eyre!("failed to parse role id"))?;
        let permission = CanModifyAccountMetadata {
            account: self.treasury.id.clone(),
        };
        let role = Role::new(role_id.clone(), self.treasury.id.clone()).add_permission(permission);
        self.registered_roles.push(role_id.clone());
        self.role_memberships
            .entry(role_id.clone())
            .or_default()
            .insert(self.treasury.id.clone());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "register_role",
            instructions: vec![InstructionBox::from(Register::role(role))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_grant_role(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        if self.registered_roles.is_empty() {
            return self.plan_register_role();
        }
        let role = self
            .registered_roles
            .choose(rng)
            .expect("role list not empty")
            .clone();
        let assigned_accounts: HashSet<ScopedAccountId> = self
            .role_memberships
            .get(&role)
            .cloned()
            .unwrap_or_default();
        let available_accounts: Vec<&AccountRecord> = self
            .users
            .iter()
            .chain(std::iter::once(&self.treasury))
            .filter(|record| !assigned_accounts.contains(&record.id))
            .collect();
        if let Some(account) = available_accounts.choose(rng).copied() {
            let account_id = account.id.clone();
            self.role_memberships
                .entry(role.clone())
                .or_default()
                .insert(account_id.clone());
            return Ok(TransactionPlan {
                state_updates: Vec::new(),
                label: "grant_role",
                instructions: vec![InstructionBox::from(Grant::account_role(role, account_id))],
                signer: self.treasury.clone(),
                expect_success: true,
            });
        }

        let assigned_vec: Vec<ScopedAccountId> = assigned_accounts.into_iter().collect();
        let fallback_account = if let Some(candidate) = assigned_vec.choose(rng) {
            candidate.clone()
        } else {
            self.random_user(rng)?.id.clone()
        };
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "grant_role",
            instructions: vec![InstructionBox::from(Grant::account_role(
                role,
                fallback_account,
            ))],
            signer: self.treasury.clone(),
            expect_success: false,
        })
    }

    fn plan_revoke_role(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        if self.registered_roles.is_empty() {
            return self.plan_register_role();
        }
        let role = self
            .registered_roles
            .choose(rng)
            .expect("role list not empty")
            .clone();
        let existing_members: Vec<ScopedAccountId> = self
            .role_memberships
            .get(&role)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();

        if let Some(account_id) = existing_members
            .iter()
            .filter(|member| *member != &self.treasury.id)
            .cloned()
            .collect::<Vec<_>>()
            .choose(rng)
            .cloned()
        {
            let remove_role_entry = self.role_memberships.get_mut(&role).is_some_and(|members| {
                members.remove(&account_id);
                members.is_empty()
            });
            if remove_role_entry {
                self.role_memberships.remove(&role);
            }
            return Ok(TransactionPlan {
                state_updates: Vec::new(),
                label: "revoke_role",
                instructions: vec![InstructionBox::from(Revoke::account_role(role, account_id))],
                signer: self.treasury.clone(),
                expect_success: true,
            });
        }

        let account = self.random_user_except(rng, &self.treasury.id)?;
        let instructions = vec![
            InstructionBox::from(Grant::account_role(role.clone(), account.id.clone())),
            InstructionBox::from(Revoke::account_role(role.clone(), account.id.clone())),
        ];
        let remove_role_entry = self.role_memberships.get_mut(&role).is_some_and(|members| {
            members.remove(&account.id);
            members.is_empty()
        });
        if remove_role_entry {
            self.role_memberships.remove(&role);
        }
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "revoke_role",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_time_trigger(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let trigger_id: TriggerId = format!("time_trigger_{}", self.bump_trigger())
            .parse()
            .map_err(|_| eyre!("failed to parse trigger id"))?;
        let beneficiary = self.random_user(rng)?.clone();
        let amount: Numeric = rng.random_range(1_u32..=5_u32).into();
        let mint = Mint::asset_numeric(
            amount,
            AssetId::new(self.asset_numeric.clone(), beneficiary.id.clone()),
        );
        let schedule = Schedule::starting_at(Duration::from_millis(500))
            .with_period(Duration::from_millis(1_500));
        let action = Action::new(
            vec![InstructionBox::from(mint)],
            Repeats::Exactly(1),
            self.treasury.id.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
        );
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "register_time_trigger",
            instructions: vec![InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id, action,
            )))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_data_trigger(&mut self, _rng: &mut StdRng) -> Result<TransactionPlan> {
        let trigger_id: TriggerId = format!("data_trigger_{}", self.bump_trigger())
            .parse()
            .map_err(|_| eyre!("failed to parse data trigger id"))?;
        let amount: Numeric = 1_u32.into();
        let mint = Mint::asset_numeric(
            amount,
            AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone()),
        );
        let filter = AccountEventFilter::new().for_events(AccountEventSet::Created);
        let action = Action::new(
            vec![InstructionBox::from(mint)],
            Repeats::Indefinitely,
            self.treasury.id.clone(),
            DataEventFilter::Account(filter),
        );
        let state_updates = vec![PlanUpdate::RegisterTrigger(trigger_id.clone())];
        Ok(TransactionPlan {
            state_updates,
            label: "register_data_trigger",
            instructions: vec![InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id, action,
            )))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_pipeline_trigger(&mut self, _rng: &mut StdRng) -> Result<TransactionPlan> {
        let trigger_id: TriggerId = format!("pipeline_trigger_{}", self.bump_trigger())
            .parse()
            .map_err(|_| eyre!("failed to parse pipeline trigger id"))?;
        let log_instruction = Log::new(Level::INFO, "pipeline chaos".to_string());
        let action = Action::new(
            vec![InstructionBox::from(log_instruction)],
            Repeats::Indefinitely,
            self.treasury.id.clone(),
            EventFilterBox::Pipeline(PipelineEventFilterBox::Transaction(
                TransactionEventFilter::new(),
            )),
        );
        let state_updates = vec![PlanUpdate::RegisterTrigger(trigger_id.clone())];
        Ok(TransactionPlan {
            state_updates,
            label: "register_pipeline_trigger",
            instructions: vec![InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id, action,
            )))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_execute_trigger(&mut self, _rng: &mut StdRng) -> Result<TransactionPlan> {
        let trigger_id: TriggerId = format!("call_trigger_{}", self.bump_trigger())
            .parse()
            .map_err(|_| eyre!("failed to parse call trigger id"))?;
        let log_instruction = Log::new(Level::INFO, "call trigger".to_string());
        let action = Action::new(
            vec![InstructionBox::from(log_instruction)],
            Repeats::Indefinitely,
            self.treasury.id.clone(),
            EventFilterBox::ExecuteTrigger(
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
            ),
        );
        let trigger = Trigger::new(trigger_id.clone(), action);
        let state_updates = vec![
            PlanUpdate::RegisterTrigger(trigger_id.clone()),
            PlanUpdate::RegisterCallTrigger(trigger_id.clone()),
        ];
        Ok(TransactionPlan {
            state_updates,
            label: "execute_trigger",
            instructions: vec![
                InstructionBox::from(Register::trigger(trigger)),
                InstructionBox::from(ExecuteTrigger::new(trigger_id)),
            ],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_execute_missing_trigger(&mut self) -> Result<TransactionPlan> {
        let trigger_id: TriggerId = format!("ghost_trigger_{}", self.bump_invalid())
            .parse()
            .map_err(|_| eyre!("failed to parse ghost trigger id"))?;
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "execute_missing_trigger",
            instructions: vec![InstructionBox::from(ExecuteTrigger::new(trigger_id))],
            signer: self.treasury.clone(),
            expect_success: false,
        })
    }

    fn plan_deploy_ivm(&mut self, _rng: &mut StdRng) -> Result<TransactionPlan> {
        let trigger_id: TriggerId = format!("ivm_trigger_{}", self.bump_trigger())
            .parse()
            .map_err(|_| eyre!("failed to parse ivm trigger id"))?;
        let bytecode = smart_contracts::ivm_trigger_program()?;
        let action = Action::new(
            bytecode,
            Repeats::Exactly(1),
            self.treasury.id.clone(),
            TimeEventFilter::new(ExecutionTime::PreCommit),
        );
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "deploy_ivm_contract",
            instructions: vec![InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id, action,
            )))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_deploy_kotodama(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let trigger_id: TriggerId = format!("kotodama_trigger_{}", self.bump_trigger())
            .parse()
            .map_err(|_| eyre!("failed to parse kotodama trigger id"))?;
        let program_name = [
            "asset_ops",
            "domain_ops",
            "mint_rose_trigger",
            "tuple_return_demo",
        ]
        .choose(rng)
        .copied()
        .unwrap_or("asset_ops");
        let bytecode = smart_contracts::kotodama_program(program_name)?;
        let action = Action::new(
            bytecode,
            Repeats::Indefinitely,
            self.treasury.id.clone(),
            DataEventFilter::Any,
        );
        let state_updates = vec![PlanUpdate::RegisterTrigger(trigger_id.clone())];
        Ok(TransactionPlan {
            state_updates,
            label: "deploy_kotodama_contract",
            instructions: vec![InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id, action,
            )))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_publish_space_manifest(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let dataspace = self.random_dataspace(rng);
        let mut instructions = Vec::new();
        let uaid = if let Some(uaid) = self.pick_uaid_without_manifest(dataspace, rng) {
            self.uaid_accounts
                .get(&uaid)
                .ok_or_else(|| eyre!("UAID record missing from registry"))?;
            uaid
        } else {
            let record = self.allocate_uaid_record();
            let account = account_from_record(&record);
            instructions.push(InstructionBox::from(Register::account(account)));
            let uaid = record
                .uaid
                .expect("allocated UAID record should carry uaid");
            self.track_account(record);
            uaid
        };

        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid,
            dataspace,
            issued_ms: now_ms(),
            activation_epoch: self.bump_metadata(),
            expiry_epoch: None,
            entries: vec![ManifestEntry {
                scope: CapabilityScope {
                    dataspace: Some(dataspace),
                    program: None,
                    method: None,
                    asset: None,
                    role: None,
                },
                effect: ManifestEffect::Allow(Allowance {
                    max_amount: None,
                    window: AllowanceWindow::PerSlot,
                }),
                notes: Some("chaos allowance".to_string()),
            }],
        };
        instructions.push(InstructionBox::from(PublishSpaceDirectoryManifest {
            manifest,
        }));
        self.space_directory_manifests
            .entry(uaid)
            .or_default()
            .insert(dataspace);

        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "publish_space_directory_manifest",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_revoke_space_manifest(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let dataspace = self.random_dataspace(rng);
        let Some(uaid) = self.pick_manifest_for_dataspace(dataspace, rng) else {
            let missing = UniversalAccountId::from_hash(Hash::new(b"izanami-missing-manifest"));
            return Ok(TransactionPlan {
                state_updates: Vec::new(),
                label: "revoke_space_directory_manifest",
                instructions: vec![InstructionBox::from(RevokeSpaceDirectoryManifest {
                    uaid: missing,
                    dataspace,
                    revoked_epoch: self.bump_invalid(),
                    reason: Some("revocation should fail for missing manifest".to_string()),
                })],
                signer: self.treasury.clone(),
                expect_success: false,
            });
        };

        self.mark_manifest_removed(uaid, dataspace);
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "revoke_space_directory_manifest",
            instructions: vec![InstructionBox::from(RevokeSpaceDirectoryManifest {
                uaid,
                dataspace,
                revoked_epoch: self.bump_metadata(),
                reason: Some("chaos revocation".to_string()),
            })],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_expire_space_manifest(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let dataspace = self.random_dataspace(rng);
        let Some(uaid) = self.pick_manifest_for_dataspace(dataspace, rng) else {
            let missing = UniversalAccountId::from_hash(Hash::new(b"izanami-missing-expiry"));
            return Ok(TransactionPlan {
                state_updates: Vec::new(),
                label: "expire_space_directory_manifest",
                instructions: vec![InstructionBox::from(ExpireSpaceDirectoryManifest {
                    uaid: missing,
                    dataspace,
                    expired_epoch: self.bump_invalid(),
                })],
                signer: self.treasury.clone(),
                expect_success: false,
            });
        };

        self.mark_manifest_removed(uaid, dataspace);
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "expire_space_directory_manifest",
            instructions: vec![InstructionBox::from(ExpireSpaceDirectoryManifest {
                uaid,
                dataspace,
                expired_epoch: self.bump_metadata(),
            })],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_register_public_validator(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let lane = self.random_lane(rng);
        let validator = self
            .random_staking_validator(rng)
            .unwrap_or(self.random_user(rng)?.clone());
        let stake_account = if self.nexus_staking.is_some() {
            validator.clone()
        } else {
            self.random_user(rng)?.clone()
        };
        let stake_amount_value = u64::from(rng.random_range(10_u32..=100_u32));
        let stake_amount: Numeric = stake_amount_value.into();
        let stake_asset_def = self.stake_asset_definition();
        let treasury_asset = AssetId::new(stake_asset_def.clone(), self.treasury.id.clone());
        let stake_asset = AssetId::new(stake_asset_def, stake_account.id.clone());

        let mut instructions = vec![InstructionBox::from(Mint::asset_numeric(
            stake_amount.clone(),
            treasury_asset.clone(),
        ))];
        instructions.push(InstructionBox::from(Transfer::asset_numeric(
            treasury_asset.clone(),
            stake_amount.clone(),
            stake_account.id.clone(),
        )));
        instructions.push(InstructionBox::from(RegisterPublicLaneValidator {
            lane_id: lane,
            validator: validator.id.clone(),
            stake_account: stake_account.id.clone(),
            initial_stake: stake_amount,
            metadata: Metadata::default(),
        }));
        let mut expect_success = self.nexus_staking_expect_success();
        if expect_success
            && self
                .public_lane_validators
                .get(&lane)
                .is_some_and(|validators| validators.contains(&validator.id))
        {
            expect_success = false;
        }
        if expect_success {
            self.asset_instances.insert(treasury_asset);
            self.asset_instances.insert(stake_asset);
            self.public_lane_validators
                .entry(lane)
                .or_default()
                .insert(validator.id.clone());
            self.add_public_lane_stake_share(
                lane,
                &validator.id,
                &stake_account.id,
                stake_amount_value,
            );
        }

        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "register_public_lane_validator",
            instructions,
            signer: self.treasury.clone(),
            expect_success,
        })
    }

    fn plan_bond_public_stake(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let Some((lane, validator)) = self.pick_registered_validator(rng) else {
            let fallback_lane = self.random_lane(rng);
            let staker = self.random_user(rng)?.clone();
            let amount: Numeric = rng.random_range(1_u32..=5_u32).into();
            return Ok(TransactionPlan {
                state_updates: Vec::new(),
                label: "bond_public_lane_stake",
                instructions: vec![InstructionBox::from(BondPublicLaneStake {
                    lane_id: fallback_lane,
                    validator: staker.id.clone(),
                    staker: staker.id.clone(),
                    amount,
                    metadata: Metadata::default(),
                })],
                signer: staker,
                expect_success: false,
            });
        };
        let staker = self.random_user(rng)?.clone();
        let amount_value = u64::from(rng.random_range(5_u32..=40_u32));
        let amount: Numeric = amount_value.into();
        let stake_asset_def = self.stake_asset_definition();
        let treasury_asset = AssetId::new(stake_asset_def.clone(), self.treasury.id.clone());
        let staker_asset = AssetId::new(stake_asset_def, staker.id.clone());
        let mut instructions = vec![InstructionBox::from(Mint::asset_numeric(
            amount.clone(),
            treasury_asset.clone(),
        ))];
        instructions.push(InstructionBox::from(Transfer::asset_numeric(
            treasury_asset.clone(),
            amount.clone(),
            staker.id.clone(),
        )));
        instructions.push(InstructionBox::from(BondPublicLaneStake {
            lane_id: lane,
            validator: validator.id.clone(),
            staker: staker.id.clone(),
            amount,
            metadata: Metadata::default(),
        }));
        let expect_success = self.nexus_staking_expect_success();
        if expect_success {
            self.asset_instances.insert(treasury_asset);
            self.asset_instances.insert(staker_asset);
            self.add_public_lane_stake_share(lane, &validator.id, &staker.id, amount_value);
        }

        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "bond_public_lane_stake",
            instructions,
            signer: self.treasury.clone(),
            expect_success,
        })
    }

    fn plan_schedule_public_unbond(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let Some((lane, validator)) = self.pick_registered_validator(rng) else {
            let request_id = Hash::new(b"izanami-missing-unbond");
            let staker = self.random_user(rng)?.clone();
            return Ok(TransactionPlan {
                state_updates: Vec::new(),
                label: "schedule_public_lane_unbond",
                instructions: vec![InstructionBox::from(SchedulePublicLaneUnbond {
                    lane_id: LaneId::SINGLE,
                    validator: staker.id.clone(),
                    staker: staker.id.clone(),
                    request_id,
                    amount: 1u32.into(),
                    release_at_ms: now_ms(),
                })],
                signer: staker,
                expect_success: false,
            });
        };
        let staker = if self.nexus_staking.is_some() {
            validator.clone()
        } else {
            self.random_user(rng)?.clone()
        };
        let request_id = Hash::new(format!("izanami-unbond-{}", self.bump_staking()).as_bytes());
        let mut expect_success = self.nexus_staking_expect_success();
        let available = if expect_success {
            self.available_public_lane_stake_share(lane, &validator.id, &staker.id)
        } else {
            0
        };
        let amount_value = if available > 0 {
            rng.random_range(1_u64..=available.min(10))
        } else {
            u64::from(rng.random_range(1_u32..=10_u32))
        };
        let amount: Numeric = amount_value.into();
        let release_at = now_ms().saturating_add(5_000);
        if expect_success && available >= amount_value {
            self.pending_unbonds.push(PendingUnbond {
                lane,
                validator: validator.id.clone(),
                staker: staker.id.clone(),
                request_id,
            });
            self.reduce_public_lane_stake_share(lane, &validator.id, &staker.id, amount_value);
        } else {
            expect_success = false;
        }

        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "schedule_public_lane_unbond",
            instructions: vec![InstructionBox::from(SchedulePublicLaneUnbond {
                lane_id: lane,
                validator: validator.id.clone(),
                staker: staker.id.clone(),
                request_id,
                amount,
                release_at_ms: release_at,
            })],
            signer: staker,
            expect_success,
        })
    }

    fn plan_finalize_public_unbond(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let pending = if let Some(entry) = self.pending_unbonds.choose(rng).cloned() {
            entry
        } else {
            return self.plan_schedule_public_unbond(rng);
        };
        let expect_success = self.nexus_staking_expect_success();
        if expect_success {
            self.pending_unbonds
                .retain(|entry| entry.request_id != pending.request_id);
        }
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "finalize_public_lane_unbond",
            instructions: vec![InstructionBox::from(FinalizePublicLaneUnbond {
                lane_id: pending.lane,
                validator: pending.validator.clone(),
                staker: pending.staker.clone(),
                request_id: pending.request_id,
            })],
            signer: self.treasury.clone(),
            expect_success,
        })
    }

    fn plan_slash_public_validator(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let Some((lane, validator)) = self.pick_registered_validator(rng) else {
            let slash_id = Hash::new(b"izanami-missing-slash");
            let staker = self.random_user(rng)?.clone();
            return Ok(TransactionPlan {
                state_updates: Vec::new(),
                label: "slash_public_lane_validator",
                instructions: vec![InstructionBox::from(SlashPublicLaneValidator {
                    lane_id: LaneId::SINGLE,
                    validator: staker.id.clone(),
                    slash_id,
                    amount: 1u32.into(),
                    reason_code: "validator_missing".to_string(),
                    metadata: Metadata::default(),
                })],
                signer: staker,
                expect_success: false,
            });
        };

        let amount: Numeric = rng.random_range(1_u32..=20_u32).into();
        let slash_id = Hash::new(format!("izanami-slash-{}", self.bump_staking()).as_bytes());
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "slash_public_lane_validator",
            instructions: vec![InstructionBox::from(SlashPublicLaneValidator {
                lane_id: lane,
                validator: validator.id.clone(),
                slash_id,
                amount,
                reason_code: "chaos_injected".to_string(),
                metadata: Metadata::default(),
            })],
            signer: self.treasury.clone(),
            expect_success: self.nexus_staking_expect_success(),
        })
    }

    fn plan_record_public_rewards(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let Some((lane, validator)) = self.pick_registered_validator(rng) else {
            let epoch = self.bump_staking();
            let (reward_asset_def, reward_sink) = self.fee_asset_and_sink();
            let reward_asset = AssetId::new(reward_asset_def, reward_sink);
            return Ok(TransactionPlan {
                state_updates: Vec::new(),
                label: "record_public_lane_rewards",
                instructions: vec![InstructionBox::from(RecordPublicLaneRewards {
                    lane_id: LaneId::SINGLE,
                    epoch,
                    reward_asset,
                    total_reward: 1u32.into(),
                    shares: Vec::new(),
                    metadata: Metadata::default(),
                })],
                signer: self.treasury.clone(),
                expect_success: false,
            });
        };
        let (reward_asset_def, reward_sink) = self.fee_asset_and_sink();
        let reward_asset = AssetId::new(reward_asset_def, reward_sink);
        let expect_success = self.nexus_staking_expect_success();
        if expect_success {
            self.asset_instances.insert(reward_asset.clone());
        }
        let reward: Numeric = rng.random_range(5_u32..=50_u32).into();
        let share = PublicLaneRewardShare {
            account: validator.id.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: reward.clone(),
        };
        let epoch = self.bump_staking();
        let mint = InstructionBox::from(Mint::asset_numeric(reward.clone(), reward_asset.clone()));
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "record_public_lane_rewards",
            instructions: vec![
                mint,
                InstructionBox::from(RecordPublicLaneRewards {
                    lane_id: lane,
                    epoch,
                    reward_asset,
                    total_reward: reward,
                    shares: vec![share],
                    metadata: Metadata::default(),
                }),
            ],
            signer: self.treasury.clone(),
            expect_success,
        })
    }

    fn plan_dvp_settlement(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let seller = self.treasury.clone();
        let buyer = self.random_user_except(rng, &seller.id)?;
        let settlement_id: SettlementId = format!("settlement_{}", self.bump_settlement())
            .parse()
            .map_err(|_| eyre!("failed to parse settlement id"))?;

        let delivery_amount: Numeric = rng.random_range(1_u32..=25_u32).into();
        let payment_amount: Numeric = rng.random_range(1_u32..=25_u32).into();
        let delivery_asset = AssetId::new(self.asset_numeric.clone(), seller.id.clone());
        let payment_asset = AssetId::new(self.asset_numeric.clone(), buyer.id.clone());
        self.asset_instances.insert(delivery_asset.clone());
        self.asset_instances.insert(payment_asset.clone());

        let delivery_mint = InstructionBox::from(Mint::asset_numeric(
            delivery_amount.clone(),
            delivery_asset.clone(),
        ));
        let payment_mint = InstructionBox::from(Mint::asset_numeric(
            payment_amount.clone(),
            payment_asset.clone(),
        ));

        let delivery_leg = SettlementLeg::new(
            self.asset_numeric.clone(),
            delivery_amount,
            seller.id.clone(),
            buyer.id.clone(),
        );
        let payment_leg = SettlementLeg::new(
            self.asset_numeric.clone(),
            payment_amount,
            buyer.id.clone(),
            seller.id.clone(),
        );
        let dvp = SettlementInstructionBox::from(DvpIsi::new(
            settlement_id,
            delivery_leg,
            payment_leg,
            SettlementPlan::default(),
        ));

        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "dvp_settlement",
            instructions: vec![delivery_mint, payment_mint, InstructionBox::from(dvp)],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_seed_replication(&mut self) -> Result<TransactionPlan> {
        let (manifest_digest, chunker, provider_id) = {
            let Some(replication) = self.sorafs_replication.as_ref() else {
                return Err(eyre!("SoraFS replication seed not initialized"));
            };
            (
                replication.manifest_digest,
                replication.chunker.clone(),
                replication.provider_id,
            )
        };
        let manifest_epoch = self.bump_replication();
        let chunk_digest = *Hash::new(b"izanami-sorafs-chunk-digest").as_ref();
        let council_digest = *Hash::new(b"izanami-sorafs-council-digest").as_ref();
        let instructions = vec![
            InstructionBox::from(RegisterPinManifest {
                digest: manifest_digest,
                chunker,
                chunk_digest_sha3_256: chunk_digest,
                policy: PinPolicy::default(),
                submitted_epoch: manifest_epoch,
                alias: None,
                successor_of: None,
            }),
            InstructionBox::from(ApprovePinManifest {
                digest: manifest_digest,
                approved_epoch: manifest_epoch,
                council_envelope: None,
                council_envelope_digest: Some(council_digest),
            }),
            Box::new(RegisterProviderOwner {
                provider_id,
                owner: self.treasury.id.clone(),
            })
            .into_instruction_box(),
        ];
        self.sorafs_replication_ready = true;
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "seed_replication_manifest",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_issue_replication_order(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        if !self.sorafs_replication_ready {
            return self.plan_seed_replication();
        }
        let (manifest_digest, chunker, provider_id) = {
            let Some(replication) = self.sorafs_replication.as_ref() else {
                return Err(eyre!("SoraFS replication seed not initialized"));
            };
            (
                replication.manifest_digest,
                replication.chunker.clone(),
                replication.provider_id,
            )
        };
        let mut order_id_bytes = [0u8; 32];
        rng.fill_bytes(&mut order_id_bytes);
        if order_id_bytes.iter().all(|byte| *byte == 0) {
            order_id_bytes[0] = 1;
        }
        let issued_epoch = self.bump_replication();
        let manifest_cid = format!("cid-{issued_epoch}").into_bytes();
        let deadline_epoch = issued_epoch.saturating_add(60);
        let issued_at = now_ms() / 1_000;
        let deadline_at = issued_at.saturating_add(60);
        let order = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: order_id_bytes,
            manifest_cid,
            manifest_digest: *manifest_digest.as_bytes(),
            chunking_profile: chunker.to_handle(),
            target_replicas: 1,
            assignments: vec![ReplicationAssignmentV1 {
                provider_id: *provider_id.as_bytes(),
                slice_gib: 1,
                lane: None,
            }],
            issued_at,
            deadline_at,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 60,
                min_availability_percent_milli: 100_000,
                min_por_success_percent_milli: 100_000,
            },
            metadata: Vec::new(),
        };
        let payload = order.encode();
        let order_id = ReplicationOrderId::new(order_id_bytes);
        self.pending_replication_orders.push(order_id);
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "issue_replication_order",
            instructions: vec![InstructionBox::from(IssueReplicationOrder {
                order_id,
                order_payload: payload,
                issued_epoch,
                deadline_epoch,
            })],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_complete_replication_order(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        if self.pending_replication_orders.is_empty() {
            return self.plan_issue_replication_order(rng);
        }
        let order_index = rng.random_range(0..self.pending_replication_orders.len());
        let order_id = self.pending_replication_orders.swap_remove(order_index);
        Ok(TransactionPlan {
            state_updates: Vec::new(),
            label: "complete_replication_order",
            instructions: vec![InstructionBox::from(CompleteReplicationOrder {
                order_id,
                completion_epoch: self.bump_replication(),
            })],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn random_user(&self, rng: &mut StdRng) -> Result<&AccountRecord> {
        self.users
            .choose(rng)
            .or(Some(&self.treasury))
            .ok_or_else(|| eyre!("no accounts available"))
    }

    fn random_user_except(
        &self,
        rng: &mut StdRng,
        excluded: &ScopedAccountId,
    ) -> Result<AccountRecord> {
        let candidates: Vec<_> = self
            .users
            .iter()
            .chain(std::iter::once(&self.treasury))
            .filter(|record| &record.id != excluded)
            .cloned()
            .collect();
        candidates
            .choose(rng)
            .cloned()
            .ok_or_else(|| eyre!("no alternative accounts available"))
    }

    fn account_by_id(&self, id: &ScopedAccountId) -> Option<AccountRecord> {
        if &self.treasury.id == id {
            Some(self.treasury.clone())
        } else {
            self.users
                .iter()
                .find(|record| &record.id == id)
                .cloned()
                .or_else(|| {
                    self.nexus_staking.as_ref().and_then(|setup| {
                        setup
                            .validator_accounts
                            .iter()
                            .find(|record| &record.id == id)
                            .cloned()
                    })
                })
        }
    }

    fn random_asset_definition(&self, rng: &mut StdRng) -> Result<AssetDefinitionId> {
        let definitions: Vec<_> = self.asset_definitions.iter().cloned().collect();
        definitions
            .choose(rng)
            .cloned()
            .ok_or_else(|| eyre!("no asset definitions available"))
    }

    fn random_asset_instance(&self, rng: &mut StdRng) -> Result<AssetId> {
        let assets: Vec<_> = self.asset_instances.iter().cloned().collect();
        assets
            .choose(rng)
            .cloned()
            .ok_or_else(|| eyre!("no asset instances available"))
    }

    fn random_staking_validator(&self, rng: &mut StdRng) -> Option<AccountRecord> {
        self.nexus_staking
            .as_ref()
            .and_then(|setup| setup.validator_accounts.choose(rng).cloned())
    }

    fn stake_asset_definition(&self) -> AssetDefinitionId {
        self.nexus_staking
            .as_ref()
            .map(|setup| setup.stake_asset.clone())
            .unwrap_or_else(|| self.asset_numeric.clone())
    }

    fn fee_asset_and_sink(&self) -> (AssetDefinitionId, ScopedAccountId) {
        self.nexus_staking
            .as_ref()
            .map(|setup| (setup.fee_asset.clone(), setup.fee_sink.clone()))
            .unwrap_or_else(|| (self.asset_numeric.clone(), self.treasury.id.clone()))
    }

    fn add_public_lane_stake_share(
        &mut self,
        lane: LaneId,
        validator: &ScopedAccountId,
        staker: &ScopedAccountId,
        amount: u64,
    ) {
        let key = (lane, validator.clone(), staker.clone());
        let entry = self.public_lane_stakes.entry(key).or_insert(0);
        *entry = entry.saturating_add(amount);
    }

    fn available_public_lane_stake_share(
        &self,
        lane: LaneId,
        validator: &ScopedAccountId,
        staker: &ScopedAccountId,
    ) -> u64 {
        let key = (lane, validator.clone(), staker.clone());
        self.public_lane_stakes.get(&key).copied().unwrap_or(0)
    }

    fn reduce_public_lane_stake_share(
        &mut self,
        lane: LaneId,
        validator: &ScopedAccountId,
        staker: &ScopedAccountId,
        amount: u64,
    ) -> bool {
        let key = (lane, validator.clone(), staker.clone());
        let Some(entry) = self.public_lane_stakes.get_mut(&key) else {
            return false;
        };
        if *entry < amount {
            return false;
        }
        *entry -= amount;
        if *entry == 0 {
            self.public_lane_stakes.remove(&key);
        }
        true
    }

    fn random_repeatable_trigger(&self, rng: &mut StdRng) -> Option<TriggerId> {
        self.repeatable_triggers.choose(rng).cloned()
    }

    fn track_repeatable_trigger(&mut self, trigger_id: TriggerId) {
        if !self.repeatable_triggers.contains(&trigger_id) {
            self.repeatable_triggers.push(trigger_id.clone());
        }
        self.trigger_repetitions.entry(trigger_id).or_insert(1);
    }

    fn pending_trigger_repetitions(&self, trigger_id: &TriggerId) -> u32 {
        self.pending_trigger_repetitions
            .get(trigger_id)
            .copied()
            .unwrap_or(0)
    }

    fn reserve_trigger_repetitions(&mut self, trigger_id: &TriggerId, amount: u32) {
        if amount == 0 {
            return;
        }
        let entry = self
            .pending_trigger_repetitions
            .entry(trigger_id.clone())
            .or_insert(0);
        *entry = entry.saturating_add(amount);
    }

    fn release_trigger_repetitions_reservation(&mut self, trigger_id: &TriggerId, amount: u32) {
        if amount == 0 {
            return;
        }
        let Some(entry) = self.pending_trigger_repetitions.get_mut(trigger_id) else {
            return;
        };
        *entry = entry.saturating_sub(amount);
        if *entry == 0 {
            self.pending_trigger_repetitions.remove(trigger_id);
        }
    }

    fn bump_domain(&mut self) -> u64 {
        let value = self.counters.domain;
        self.counters.domain += 1;
        value
    }

    fn bump_account(&mut self) -> u64 {
        let value = self.counters.account;
        self.counters.account += 1;
        value
    }

    fn bump_uaid(&mut self) -> u64 {
        let value = self.counters.uaid;
        self.counters.uaid += 1;
        value
    }

    fn bump_trigger(&mut self) -> u64 {
        let value = self.counters.trigger;
        self.counters.trigger += 1;
        value
    }

    fn bump_role(&mut self) -> u64 {
        let value = self.counters.role;
        self.counters.role += 1;
        value
    }

    fn bump_asset_definition(&mut self) -> u64 {
        let value = self.counters.asset_definition;
        self.counters.asset_definition += 1;
        value
    }

    fn bump_nft(&mut self) -> u64 {
        let value = self.counters.nft;
        self.counters.nft += 1;
        value
    }

    fn bump_metadata(&mut self) -> u64 {
        let value = self.counters.metadata;
        self.counters.metadata += 1;
        value
    }

    fn bump_invalid(&mut self) -> u64 {
        let value = self.counters.invalid;
        self.counters.invalid += 1;
        value
    }

    fn bump_staking(&mut self) -> u64 {
        let value = self.counters.staking;
        self.counters.staking += 1;
        value
    }

    fn bump_settlement(&mut self) -> u64 {
        let value = self.counters.settlement;
        self.counters.settlement += 1;
        value
    }

    fn bump_replication(&mut self) -> u64 {
        let value = self.counters.replication;
        self.counters.replication += 1;
        value
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::isi::{
        MintBox, RegisterBox, RemoveKeyValueBox, SetKeyValueBox, UnregisterBox,
    };
    use norito::codec::Decode;
    use rand::SeedableRng;
    use tokio::runtime::Builder;

    use super::*;
    use crate::config::{NexusProfile, WorkloadProfile};

    #[test]
    fn json_pair_builds_object() {
        let value = json_pair("answer", 42u64);
        let mut expected = JsonMap::new();
        expected.insert("answer".to_string(), JsonValue::from(42u64));
        assert_eq!(value, JsonValue::Object(expected));
    }

    #[test]
    fn prepare_state_builds_genesis() {
        let prepared =
            prepare_state(4, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        assert!(!prepared.genesis.is_empty());
        assert!(!prepared.genesis[0].is_empty());
        assert!(prepared.state.users.len() >= 3);
        let expected: DomainId = "chaosnet".parse().unwrap();
        assert_eq!(prepared.state.base_domain(), &expected);
    }

    #[test]
    fn prepare_state_grants_treasury_mint_permission() {
        let prepared =
            prepare_state(4, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let base_asset = prepared.state.asset_numeric.clone();
        let treasury_id = prepared.state.treasury.id.clone();
        let has_grant = prepared.genesis.iter().flatten().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<GrantBox>()
                .is_some_and(|grant| match grant {
                    GrantBox::Permission(permission_grant) => {
                        permission_grant.object
                            == CanMintAssetWithDefinition {
                                asset_definition: base_asset.clone(),
                            }
                            .into()
                            && permission_grant.destination == treasury_id
                    }
                    _ => false,
                })
        });
        assert!(
            has_grant,
            "treasury should be able to mint assets for the base definition"
        );
    }

    #[test]
    fn nexus_staking_genesis_registers_fee_sink_and_validators() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { state, genesis, .. } =
            prepare_state(2, None, Some(&profile), WorkloadProfile::Stable, false)
                .expect("state prepared");
        let setup = state
            .nexus_staking
            .as_ref()
            .expect("nexus staking setup should be present");
        assert!(
            !setup.validator_accounts.is_empty(),
            "validator accounts should be provisioned"
        );

        let expected_label = AccountLabel::new(
            "ivm".parse().expect("ivm domain"),
            "gas".parse().expect("gas label"),
        );
        let mut found_fee_sink = false;
        for instruction in genesis.iter().flatten() {
            let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() else {
                continue;
            };
            if let RegisterBox::Account(account) = register {
                if account.object.label() == Some(&expected_label) {
                    found_fee_sink = true;
                    break;
                }
            }
        }
        assert!(found_fee_sink, "fee sink account should be registered");

        let post_topology = npos_post_topology_instructions(setup.validator_accounts.len());
        let mut registered_validators = HashSet::new();
        let mut activated_validators = HashSet::new();
        for instruction in &post_topology {
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
            {
                registered_validators.insert(register.validator.clone());
            }
            if let Some(activate) = instruction
                .as_any()
                .downcast_ref::<ActivatePublicLaneValidator>()
            {
                activated_validators.insert(activate.validator.clone());
            }
        }

        for validator in &setup.validator_accounts {
            let registered = genesis.iter().flatten().any(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterBox>()
                    .is_some_and(|register| match register {
                        RegisterBox::Account(account) => account.object.id == validator.id,
                        _ => false,
                    })
            });
            assert!(
                registered,
                "validator account {} should be registered in genesis",
                validator.id
            );
            assert!(
                registered_validators.contains(&validator.id),
                "validator {} should be registered for staking in genesis",
                validator.id
            );
            assert!(
                activated_validators.contains(&validator.id),
                "validator {} should be activated in genesis",
                validator.id
            );
        }
    }

    #[test]
    fn nexus_profile_injects_additional_recipes() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { recipes, .. } =
            prepare_state(3, None, Some(&profile), WorkloadProfile::Chaos, false)
                .expect("state prepared");
        assert!(
            recipes
                .iter()
                .any(|kind| matches!(kind, RecipeKind::RegisterPublicLaneValidator)),
            "nexus recipes should include staking paths"
        );
    }

    #[test]
    fn stable_recipes_skip_contract_deploys_by_default() {
        let PreparedChaos { recipes, .. } =
            prepare_state(2, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        assert!(
            !recipes
                .iter()
                .any(|kind| matches!(kind, RecipeKind::DeployIvmContract)),
            "stable recipes should skip IVM contract deployment by default"
        );
        assert!(
            !recipes
                .iter()
                .any(|kind| matches!(kind, RecipeKind::DeployKotodamaContract)),
            "stable recipes should skip Kotodama contract deployment by default"
        );
    }

    #[test]
    fn stable_recipes_allow_contract_deploys_when_enabled() {
        let PreparedChaos { recipes, .. } =
            prepare_state(2, None, None, WorkloadProfile::Stable, true).expect("state prepared");
        assert!(
            recipes
                .iter()
                .any(|kind| matches!(kind, RecipeKind::DeployIvmContract)),
            "stable recipes should include IVM contract deployment when enabled"
        );
        assert!(
            recipes
                .iter()
                .any(|kind| matches!(kind, RecipeKind::DeployKotodamaContract)),
            "stable recipes should include Kotodama contract deployment when enabled"
        );
    }

    #[test]
    fn base_recipes_include_asset_instance_metadata() {
        assert!(
            BASE_RECIPES_STABLE
                .iter()
                .any(|kind| matches!(kind, RecipeKind::SetAssetInstanceKeyValue))
        );
        assert!(
            BASE_RECIPES_STABLE
                .iter()
                .any(|kind| matches!(kind, RecipeKind::RemoveAssetInstanceKeyValue))
        );
        assert!(
            BASE_RECIPES_CHAOS
                .iter()
                .any(|kind| matches!(kind, RecipeKind::SetAssetInstanceKeyValue))
        );
        assert!(
            BASE_RECIPES_CHAOS
                .iter()
                .any(|kind| matches!(kind, RecipeKind::RemoveAssetInstanceKeyValue))
        );
        assert!(
            BASE_RECIPES_STABLE
                .iter()
                .any(|kind| matches!(kind, RecipeKind::MintTriggerRepetitions))
        );
        assert!(
            BASE_RECIPES_STABLE
                .iter()
                .any(|kind| matches!(kind, RecipeKind::BurnTriggerRepetitions))
        );
        assert!(
            BASE_RECIPES_STABLE
                .iter()
                .any(|kind| matches!(kind, RecipeKind::DeployIvmContract))
        );
        assert!(
            BASE_RECIPES_STABLE
                .iter()
                .any(|kind| matches!(kind, RecipeKind::DeployKotodamaContract))
        );
        assert!(
            BASE_RECIPES_CHAOS
                .iter()
                .any(|kind| matches!(kind, RecipeKind::DeployKotodamaContract))
        );
    }

    #[test]
    fn produce_plan_for_all_recipes() {
        let PreparedChaos {
            mut state, recipes, ..
        } = prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(7);
        for kind in recipes {
            let plan = state.produce_plan(kind, &mut rng).expect("plan builds");
            assert!(
                !plan.instructions.is_empty(),
                "{kind:?} produced empty plan"
            );
        }
    }

    #[test]
    fn workload_engine_grant_role_never_duplicates_memberships() {
        let PreparedChaos { state, recipes, .. } =
            prepare_state(4, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let engine = WorkloadEngine::new(state, recipes);
        let mut rng = StdRng::seed_from_u64(17);
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime builds");

        runtime.block_on(async {
            let mut guard = engine.state.lock().await;
            let _ = guard.plan_register_role().expect("role prepared");
        });
        engine.set_recipe_override(Some(RecipeKind::GrantRole));

        let membership_count = |engine: &WorkloadEngine| -> usize {
            runtime.block_on(async {
                let guard = engine.state.lock().await;
                guard.role_memberships.values().map(HashSet::len).sum()
            })
        };

        let available_accounts = runtime.block_on(async {
            let guard = engine.state.lock().await;
            let total_accounts = guard.users.len() + 1;
            total_accounts.saturating_sub(guard.role_memberships.values().map(HashSet::len).sum())
        });

        for _ in 0..available_accounts {
            let before = membership_count(&engine);
            let plan = runtime
                .block_on(engine.next_plan(&mut rng))
                .expect("plan builds");
            assert_eq!(plan.label, "grant_role");
            assert!(plan.expect_success, "grant should succeed for new accounts");
            let after = membership_count(&engine);
            assert_eq!(after, before + 1, "grant must add a new membership");
        }

        let before = membership_count(&engine);
        let plan = runtime
            .block_on(engine.next_plan(&mut rng))
            .expect("plan builds");
        assert_eq!(plan.label, "grant_role");
        assert!(
            !plan.expect_success,
            "grant should downgrade once all accounts have the role"
        );
        let after = membership_count(&engine);
        assert_eq!(after, before, "failing grant must not change memberships");
    }

    #[test]
    fn publish_manifest_tracks_dataspaces() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(2, None, Some(&profile), WorkloadProfile::Stable, false)
                .expect("state prepared");
        let mut rng = StdRng::seed_from_u64(99);
        let plan = state
            .plan_publish_space_manifest(&mut rng)
            .expect("manifest plan builds");
        assert_eq!(plan.label, "publish_space_directory_manifest");
        assert!(
            state
                .space_directory_manifests
                .values()
                .any(|spaces| !spaces.is_empty()),
            "dataspace tracking should record published manifests"
        );
    }

    #[test]
    fn staking_recipes_track_validator_registry() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, Some(&profile), WorkloadProfile::Stable, false)
                .expect("state prepared");
        state.public_lane_validators.clear();
        state.public_lane_stakes.clear();
        let mut rng = StdRng::seed_from_u64(31);
        let plan = state
            .plan_register_public_validator(&mut rng)
            .expect("validator plan builds");
        assert_eq!(plan.label, "register_public_lane_validator");
        assert!(
            plan.expect_success,
            "staking should be provisioned in genesis"
        );
        let has_validators = state
            .public_lane_validators
            .values()
            .any(|validators| !validators.is_empty());
        assert_eq!(
            has_validators, plan.expect_success,
            "validator registry tracking should follow plan success"
        );

        let register = plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterPublicLaneValidator>()
            })
            .expect("register validator instruction");
        assert_eq!(
            register.validator, register.stake_account,
            "validator registration should self-stake in Izanami genesis"
        );
        let stake_asset = state
            .nexus_staking
            .as_ref()
            .expect("staking setup")
            .stake_asset
            .clone();
        let mint_asset = plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<MintBox>()
                    .and_then(|mint| match mint {
                        MintBox::Asset(asset) => Some(asset.destination.definition().clone()),
                        _ => None,
                    })
            })
            .expect("mint instruction");
        assert_eq!(mint_asset, stake_asset);
        assert!(
            !state.public_lane_stakes.is_empty(),
            "stake shares should be tracked after validator registration"
        );
    }

    #[test]
    fn bond_public_stake_tracks_share_and_uses_stake_asset() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, Some(&profile), WorkloadProfile::Stable, false)
                .expect("state prepared");
        let mut rng = StdRng::seed_from_u64(41);
        assert!(
            state
                .public_lane_validators
                .values()
                .any(|validators| !validators.is_empty()),
            "genesis should seed public lane validators"
        );
        let before_shares = state.public_lane_stakes.len();
        let plan = state.plan_bond_public_stake(&mut rng).expect("bond plan");
        assert_eq!(plan.label, "bond_public_lane_stake");
        assert!(plan.expect_success, "bond plan should succeed");
        assert!(
            state.public_lane_stakes.len() >= before_shares,
            "bond should add or update stake shares"
        );
        let stake_asset = state
            .nexus_staking
            .as_ref()
            .expect("staking setup")
            .stake_asset
            .clone();
        let mint_asset = plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<MintBox>()
                    .and_then(|mint| match mint {
                        MintBox::Asset(asset) => Some(asset.destination.definition().clone()),
                        _ => None,
                    })
            })
            .expect("mint instruction");
        assert_eq!(mint_asset, stake_asset);
    }

    #[test]
    fn replication_orders_are_tracked() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(2, None, Some(&profile), WorkloadProfile::Stable, false)
                .expect("state prepared");
        let mut rng = StdRng::seed_from_u64(45);
        let plan = state
            .plan_issue_replication_order(&mut rng)
            .expect("replication plan builds");
        assert_eq!(plan.label, "seed_replication_manifest");
        assert!(
            state.pending_replication_orders.is_empty(),
            "replication orders should not be tracked before issuance"
        );
        let plan = state
            .plan_issue_replication_order(&mut rng)
            .expect("replication plan builds");
        assert_eq!(plan.label, "issue_replication_order");
        assert!(
            !state.pending_replication_orders.is_empty(),
            "pending replication orders should be tracked"
        );
    }

    #[test]
    fn replication_order_payload_references_seeded_manifest() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(2, None, Some(&profile), WorkloadProfile::Stable, false)
                .expect("state prepared");
        let mut rng = StdRng::seed_from_u64(52);
        let seed = state
            .plan_issue_replication_order(&mut rng)
            .expect("replication seed builds");
        assert_eq!(seed.label, "seed_replication_manifest");
        let plan = state
            .plan_issue_replication_order(&mut rng)
            .expect("replication plan builds");
        assert_eq!(plan.label, "issue_replication_order");
        let instruction = plan
            .instructions
            .first()
            .expect("replication order instruction");
        let issue = instruction
            .as_any()
            .downcast_ref::<IssueReplicationOrder>()
            .expect("issue replication order");
        let decoded = ReplicationOrderV1::decode(&mut issue.order_payload.as_slice())
            .expect("decode payload");
        let seed = state.sorafs_replication.as_ref().expect("replication seed");
        assert_eq!(decoded.manifest_digest, *seed.manifest_digest.as_bytes());
        assert_eq!(decoded.chunking_profile, seed.chunker.to_handle());
        let assignment = decoded.assignments.first().expect("assignment");
        assert_eq!(assignment.provider_id, *seed.provider_id.as_bytes());
    }

    #[test]
    fn complete_replication_order_clears_pending() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(2, None, Some(&profile), WorkloadProfile::Stable, false)
                .expect("state prepared");
        let mut rng = StdRng::seed_from_u64(91);
        let seed = state
            .plan_issue_replication_order(&mut rng)
            .expect("replication seed builds");
        assert_eq!(seed.label, "seed_replication_manifest");
        let plan = state
            .plan_issue_replication_order(&mut rng)
            .expect("replication order builds");
        assert_eq!(plan.label, "issue_replication_order");
        assert_eq!(state.pending_replication_orders.len(), 1);
        let complete = state
            .plan_complete_replication_order(&mut rng)
            .expect("completion plan builds");
        assert_eq!(complete.label, "complete_replication_order");
        assert!(
            state.pending_replication_orders.is_empty(),
            "completion should clear the pending order"
        );
    }

    #[test]
    fn dvp_settlement_plan_builds() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(77);
        let plan = state
            .plan_dvp_settlement(&mut rng)
            .expect("dvp plan builds");
        assert_eq!(plan.label, "dvp_settlement");
        assert!(plan.expect_success);
        let settlement = plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<SettlementInstructionBox>()
            })
            .expect("settlement instruction");
        match settlement {
            SettlementInstructionBox::Dvp(isi) => {
                assert_eq!(isi.delivery_leg.from, plan.signer.id);
            }
            SettlementInstructionBox::Pvp(_) => {
                panic!("expected DvP settlement instruction");
            }
        }
    }

    #[test]
    fn public_unbond_tracks_pending_requests() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, Some(&profile), WorkloadProfile::Stable, false)
                .expect("state prepared");
        let mut rng = StdRng::seed_from_u64(51);
        let before_state = state.clone();
        let plan = state
            .plan_schedule_public_unbond(&mut rng)
            .expect("unbond plan");
        assert_eq!(plan.label, "schedule_public_lane_unbond");
        assert_eq!(
            !state.pending_unbonds.is_empty(),
            plan.expect_success,
            "pending unbond tracking should follow plan success"
        );
        if plan.expect_success {
            let schedule = plan
                .instructions
                .iter()
                .find_map(|instruction| {
                    instruction
                        .as_any()
                        .downcast_ref::<SchedulePublicLaneUnbond>()
                })
                .expect("schedule unbond instruction");
            let before_share = before_state.available_public_lane_stake_share(
                schedule.lane_id,
                &schedule.validator,
                &schedule.staker,
            );
            let after_share = state.available_public_lane_stake_share(
                schedule.lane_id,
                &schedule.validator,
                &schedule.staker,
            );
            assert!(
                after_share < before_share,
                "successful unbond should reduce tracked stake share"
            );
        }
    }

    #[test]
    fn public_rewards_follow_validator_registry() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, Some(&profile), WorkloadProfile::Stable, false)
                .expect("state prepared");
        let mut rng = StdRng::seed_from_u64(61);
        let plan = state
            .plan_record_public_rewards(&mut rng)
            .expect("reward plan");
        assert_eq!(plan.label, "record_public_lane_rewards");
        assert!(plan.expect_success, "staking genesis should enable rewards");
        let setup = state.nexus_staking.as_ref().expect("staking setup");
        let expected_reward_asset = AssetId::new(setup.fee_asset.clone(), setup.fee_sink.clone());
        let reward = plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RecordPublicLaneRewards>()
            })
            .expect("record rewards instruction");
        assert_eq!(reward.reward_asset, expected_reward_asset);
        let minted = plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<MintBox>()
                    .and_then(|mint| match mint {
                        MintBox::Asset(asset) => Some(asset.destination.clone()),
                        _ => None,
                    })
            })
            .expect("mint reward instruction");
        assert_eq!(minted, expected_reward_asset);
    }

    #[test]
    fn asset_definition_register_and_unregister_moves_tracking() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(11);
        let before = state.asset_definitions_unclaimed.len();
        let register_plan = state
            .plan_register_asset_definition()
            .expect("register definition");
        assert_eq!(register_plan.label, "register_asset_definition");
        assert_eq!(
            state.asset_definitions_unclaimed.len(),
            before + 1,
            "new definition should be tracked as unclaimed"
        );
        let unregister_plan = state.plan_unregister_asset_definition(&mut rng);
        assert!(
            unregister_plan.expect_success,
            "recent definition should be removable"
        );
        assert!(
            state.asset_definitions_unclaimed.len() <= before,
            "unclaimed pool should shrink after removal"
        );
    }

    #[test]
    fn unregister_asset_definition_fallback_uses_missing_definition() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        assert!(state.asset_definitions_unclaimed.is_empty());
        let before = state.asset_definitions.clone();
        let mut rng = StdRng::seed_from_u64(19);
        let plan = state.plan_unregister_asset_definition(&mut rng);
        assert_eq!(plan.label, "unregister_asset_definition");
        assert!(!plan.expect_success, "fallback should expect failure");
        assert_eq!(
            state.asset_definitions, before,
            "fallback should not mutate tracked definitions"
        );
        let target = plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<UnregisterBox>()
                    .and_then(|unregister| match unregister {
                        UnregisterBox::AssetDefinition(inner) => Some(inner.object.clone()),
                        _ => None,
                    })
            })
            .expect("unregister asset definition instruction");
        assert!(
            !before.contains(&target),
            "fallback should use a missing asset definition id"
        );
    }

    #[test]
    fn nft_registration_and_transfer_updates_owner() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(29);
        let register_plan = state.plan_register_nft(&mut rng).expect("register nft");
        assert_eq!(register_plan.label, "register_nft");
        assert_eq!(state.nft_holdings.len(), 1, "nft holding should be tracked");
        let before_ids: HashSet<NftId> = state.nft_holdings.keys().cloned().collect();
        let transfer_plan = state.plan_transfer_nft(&mut rng).expect("transfer nft");
        assert_eq!(transfer_plan.label, "transfer_nft");
        let after_ids: HashSet<NftId> = state.nft_holdings.keys().cloned().collect();
        assert_eq!(
            after_ids.len(),
            before_ids.len() + 1,
            "transfer should register a new nft entry"
        );
        let new_id = after_ids
            .difference(&before_ids)
            .next()
            .expect("new nft id should be recorded");
        let new_owner = state
            .nft_holdings
            .get(new_id)
            .expect("new owner should be tracked");
        assert_ne!(
            new_owner, &state.treasury.id,
            "transfer should move newly minted nft away from treasury"
        );
    }

    #[test]
    fn trigger_repetition_mint_and_burn_balance() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(93);
        let trigger_id = loop {
            let mint_plan = state
                .plan_mint_trigger_repetitions(&mut rng)
                .expect("mint repetitions");
            assert_eq!(mint_plan.label, "mint_trigger_repetitions");
            mint_plan.apply_updates(&mut state, true);
            if let Some((id, amount)) = state
                .trigger_repetitions
                .iter()
                .next()
                .map(|(id, amount)| (id.clone(), *amount))
                && amount > 1
            {
                break id;
            }
        };
        let minted = state
            .trigger_repetitions
            .get(&trigger_id)
            .copied()
            .unwrap_or_default();
        assert!(
            minted > 1,
            "mint should provide enough repetitions for burn"
        );
        let burn_plan = loop {
            let plan = state
                .plan_burn_trigger_repetitions(&mut rng)
                .expect("burn repetitions");
            if plan.label == "burn_trigger_repetitions" {
                break plan;
            }
            assert_eq!(plan.label, "mint_trigger_repetitions");
            plan.apply_updates(&mut state, true);
        };
        assert_eq!(burn_plan.label, "burn_trigger_repetitions");
        burn_plan.apply_updates(&mut state, true);
        let remaining = state
            .trigger_repetitions
            .get(&trigger_id)
            .copied()
            .unwrap_or_default();
        assert!(remaining <= minted, "burn must not increase repetitions");
    }

    #[test]
    fn mint_trigger_repetitions_registers_repeatable_trigger() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(111);
        let pipeline = state
            .plan_pipeline_trigger(&mut rng)
            .expect("pipeline trigger");
        assert_eq!(pipeline.label, "register_pipeline_trigger");
        assert!(state.repeatable_triggers.is_empty());

        let plan = state
            .plan_mint_trigger_repetitions(&mut rng)
            .expect("mint plan");
        let trigger = plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterBox>()
                    .and_then(|register| match register {
                        RegisterBox::Trigger(registration) => Some(&registration.object),
                        _ => None,
                    })
            })
            .expect("repeatable trigger registration");
        assert!(matches!(trigger.action().repeats(), Repeats::Exactly(_)));
        assert!(matches!(
            trigger.action().filter(),
            EventFilterBox::ExecuteTrigger(_)
        ));
    }

    #[test]
    fn mint_trigger_repetitions_plan_exposes_target() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(222);

        let register_plan = state
            .plan_mint_trigger_repetitions(&mut rng)
            .expect("register repeatable trigger");
        register_plan.apply_updates(&mut state, true);

        let mint_plan = state
            .plan_mint_trigger_repetitions(&mut rng)
            .expect("mint repetitions");
        let (trigger_id, amount) = mint_plan
            .mint_trigger_repetitions()
            .expect("mint plan should contain trigger repetitions");
        assert!(
            state.repeatable_triggers.contains(&trigger_id),
            "mint plan should target a tracked trigger"
        );
        assert!(amount > 0, "mint amount should be positive");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workload_record_result_applies_updates_on_success() {
        let PreparedChaos {
            state,
            genesis: _,
            recipes,
        } = prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let engine = WorkloadEngine::new(state, recipes);
        engine.set_recipe_override(Some(RecipeKind::SetTriggerKeyValue));
        let mut rng = StdRng::seed_from_u64(211);
        let plan = engine.next_plan(&mut rng).await.expect("plan built");
        {
            let guard = engine.state.lock().await;
            assert!(
                guard.registered_triggers.is_empty(),
                "plans should not mutate trigger state before success"
            );
        }
        engine.record_result(&plan, false).await;
        {
            let guard = engine.state.lock().await;
            assert!(
                guard.registered_triggers.is_empty(),
                "failed submissions should not update trigger state"
            );
        }
        engine.record_result(&plan, true).await;
        {
            let guard = engine.state.lock().await;
            assert_eq!(
                guard.registered_triggers.len(),
                1,
                "successful submissions should apply trigger updates"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workload_record_result_releases_trigger_reservation_on_failure() {
        let PreparedChaos {
            mut state,
            genesis: _,
            recipes,
        } = prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let trigger_id: TriggerId = "repeatable_trigger_failure"
            .parse()
            .expect("valid trigger id");
        state.repeatable_triggers.push(trigger_id.clone());
        state.trigger_repetitions.insert(trigger_id.clone(), 2);
        let engine = WorkloadEngine::new(state, recipes);
        engine.set_recipe_override(Some(RecipeKind::BurnTriggerRepetitions));
        let mut rng = StdRng::seed_from_u64(412);
        let plan = engine.next_plan(&mut rng).await.expect("burn plan");
        {
            let guard = engine.state.lock().await;
            assert!(
                guard.pending_trigger_repetitions.contains_key(&trigger_id),
                "burn plan should reserve repetitions"
            );
        }

        engine.record_result(&plan, false).await;
        {
            let guard = engine.state.lock().await;
            assert_eq!(
                guard
                    .pending_trigger_repetitions
                    .get(&trigger_id)
                    .copied()
                    .unwrap_or(0),
                0,
                "failed submissions should release trigger reservations"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workload_record_result_releases_pending_on_failure() {
        let PreparedChaos {
            mut state,
            genesis: _,
            recipes,
        } = prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let trigger_id: TriggerId = "repeatable_trigger_pending_failure"
            .parse()
            .expect("valid trigger id");
        state
            .pending_trigger_repetitions
            .insert(trigger_id.clone(), 2);
        let plan = TransactionPlan {
            label: "release_pending_trigger_repetitions",
            instructions: Vec::new(),
            signer: state.treasury.clone(),
            expect_success: false,
            state_updates: vec![PlanUpdate::ReleaseTriggerRepetitionsReservation {
                trigger_id: trigger_id.clone(),
                amount: 2,
            }],
        };
        let engine = WorkloadEngine::new(state, recipes);
        engine.record_result(&plan, false).await;
        let guard = engine.state.lock().await;
        assert!(
            guard
                .pending_trigger_repetitions
                .get(&trigger_id)
                .copied()
                .unwrap_or(0)
                == 0,
            "failed submissions should release pending burn reservations"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workload_sync_trigger_repetitions_updates_tracking() {
        let PreparedChaos {
            state,
            genesis: _,
            recipes,
        } = prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let engine = WorkloadEngine::new(state, recipes);
        let trigger_id: TriggerId = "repeatable_trigger_sync".parse().expect("valid trigger id");

        engine.sync_trigger_repetitions(&trigger_id, Some(3)).await;
        {
            let guard = engine.state.lock().await;
            assert_eq!(
                guard
                    .trigger_repetitions
                    .get(&trigger_id)
                    .copied()
                    .unwrap_or(0),
                3,
                "sync should update tracked repetitions"
            );
            assert!(
                guard.repeatable_triggers.contains(&trigger_id),
                "sync should track repeatable trigger ids"
            );
        }

        engine.sync_trigger_repetitions(&trigger_id, None).await;
        let guard = engine.state.lock().await;
        assert!(
            !guard.repeatable_triggers.contains(&trigger_id),
            "sync removal should untrack repeatable trigger ids"
        );
        assert!(
            !guard.trigger_repetitions.contains_key(&trigger_id),
            "sync removal should clear repetition tracking"
        );
    }

    #[test]
    fn repeatable_trigger_tracking_dedupes_ids() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let trigger_id: TriggerId = "repeatable_trigger_dedupe"
            .parse()
            .expect("valid trigger id");
        state.track_repeatable_trigger(trigger_id.clone());
        state.track_repeatable_trigger(trigger_id);
        assert_eq!(
            state.repeatable_triggers.len(),
            1,
            "repeatable trigger tracking should not duplicate ids"
        );
    }

    #[test]
    fn burn_trigger_repetitions_keeps_trigger_alive() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(141);
        let mint_plan = state
            .plan_mint_trigger_repetitions(&mut rng)
            .expect("mint plan");
        mint_plan.apply_updates(&mut state, true);
        let trigger_id = state
            .repeatable_triggers
            .first()
            .cloned()
            .expect("repeatable trigger");
        state.trigger_repetitions.insert(trigger_id.clone(), 2);

        let burn = state
            .plan_burn_trigger_repetitions(&mut rng)
            .expect("burn plan");
        assert_eq!(burn.label, "burn_trigger_repetitions");
        let (_, amount) = burn.burn_trigger_repetitions().expect("burn plan amount");
        assert!(amount < 2, "burn should leave at least one repetition");
        burn.apply_updates(&mut state, true);
        assert!(
            state.repeatable_triggers.contains(&trigger_id),
            "burn should not remove repeatable trigger"
        );
        assert!(
            state
                .trigger_repetitions
                .get(&trigger_id)
                .copied()
                .unwrap_or(0)
                >= 1,
            "burn should leave at least one repetition"
        );
    }

    #[test]
    fn burn_trigger_repetitions_releases_pending_on_failure() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let trigger_id: TriggerId = "repeatable_trigger_pending"
            .parse()
            .expect("valid trigger id");
        state.repeatable_triggers.push(trigger_id.clone());
        state.trigger_repetitions.insert(trigger_id.clone(), 2);
        let mut rng = StdRng::seed_from_u64(212);

        let burn = state
            .plan_burn_trigger_repetitions(&mut rng)
            .expect("burn plan");
        let reserved = state
            .pending_trigger_repetitions
            .get(&trigger_id)
            .copied()
            .unwrap_or(0);
        assert!(reserved > 0, "burn plan should reserve repetitions");

        burn.apply_updates(&mut state, false);
        assert_eq!(
            state
                .pending_trigger_repetitions
                .get(&trigger_id)
                .copied()
                .unwrap_or(0),
            0,
            "failed burn should release pending reservation"
        );
        assert_eq!(
            state
                .trigger_repetitions
                .get(&trigger_id)
                .copied()
                .unwrap_or(0),
            2,
            "failed burn should not change tracked repetitions"
        );
    }

    #[test]
    fn trigger_metadata_plans_do_not_track_repeatable_triggers() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(151);
        let set_plan = state
            .plan_set_trigger_key(&mut rng)
            .expect("set trigger metadata");
        assert_eq!(set_plan.label, "set_trigger_kv");
        set_plan.apply_updates(&mut state, true);
        let trigger = set_plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterBox>()
                    .and_then(|register| match register {
                        RegisterBox::Trigger(registration) => Some(&registration.object),
                        _ => None,
                    })
            })
            .expect("metadata trigger registration");
        assert!(matches!(
            trigger.action().filter(),
            EventFilterBox::ExecuteTrigger(_)
        ));
        assert!(matches!(trigger.action().repeats(), Repeats::Indefinitely));
        assert!(
            state.registered_triggers.contains(trigger.id()),
            "metadata trigger should be tracked as durable"
        );
        assert!(
            state.repeatable_triggers.is_empty(),
            "metadata trigger should not be tracked as repeatable"
        );
        let remove_plan = state
            .plan_remove_trigger_key(&mut rng)
            .expect("remove trigger metadata");
        assert_eq!(remove_plan.label, "remove_trigger_kv");
        remove_plan.apply_updates(&mut state, true);
        assert!(
            state.repeatable_triggers.is_empty(),
            "metadata trigger should not be tracked as repeatable"
        );
    }

    #[test]
    fn time_trigger_is_not_repeatable() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(161);
        let plan = state.plan_time_trigger(&mut rng).expect("time trigger");
        assert_eq!(plan.label, "register_time_trigger");
        assert!(
            state.repeatable_triggers.is_empty(),
            "time triggers should not be used for repetition mint/burn"
        );
    }

    #[test]
    fn deploy_ivm_trigger_is_not_repeatable() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(171);
        let plan = state.plan_deploy_ivm(&mut rng).expect("ivm trigger");
        assert_eq!(plan.label, "deploy_ivm_contract");
        assert!(
            state.repeatable_triggers.is_empty(),
            "IVM triggers should not be used for repetition mint/burn"
        );
    }

    #[test]
    fn execute_trigger_registers_and_executes_call_trigger() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(103);
        let plan = state
            .plan_execute_trigger(&mut rng)
            .expect("execute trigger");
        assert_eq!(plan.label, "execute_trigger");
        plan.apply_updates(&mut state, true);
        let registered = plan
            .instructions
            .iter()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterBox>()
                    .and_then(|register| match register {
                        RegisterBox::Trigger(registration) => Some(registration.object.clone()),
                        _ => None,
                    })
            })
            .expect("call trigger registration");
        let execute = plan
            .instructions
            .iter()
            .find_map(|instruction| instruction.as_any().downcast_ref::<ExecuteTrigger>())
            .expect("execute trigger instruction");
        assert_eq!(execute.trigger, *registered.id());
        assert!(
            state.call_triggers.contains(registered.id()),
            "call trigger should be tracked"
        );
    }

    #[test]
    fn asset_metadata_set_and_remove_trackers() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(55);
        let tracked_before: usize = state.asset_metadata.values().map(HashSet::len).sum();
        let set_plan = state
            .plan_set_asset_metadata(&mut rng)
            .expect("set asset metadata");
        assert_eq!(set_plan.label, "set_asset_kv");
        let tracked_after_set: usize = state.asset_metadata.values().map(HashSet::len).sum();
        assert!(
            tracked_after_set >= tracked_before,
            "setting metadata should not reduce tracked keys"
        );
        let remove_plan = state
            .plan_remove_asset_metadata(&mut rng)
            .expect("remove asset metadata");
        assert_eq!(remove_plan.label, "remove_asset_kv");
        let tracked_after_remove: usize = state.asset_metadata.values().map(HashSet::len).sum();
        assert!(
            tracked_after_remove <= tracked_after_set,
            "removal should not grow metadata tracking"
        );
    }

    #[test]
    fn account_metadata_plans_use_account_signer() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(101);
        let set_plan = state.plan_set_key(&mut rng).expect("set key plan");
        let set_target = set_plan
            .instructions
            .iter()
            .find_map(
                |instr| match instr.as_any().downcast_ref::<SetKeyValueBox>() {
                    Some(SetKeyValueBox::Account(set)) => Some(set.object.clone()),
                    _ => None,
                },
            )
            .expect("set key instruction should target an account");
        assert_eq!(
            set_plan.signer.id, set_target,
            "set key plan should be signed by the account owner"
        );

        let remove_plan = state.plan_remove_key(&mut rng).expect("remove key plan");
        let remove_target = remove_plan
            .instructions
            .iter()
            .find_map(
                |instr| match instr.as_any().downcast_ref::<RemoveKeyValueBox>() {
                    Some(RemoveKeyValueBox::Account(remove)) => Some(remove.object.clone()),
                    _ => None,
                },
            )
            .expect("remove key instruction should target an account");
        assert_eq!(
            remove_plan.signer.id, remove_target,
            "remove key plan should be signed by the account owner"
        );
    }

    #[test]
    fn account_metadata_set_and_remove_trackers() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        assert!(state.account_metadata.is_empty());
        let mut rng = StdRng::seed_from_u64(205);
        let set_plan = state.plan_set_key(&mut rng).expect("set key plan");
        assert_eq!(set_plan.label, "set_account_kv");
        let tracked_after_set: usize = state.account_metadata.values().map(HashSet::len).sum();
        assert!(tracked_after_set > 0, "set key should be tracked");

        let remove_plan = state.plan_remove_key(&mut rng).expect("remove key plan");
        assert_eq!(remove_plan.label, "remove_account_kv");
        let tracked_after_remove: usize = state.account_metadata.values().map(HashSet::len).sum();
        assert!(
            tracked_after_remove <= tracked_after_set,
            "removal should not increase tracked keys"
        );
    }

    #[test]
    fn register_uaid_account_tracks_mapping() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let plan = state.plan_register_uaid_account();
        assert_eq!(plan.label, "register_uaid_account");
        assert!(
            state
                .uaid_accounts
                .values()
                .any(|record| record.uaid.is_some()),
            "UAID registry should record the new account"
        );
    }

    #[test]
    fn publish_manifest_records_dataspace() {
        let PreparedChaos { mut state, .. } =
            prepare_state(2, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(99);
        let plan = state
            .plan_publish_space_manifest(&mut rng)
            .expect("manifest plan builds");
        assert_eq!(plan.label, "publish_space_directory_manifest");
        assert!(
            state
                .space_directory_manifests
                .values()
                .any(|spaces| spaces.contains(&DataSpaceId::GLOBAL)),
            "dataspace should be recorded as published"
        );
    }

    #[test]
    fn revoke_manifest_without_entry_sets_failure() {
        let PreparedChaos { mut state, .. } =
            prepare_state(2, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(3);
        let plan = state
            .plan_revoke_space_manifest(&mut rng)
            .expect("revoke plan builds");
        assert!(
            !plan.expect_success,
            "revocation without manifest should fail"
        );
    }

    #[test]
    fn domain_metadata_roundtrip_clears_tracking() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let set_plan = state.plan_set_domain_key().expect("set domain metadata");
        assert_eq!(set_plan.label, "set_domain_kv");
        assert!(
            !state.domain_metadata.is_empty(),
            "domain metadata should be tracked after setting"
        );
        let remove_plan = state
            .plan_remove_domain_key()
            .expect("remove domain metadata");
        assert_eq!(remove_plan.label, "remove_domain_kv");
        // Removal path may leave map empty or retain other keys, but it must not panic.
    }
}
