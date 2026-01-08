//! Workload generation utilities for Izanami, covering diverse ISI mixes and trigger flavours.

#[cfg(test)]
use std::sync::Mutex as StdMutex;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use color_eyre::{Result, eyre::eyre};
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    account::NewAccount,
    events::{
        EventFilterBox,
        pipeline::{PipelineEventFilterBox, TransactionEventFilter},
        prelude::{AccountEventFilter, AccountEventSet, DataEventFilter, TimeEventFilter},
        time::{ExecutionTime, Schedule},
    },
    isi::{
        RemoveAssetKeyValue, SetAssetKeyValue,
        settlement::{
            DvpIsi, SettlementId, SettlementInstructionBox, SettlementLeg, SettlementPlan,
        },
        sorafs::{CompleteReplicationOrder, IssueReplicationOrder},
        space_directory::{
            ExpireSpaceDirectoryManifest, PublishSpaceDirectoryManifest,
            RevokeSpaceDirectoryManifest,
        },
        staking::{
            BondPublicLaneStake, FinalizePublicLaneUnbond, RecordPublicLaneRewards,
            RegisterPublicLaneValidator, SchedulePublicLaneUnbond, SlashPublicLaneValidator,
        },
    },
    metadata::Metadata,
    nexus::{
        Allowance, AllowanceWindow, AssetPermissionManifest, CapabilityScope, DataSpaceId, LaneId,
        ManifestEffect, ManifestEntry, ManifestVersion, PublicLaneRewardRole,
        PublicLaneRewardShare, UniversalAccountId,
    },
    prelude::*,
    sorafs::pin_registry::ReplicationOrderId,
    trigger::{
        Trigger,
        action::{Action, Repeats},
    },
};
use iroha_executor_data_model::permission::{
    account::CanModifyAccountMetadata, nexus::CanPublishSpaceDirectoryManifest,
};
use norito::{
    codec::Encode as NoritoEncode,
    json::{Map as JsonMap, Value as JsonValue},
};
use rand::{Rng, RngCore, rngs::StdRng, seq::IndexedRandom};
use sorafs_manifest::pin_registry::ReplicationOrderV1;
use tokio::sync::Mutex;

use crate::smart_contracts;

/// Record describing an account and its signing material.
#[derive(Clone, Debug)]
pub struct AccountRecord {
    pub id: AccountId,
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

/// Prepared chaos state with pre-built genesis instructions.
#[derive(Debug)]
pub struct PreparedChaos {
    pub state: ChaosState,
    pub genesis: Vec<Vec<InstructionBox>>,
    pub recipes: Vec<RecipeKind>,
}

/// Create the baseline chaos state and its associated genesis block.
pub fn prepare_state(
    account_count: usize,
    nexus: Option<&crate::config::NexusProfile>,
) -> Result<PreparedChaos> {
    let effective_accounts = account_count.max(3);
    let base_domain: DomainId = "chaosnet"
        .parse()
        .map_err(|_| eyre!("invalid base domain"))?;
    let domain_name = base_domain.name.to_string();

    let treasury_key = KeyPair::random();
    let treasury_id = AccountId::new(base_domain.clone(), treasury_key.public_key().clone());
    let treasury_uaid = UniversalAccountId::from_hash(Hash::new(b"izanami-chaos-treasury-uaid"));
    let treasury = AccountRecord {
        id: treasury_id,
        key_pair: treasury_key,
        uaid: Some(treasury_uaid),
    };

    let mut users = Vec::with_capacity(effective_accounts);
    for _ in 0..effective_accounts {
        let key = KeyPair::random();
        let account_id = AccountId::new(base_domain.clone(), key.public_key().clone());
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
    genesis_tx.push(InstructionBox::from(Grant::account_permission(
        CanPublishSpaceDirectoryManifest {
            dataspace: DataSpaceId::GLOBAL,
        },
        treasury.id.clone(),
    )));
    let initial_float: Numeric = 1_000_000_000_u64.into();
    let treasury_asset_id = AssetId::new(asset_numeric_id.clone(), treasury.id.clone());
    genesis_tx.push(InstructionBox::from(Mint::asset_numeric(
        initial_float,
        treasury_asset_id.clone(),
    )));

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

    let mut state = ChaosState::new(
        base_domain.clone(),
        treasury,
        users,
        asset_numeric_id,
        asset_nft_id,
        dataspaces,
        lanes,
    );
    state.asset_instances.insert(treasury_asset_id);
    let mut recipes = BASE_RECIPES.to_vec();
    if nexus.is_some() {
        recipes.extend_from_slice(NEXUS_RECIPES);
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

const BASE_RECIPES: &[RecipeKind] = &[
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

const NEXUS_RECIPES: &[RecipeKind] = &[
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
    role_memberships: HashMap<RoleId, HashSet<AccountId>>,
    registered_triggers: Vec<TriggerId>,
    asset_definitions: HashSet<AssetDefinitionId>,
    asset_definitions_unclaimed: HashSet<AssetDefinitionId>,
    asset_instances: HashSet<AssetId>,
    nft_holdings: HashMap<NftId, AccountId>,
    domain_metadata: HashMap<DomainId, HashSet<Name>>,
    asset_definition_metadata: HashMap<AssetDefinitionId, HashSet<Name>>,
    asset_metadata: HashMap<AssetId, HashSet<Name>>,
    trigger_metadata: HashMap<TriggerId, HashSet<Name>>,
    trigger_repetitions: HashMap<TriggerId, u32>,
    space_directory_manifests: HashMap<UniversalAccountId, HashSet<DataSpaceId>>,
    public_lane_validators: HashMap<LaneId, HashSet<AccountId>>,
    pending_unbonds: Vec<PendingUnbond>,
    pending_replication_orders: Vec<ReplicationOrderId>,
    counters: ChaosCounters,
}

#[derive(Clone, Debug)]
struct PendingUnbond {
    lane: LaneId,
    validator: AccountId,
    staker: AccountId,
    request_id: Hash,
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

    fn new(
        base_domain: DomainId,
        treasury: AccountRecord,
        users: Vec<AccountRecord>,
        asset_numeric: AssetDefinitionId,
        asset_nft: AssetDefinitionId,
        dataspaces: Vec<DataSpaceId>,
        lanes: Vec<LaneId>,
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
            asset_definitions,
            asset_definitions_unclaimed: HashSet::new(),
            asset_instances: HashSet::new(),
            nft_holdings: HashMap::new(),
            domain_metadata: HashMap::new(),
            asset_definition_metadata: HashMap::new(),
            asset_metadata: HashMap::new(),
            trigger_metadata: HashMap::new(),
            trigger_repetitions: HashMap::new(),
            space_directory_manifests: HashMap::new(),
            public_lane_validators: HashMap::new(),
            pending_unbonds: Vec::new(),
            pending_replication_orders: Vec::new(),
            counters: ChaosCounters::default(),
        }
    }

    fn track_account(&mut self, record: AccountRecord) {
        if let Some(uaid) = record.uaid {
            self.uaid_accounts.insert(uaid, record.clone());
        }
        self.users.push(record);
    }

    fn allocate_uaid_record(&mut self) -> AccountRecord {
        let _ = self.bump_account();
        let key = KeyPair::random();
        let uaid = self.next_uaid();
        let account_id = AccountId::new(self.base_domain.clone(), key.public_key().clone());
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
            label: "duplicate_domain",
            instructions: vec![InstructionBox::from(Register::domain(Domain::new(target)))],
            signer: self.treasury.clone(),
            expect_success: false,
        }
    }

    fn plan_register_account(&mut self) -> TransactionPlan {
        let _suffix = self.bump_account();
        let key = KeyPair::random();
        let account_id = AccountId::new(self.base_domain.clone(), key.public_key().clone());
        let record = AccountRecord {
            id: account_id.clone(),
            key_pair: key,
            uaid: None,
        };
        self.track_account(record.clone());
        TransactionPlan {
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
        Ok(TransactionPlan {
            label: "set_account_kv",
            instructions: vec![InstructionBox::from(SetKeyValue::account(
                target.id.clone(),
                key,
                value,
            ))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_remove_key(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let target = self.random_user(rng)?.clone();
        let key: Name = "ephemeral"
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
            label: "remove_account_kv",
            instructions,
            signer: self.treasury.clone(),
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
            label: "register_asset_definition",
            instructions: vec![InstructionBox::from(Register::asset_definition(
                asset_definition,
            ))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_unregister_asset_definition(&mut self, rng: &mut StdRng) -> TransactionPlan {
        if let Some(candidate) = self.asset_definitions_unclaimed.iter().next().cloned() {
            self.asset_definitions_unclaimed.remove(&candidate);
            self.asset_definitions.remove(&candidate);
            self.asset_definition_metadata.remove(&candidate);
            return TransactionPlan {
                label: "unregister_asset_definition",
                instructions: vec![InstructionBox::from(Unregister::asset_definition(
                    candidate,
                ))],
                signer: self.treasury.clone(),
                expect_success: true,
            };
        }
        let fallback = self
            .asset_definitions
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .choose(rng)
            .cloned()
            .unwrap_or_else(|| self.asset_numeric.clone());
        TransactionPlan {
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
        if let Some(keys) = self.domain_metadata.get_mut(&domain)
            && let Some(key) = keys.iter().next().cloned()
        {
            keys.remove(&key);
            if keys.is_empty() {
                self.domain_metadata.remove(&domain);
            }
            return Ok(TransactionPlan {
                label: "remove_domain_kv",
                instructions: vec![InstructionBox::from(RemoveKeyValue::domain(domain, key))],
                signer: self.treasury.clone(),
                expect_success: true,
            });
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
        if let Some(keys) = self.asset_definition_metadata.get_mut(&definition)
            && let Some(key) = keys.iter().next().cloned()
        {
            keys.remove(&key);
            if keys.is_empty() {
                self.asset_definition_metadata.remove(&definition);
            }
            return Ok(TransactionPlan {
                label: "remove_asset_definition_kv",
                instructions: vec![InstructionBox::from(RemoveKeyValue::asset_definition(
                    definition, key,
                ))],
                signer: self.treasury.clone(),
                expect_success: true,
            });
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
            label: "remove_asset_definition_kv",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_set_asset_metadata(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let asset = self.random_asset_instance(rng)?;
        let key: Name = format!("asset_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse asset metadata key"))?;
        self.asset_metadata
            .entry(asset.clone())
            .or_default()
            .insert(key.clone());
        Ok(TransactionPlan {
            label: "set_asset_kv",
            instructions: vec![InstructionBox::from(SetAssetKeyValue::new(
                asset,
                key,
                json_pair("asset", "chaos"),
            ))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_remove_asset_metadata(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let asset = self.random_asset_instance(rng)?;
        if let Some(keys) = self.asset_metadata.get_mut(&asset)
            && let Some(key) = keys.iter().next().cloned()
        {
            keys.remove(&key);
            if keys.is_empty() {
                self.asset_metadata.remove(&asset);
            }
            return Ok(TransactionPlan {
                label: "remove_asset_kv",
                instructions: vec![InstructionBox::from(RemoveAssetKeyValue::new(asset, key))],
                signer: self.treasury.clone(),
                expect_success: true,
            });
        }
        let key: Name = format!("asset_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse fallback asset key"))?;
        let instructions = vec![
            InstructionBox::from(SetAssetKeyValue::new(
                asset.clone(),
                key.clone(),
                json_pair("ephemeral", true),
            )),
            InstructionBox::from(RemoveAssetKeyValue::new(asset, key)),
        ];
        Ok(TransactionPlan {
            label: "remove_asset_kv",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_register_nft(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let suffix = self.bump_nft();
        let domain_name = self.base_domain.name().to_string();
        let nft_id: NftId = format!("chaos_nft_{suffix}${domain_name}")
            .parse()
            .map_err(|_| eyre!("failed to parse nft id"))?;
        let owner = self.random_user(rng)?.clone();
        let nft = Nft::new(nft_id.clone(), Metadata::default());
        self.nft_holdings.insert(nft_id.clone(), owner.id.clone());
        Ok(TransactionPlan {
            label: "register_nft",
            instructions: vec![InstructionBox::from(Register::nft(nft))],
            signer: owner,
            expect_success: true,
        })
    }

    fn plan_transfer_nft(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        if self.nft_holdings.is_empty() {
            // Prime the state by registering an NFT owned by treasury and transferring it.
            let suffix = self.bump_nft();
            let domain_name = self.base_domain.name().to_string();
            let nft_id: NftId = format!("chaos_nft_{suffix}${domain_name}")
                .parse()
                .map_err(|_| eyre!("failed to parse fallback nft id"))?;
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
            return Ok(TransactionPlan {
                label: "transfer_nft",
                instructions,
                signer: self.treasury.clone(),
                expect_success: true,
            });
        }

        let (nft_id, owner_id) = self
            .nft_holdings
            .iter()
            .map(|(id, owner)| (id.clone(), owner.clone()))
            .collect::<Vec<_>>()
            .choose(rng)
            .cloned()
            .ok_or_else(|| eyre!("no nft holdings available"))?;
        let current_owner = self
            .account_by_id(&owner_id)
            .ok_or_else(|| eyre!("nft owner account missing"))?;
        let new_owner = self.random_user_except(rng, &owner_id)?;
        self.nft_holdings
            .insert(nft_id.clone(), new_owner.id.clone());
        Ok(TransactionPlan {
            label: "transfer_nft",
            instructions: vec![InstructionBox::from(Transfer::nft(
                current_owner.id.clone(),
                nft_id,
                new_owner.id.clone(),
            ))],
            signer: current_owner,
            expect_success: true,
        })
    }

    fn plan_set_trigger_key(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let mut instructions = Vec::new();
        let trigger_id = if let Some(existing) = self.random_registered_trigger(rng) {
            existing
        } else {
            let trigger_id: TriggerId = format!("metadata_trigger_{}", self.bump_trigger())
                .parse()
                .map_err(|_| eyre!("failed to parse metadata trigger id"))?;
            let log_instruction = Log::new(Level::INFO, "trigger metadata bootstrap".to_string());
            let action = Action::new(
                vec![InstructionBox::from(log_instruction)],
                Repeats::Exactly(1),
                self.treasury.id.clone(),
                EventFilterBox::Pipeline(PipelineEventFilterBox::Transaction(
                    TransactionEventFilter::new(),
                )),
            );
            instructions.push(InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id.clone(),
                action,
            ))));
            self.registered_triggers.push(trigger_id.clone());
            self.trigger_repetitions
                .entry(trigger_id.clone())
                .or_insert(0);
            trigger_id
        };

        let key: Name = format!("trigger_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse trigger metadata key"))?;
        self.trigger_metadata
            .entry(trigger_id.clone())
            .or_default()
            .insert(key.clone());
        instructions.push(InstructionBox::from(SetKeyValue::trigger(
            trigger_id.clone(),
            key,
            json_pair("trigger", "chaos"),
        )));
        Ok(TransactionPlan {
            label: "set_trigger_kv",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_remove_trigger_key(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let mut instructions = Vec::new();
        let trigger_id = if let Some(existing) = self.random_registered_trigger(rng) {
            existing
        } else {
            let trigger_id: TriggerId = format!("metadata_trigger_{}", self.bump_trigger())
                .parse()
                .map_err(|_| eyre!("failed to parse metadata trigger id"))?;
            let log_instruction = Log::new(Level::INFO, "trigger metadata bootstrap".to_string());
            let action = Action::new(
                vec![InstructionBox::from(log_instruction)],
                Repeats::Exactly(1),
                self.treasury.id.clone(),
                EventFilterBox::Pipeline(PipelineEventFilterBox::Transaction(
                    TransactionEventFilter::new(),
                )),
            );
            instructions.push(InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id.clone(),
                action,
            ))));
            self.registered_triggers.push(trigger_id.clone());
            self.trigger_repetitions
                .entry(trigger_id.clone())
                .or_insert(0);
            trigger_id
        };

        if let Some(keys) = self.trigger_metadata.get_mut(&trigger_id)
            && let Some(key) = keys.iter().next().cloned()
        {
            keys.remove(&key);
            if keys.is_empty() {
                self.trigger_metadata.remove(&trigger_id);
            }
            instructions.push(InstructionBox::from(RemoveKeyValue::trigger(
                trigger_id, key,
            )));
            return Ok(TransactionPlan {
                label: "remove_trigger_kv",
                instructions,
                signer: self.treasury.clone(),
                expect_success: true,
            });
        }

        let key: Name = format!("trigger_flag_{}", self.bump_metadata())
            .parse()
            .map_err(|_| eyre!("failed to parse fallback trigger key"))?;
        instructions.push(InstructionBox::from(SetKeyValue::trigger(
            trigger_id.clone(),
            key.clone(),
            json_pair("ephemeral", true),
        )));
        instructions.push(InstructionBox::from(RemoveKeyValue::trigger(
            trigger_id, key,
        )));
        Ok(TransactionPlan {
            label: "remove_trigger_kv",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_mint_trigger_repetitions(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let mut instructions = Vec::new();
        let trigger_id = if let Some(existing) = self.random_registered_trigger(rng) {
            existing
        } else {
            let trigger_id: TriggerId = format!("repeat_trigger_{}", self.bump_trigger())
                .parse()
                .map_err(|_| eyre!("failed to parse repeat trigger id"))?;
            let schedule = Schedule::starting_at(Duration::from_millis(250))
                .with_period(Duration::from_millis(1_000));
            let action = Action::new(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "repetition bootstrap".to_string(),
                ))],
                Repeats::Indefinitely,
                self.treasury.id.clone(),
                TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
            );
            instructions.push(InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id.clone(),
                action,
            ))));
            self.registered_triggers.push(trigger_id.clone());
            self.trigger_repetitions
                .entry(trigger_id.clone())
                .or_insert(0);
            trigger_id
        };
        let amount = rng.random_range(1_u32..=3_u32);
        *self
            .trigger_repetitions
            .entry(trigger_id.clone())
            .or_default() += amount;
        instructions.push(InstructionBox::from(Mint::trigger_repetitions(
            amount,
            trigger_id.clone(),
        )));
        Ok(TransactionPlan {
            label: "mint_trigger_repetitions",
            instructions,
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_burn_trigger_repetitions(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let mut instructions = Vec::new();
        let trigger_id = if let Some(existing) = self.random_registered_trigger(rng) {
            existing
        } else {
            return self.plan_mint_trigger_repetitions(rng);
        };
        let available = *self.trigger_repetitions.get(&trigger_id).unwrap_or(&0);
        let burn_amount = if available > 0 {
            rng.random_range(1..=available)
        } else {
            let amount = rng.random_range(1_u32..=2_u32);
            instructions.push(InstructionBox::from(Mint::trigger_repetitions(
                amount,
                trigger_id.clone(),
            )));
            *self
                .trigger_repetitions
                .entry(trigger_id.clone())
                .or_default() += amount;
            amount
        };
        instructions.push(InstructionBox::from(Burn::trigger_repetitions(
            burn_amount,
            trigger_id.clone(),
        )));
        let entry = self
            .trigger_repetitions
            .entry(trigger_id.clone())
            .or_default();
        *entry = entry.saturating_sub(burn_amount);
        Ok(TransactionPlan {
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
        self.role_memberships.entry(role_id.clone()).or_default();
        Ok(TransactionPlan {
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
        let assigned_accounts: HashSet<AccountId> = self
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
                label: "grant_role",
                instructions: vec![InstructionBox::from(Grant::account_role(role, account_id))],
                signer: self.treasury.clone(),
                expect_success: true,
            });
        }

        let assigned_vec: Vec<AccountId> = assigned_accounts.into_iter().collect();
        let fallback_account = if let Some(candidate) = assigned_vec.choose(rng) {
            candidate.clone()
        } else {
            self.random_user(rng)?.id.clone()
        };
        Ok(TransactionPlan {
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
        let existing_members: Vec<AccountId> = self
            .role_memberships
            .get(&role)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();

        if let Some(account_id) = existing_members.choose(rng).cloned() {
            let remove_role_entry = self.role_memberships.get_mut(&role).is_some_and(|members| {
                members.remove(&account_id);
                members.is_empty()
            });
            if remove_role_entry {
                self.role_memberships.remove(&role);
            }
            return Ok(TransactionPlan {
                label: "revoke_role",
                instructions: vec![InstructionBox::from(Revoke::account_role(role, account_id))],
                signer: self.treasury.clone(),
                expect_success: true,
            });
        }

        let account = self.random_user(rng)?.clone();
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
        self.registered_triggers.push(trigger_id.clone());
        self.trigger_repetitions
            .entry(trigger_id.clone())
            .or_insert(0);
        Ok(TransactionPlan {
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
        self.registered_triggers.push(trigger_id.clone());
        self.trigger_repetitions
            .entry(trigger_id.clone())
            .or_insert(0);
        Ok(TransactionPlan {
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
        self.registered_triggers.push(trigger_id.clone());
        self.trigger_repetitions
            .entry(trigger_id.clone())
            .or_insert(0);
        Ok(TransactionPlan {
            label: "register_pipeline_trigger",
            instructions: vec![InstructionBox::from(Register::trigger(Trigger::new(
                trigger_id, action,
            )))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_execute_trigger(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        if self.registered_triggers.is_empty() {
            return self.plan_time_trigger(rng);
        }
        let trigger = self
            .registered_triggers
            .choose(rng)
            .expect("non-empty trigger registry")
            .clone();
        Ok(TransactionPlan {
            label: "execute_trigger",
            instructions: vec![InstructionBox::from(ExecuteTrigger::new(trigger))],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_execute_missing_trigger(&mut self) -> Result<TransactionPlan> {
        let trigger_id: TriggerId = format!("ghost_trigger_{}", self.bump_invalid())
            .parse()
            .map_err(|_| eyre!("failed to parse ghost trigger id"))?;
        Ok(TransactionPlan {
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
        let bytecode = smart_contracts::ivm_artifact("artifact_v1_7_mode00_vlen0_cycles0_abi1")?;
        let action = Action::new(
            bytecode,
            Repeats::Exactly(1),
            self.treasury.id.clone(),
            TimeEventFilter::new(ExecutionTime::PreCommit),
        );
        self.registered_triggers.push(trigger_id.clone());
        self.trigger_repetitions
            .entry(trigger_id.clone())
            .or_insert(0);
        Ok(TransactionPlan {
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
        self.registered_triggers.push(trigger_id.clone());
        self.trigger_repetitions
            .entry(trigger_id.clone())
            .or_insert(0);
        Ok(TransactionPlan {
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
        let validator = self.random_user(rng)?.clone();
        let stake_account = self.random_user(rng)?.clone();
        let stake_amount: Numeric = rng.random_range(10_u32..=100_u32).into();
        let treasury_asset = AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone());
        let stake_asset = AssetId::new(self.asset_numeric.clone(), stake_account.id.clone());
        self.asset_instances.insert(treasury_asset.clone());
        self.asset_instances.insert(stake_asset);

        let mut instructions = vec![InstructionBox::from(Mint::asset_numeric(
            stake_amount.clone(),
            treasury_asset.clone(),
        ))];
        instructions.push(InstructionBox::from(Transfer::asset_numeric(
            treasury_asset,
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
        self.public_lane_validators
            .entry(lane)
            .or_default()
            .insert(validator.id.clone());

        Ok(TransactionPlan {
            label: "register_public_lane_validator",
            instructions,
            signer: validator,
            expect_success: true,
        })
    }

    fn plan_bond_public_stake(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let Some((lane, validator)) = self.pick_registered_validator(rng) else {
            let fallback_lane = self.random_lane(rng);
            let staker = self.random_user(rng)?.clone();
            let amount: Numeric = rng.random_range(1_u32..=5_u32).into();
            return Ok(TransactionPlan {
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
        let amount: Numeric = rng.random_range(5_u32..=40_u32).into();
        let treasury_asset = AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone());
        let staker_asset = AssetId::new(self.asset_numeric.clone(), staker.id.clone());
        self.asset_instances.insert(treasury_asset.clone());
        self.asset_instances.insert(staker_asset);
        let mut instructions = vec![InstructionBox::from(Mint::asset_numeric(
            amount.clone(),
            treasury_asset.clone(),
        ))];
        instructions.push(InstructionBox::from(Transfer::asset_numeric(
            treasury_asset,
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

        Ok(TransactionPlan {
            label: "bond_public_lane_stake",
            instructions,
            signer: staker,
            expect_success: true,
        })
    }

    fn plan_schedule_public_unbond(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let Some((lane, validator)) = self.pick_registered_validator(rng) else {
            let request_id = Hash::new(b"izanami-missing-unbond");
            let staker = self.random_user(rng)?.clone();
            return Ok(TransactionPlan {
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
        let staker = self.random_user(rng)?.clone();
        let request_id = Hash::new(format!("izanami-unbond-{}", self.bump_staking()).as_bytes());
        let amount: Numeric = rng.random_range(1_u32..=10_u32).into();
        let release_at = now_ms().saturating_add(5_000);
        self.pending_unbonds.push(PendingUnbond {
            lane,
            validator: validator.id.clone(),
            staker: staker.id.clone(),
            request_id,
        });

        Ok(TransactionPlan {
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
            expect_success: true,
        })
    }

    fn plan_finalize_public_unbond(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let pending = if let Some(entry) = self.pending_unbonds.choose(rng).cloned() {
            entry
        } else {
            return self.plan_schedule_public_unbond(rng);
        };
        self.pending_unbonds
            .retain(|entry| entry.request_id != pending.request_id);
        Ok(TransactionPlan {
            label: "finalize_public_lane_unbond",
            instructions: vec![InstructionBox::from(FinalizePublicLaneUnbond {
                lane_id: pending.lane,
                validator: pending.validator.clone(),
                staker: pending.staker.clone(),
                request_id: pending.request_id,
            })],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_slash_public_validator(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let Some((lane, validator)) = self.pick_registered_validator(rng) else {
            let slash_id = Hash::new(b"izanami-missing-slash");
            let staker = self.random_user(rng)?.clone();
            return Ok(TransactionPlan {
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
            expect_success: true,
        })
    }

    fn plan_record_public_rewards(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let Some((lane, validator)) = self.pick_registered_validator(rng) else {
            let epoch = self.bump_staking();
            let reward_asset = AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone());
            return Ok(TransactionPlan {
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
        let reward_asset = AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone());
        self.asset_instances.insert(reward_asset.clone());
        let reward: Numeric = rng.random_range(5_u32..=50_u32).into();
        let share = PublicLaneRewardShare {
            account: validator.id.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: reward.clone(),
        };
        let epoch = self.bump_staking();
        let mint = InstructionBox::from(Mint::asset_numeric(
            reward.clone(),
            AssetId::new(self.asset_numeric.clone(), self.treasury.id.clone()),
        ));
        Ok(TransactionPlan {
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
            expect_success: true,
        })
    }

    fn plan_dvp_settlement(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let seller = self.random_user(rng)?.clone();
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
            label: "dvp_settlement",
            instructions: vec![delivery_mint, payment_mint, InstructionBox::from(dvp)],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_issue_replication_order(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let mut order_id_bytes = [0u8; 32];
        rng.fill_bytes(&mut order_id_bytes);
        if order_id_bytes.iter().all(|byte| *byte == 0) {
            order_id_bytes[0] = 1;
        }
        let mut provider_bytes = [0u8; 32];
        rng.fill_bytes(&mut provider_bytes);
        if provider_bytes.iter().all(|byte| *byte == 0) {
            provider_bytes[0] = 2;
        }
        let mut policy_hash = [0u8; 32];
        rng.fill_bytes(&mut policy_hash);
        if policy_hash.iter().all(|byte| *byte == 0) {
            policy_hash[0] = 3;
        }
        let manifest_cid = format!("cid-{}", self.bump_replication()).into_bytes();
        let deadline = (now_ms() / 1_000).saturating_add(60);
        let order = ReplicationOrderV1 {
            order_id: order_id_bytes,
            manifest_cid,
            providers: vec![provider_bytes],
            redundancy: 1,
            deadline,
            policy_hash,
        };
        let payload = order.encode();
        let order_id = ReplicationOrderId::new(order_id_bytes);
        self.pending_replication_orders.push(order_id);
        Ok(TransactionPlan {
            label: "issue_replication_order",
            instructions: vec![InstructionBox::from(IssueReplicationOrder {
                order_id,
                order_payload: payload,
                issued_epoch: self.bump_replication(),
                deadline_epoch: deadline,
            })],
            signer: self.treasury.clone(),
            expect_success: true,
        })
    }

    fn plan_complete_replication_order(&mut self, rng: &mut StdRng) -> Result<TransactionPlan> {
        let Some(order_id) = self.pending_replication_orders.choose(rng).cloned() else {
            return self.plan_issue_replication_order(rng);
        };
        Ok(TransactionPlan {
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

    fn random_user_except(&self, rng: &mut StdRng, excluded: &AccountId) -> Result<AccountRecord> {
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

    fn account_by_id(&self, id: &AccountId) -> Option<AccountRecord> {
        if &self.treasury.id == id {
            Some(self.treasury.clone())
        } else {
            self.users.iter().find(|record| &record.id == id).cloned()
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

    fn random_registered_trigger(&self, rng: &mut StdRng) -> Option<TriggerId> {
        self.registered_triggers.choose(rng).cloned()
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
    use rand::SeedableRng;
    use tokio::runtime::Builder;

    use super::*;
    use crate::config::NexusProfile;

    #[test]
    fn json_pair_builds_object() {
        let value = json_pair("answer", 42u64);
        let mut expected = JsonMap::new();
        expected.insert("answer".to_string(), JsonValue::from(42u64));
        assert_eq!(value, JsonValue::Object(expected));
    }

    #[test]
    fn prepare_state_builds_genesis() {
        let prepared = prepare_state(4, None).expect("state prepared");
        assert!(!prepared.genesis.is_empty());
        assert!(!prepared.genesis[0].is_empty());
        assert!(prepared.state.users.len() >= 3);
        let expected: DomainId = "chaosnet".parse().unwrap();
        assert_eq!(prepared.state.base_domain(), &expected);
    }

    #[test]
    fn nexus_profile_injects_additional_recipes() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { recipes, .. } =
            prepare_state(3, Some(&profile)).expect("state prepared");
        assert!(
            recipes
                .iter()
                .any(|kind| matches!(kind, RecipeKind::RegisterPublicLaneValidator)),
            "nexus recipes should include staking paths"
        );
    }

    #[test]
    fn produce_plan_for_all_recipes() {
        let PreparedChaos {
            mut state, recipes, ..
        } = prepare_state(3, None).expect("state prepared");
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
        let PreparedChaos { state, recipes, .. } = prepare_state(4, None).expect("state prepared");
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
            guard.users.len() + 1
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
            prepare_state(2, Some(&profile)).expect("state prepared");
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
            prepare_state(3, Some(&profile)).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(31);
        let plan = state
            .plan_register_public_validator(&mut rng)
            .expect("validator plan builds");
        assert_eq!(plan.label, "register_public_lane_validator");
        assert!(
            state
                .public_lane_validators
                .values()
                .any(|validators| !validators.is_empty()),
            "validator registry should record new entries"
        );
    }

    #[test]
    fn replication_orders_are_tracked() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(2, Some(&profile)).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(45);
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
    fn dvp_settlement_plan_builds() {
        let PreparedChaos { mut state, .. } = prepare_state(3, None).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(77);
        let plan = state
            .plan_dvp_settlement(&mut rng)
            .expect("dvp plan builds");
        assert_eq!(plan.label, "dvp_settlement");
        assert!(plan.expect_success);
    }

    #[test]
    fn public_unbond_tracks_pending_requests() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(3, Some(&profile)).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(51);
        let _ = state
            .plan_register_public_validator(&mut rng)
            .expect("validator plan");
        let plan = state
            .plan_schedule_public_unbond(&mut rng)
            .expect("unbond plan");
        assert_eq!(plan.label, "schedule_public_lane_unbond");
        assert!(
            !state.pending_unbonds.is_empty(),
            "pending unbonds should be recorded"
        );
    }

    #[test]
    fn public_rewards_follow_validator_registry() {
        let profile = NexusProfile::sora_defaults().expect("profile");
        let PreparedChaos { mut state, .. } =
            prepare_state(3, Some(&profile)).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(61);
        let _ = state
            .plan_register_public_validator(&mut rng)
            .expect("validator plan");
        let plan = state
            .plan_record_public_rewards(&mut rng)
            .expect("reward plan");
        assert_eq!(plan.label, "record_public_lane_rewards");
        assert!(plan.expect_success);
    }

    #[test]
    fn asset_definition_register_and_unregister_moves_tracking() {
        let PreparedChaos { mut state, .. } = prepare_state(3, None).expect("state prepared");
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
    fn nft_registration_and_transfer_updates_owner() {
        let PreparedChaos { mut state, .. } = prepare_state(3, None).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(29);
        let register_plan = state.plan_register_nft(&mut rng).expect("register nft");
        assert_eq!(register_plan.label, "register_nft");
        assert_eq!(state.nft_holdings.len(), 1, "nft holding should be tracked");
        let (nft_id, initial_owner) = state
            .nft_holdings
            .iter()
            .next()
            .map(|(id, owner)| (id.clone(), owner.clone()))
            .expect("nft recorded");
        let transfer_plan = state.plan_transfer_nft(&mut rng).expect("transfer nft");
        assert_eq!(transfer_plan.label, "transfer_nft");
        let updated_owner = state
            .nft_holdings
            .get(&nft_id)
            .expect("owner should remain tracked");
        assert_ne!(
            updated_owner, &initial_owner,
            "transfer should update nft owner tracking"
        );
    }

    #[test]
    fn trigger_repetition_mint_and_burn_balance() {
        let PreparedChaos { mut state, .. } = prepare_state(3, None).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(93);
        let mint_plan = state
            .plan_mint_trigger_repetitions(&mut rng)
            .expect("mint repetitions");
        assert_eq!(mint_plan.label, "mint_trigger_repetitions");
        let (trigger_id, minted) = state
            .trigger_repetitions
            .iter()
            .next()
            .map(|(id, amount)| (id.clone(), *amount))
            .expect("repetition tracked");
        assert!(minted > 0, "mint should increase repetition counter");
        let burn_plan = state
            .plan_burn_trigger_repetitions(&mut rng)
            .expect("burn repetitions");
        assert_eq!(burn_plan.label, "burn_trigger_repetitions");
        let remaining = state
            .trigger_repetitions
            .get(&trigger_id)
            .copied()
            .unwrap_or_default();
        assert!(remaining <= minted, "burn must not increase repetitions");
    }

    #[test]
    fn asset_metadata_set_and_remove_trackers() {
        let PreparedChaos { mut state, .. } = prepare_state(3, None).expect("state prepared");
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
    fn register_uaid_account_tracks_mapping() {
        let PreparedChaos { mut state, .. } = prepare_state(3, None).expect("state prepared");
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
        let PreparedChaos { mut state, .. } = prepare_state(2, None).expect("state prepared");
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
        let PreparedChaos { mut state, .. } = prepare_state(2, None).expect("state prepared");
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
        let PreparedChaos { mut state, .. } = prepare_state(3, None).expect("state prepared");
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
