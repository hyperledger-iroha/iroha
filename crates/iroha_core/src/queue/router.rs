//! Lane and dataspace routing utilities for the transaction queue.
//!
//! These helpers translate pending transactions into the lane/dataspace
//! identifiers that the Nexus scheduler expects, based on the runtime
//! configuration. The router abstraction keeps the queue decoupled from the
//! exact routing policy while allowing metrics to reflect the real
//! assignments instead of single-lane placeholders.

use std::{collections::BTreeSet, sync::Arc};

use iroha_config::parameters::actual::{LaneRoutingMatcher, LaneRoutingPolicy, LaneRoutingRule};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::{AccountAlias, AccountId},
    asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionAlias, AssetDefinitionId},
    domain::DomainId,
    isi::{
        BurnBox, GrantBox, Instruction, MintBox, RegisterBox, RemoveKeyValueBox, RevokeBox,
        SetKeyValueBox, TransferBox, UnregisterBox,
        asset_alias::SetAssetDefinitionBalancePolicy,
        contract_alias::SetContractAlias,
        musubi::{
            AssertMusubiReleaseExists, PublishMusubiRelease, SetMusubiShortAlias, YankMusubiRelease,
        },
        offline::{AuditOfflineNote, IssueOfflineNote, KagemushaTransfer, RedeemOfflineNote},
        settlement::{DvpIsi, PvpIsi, SettlementInstructionBox},
        smart_contract_code::{
            ActivateContractInstance, DeactivateContractInstance, RegisterSmartContractBytes,
            RegisterSmartContractCode,
        },
    },
    musubi::{MusubiNamespace, MusubiPackageId},
    nexus::{DataSpaceCatalog, DataSpaceId, LaneCatalog, LaneId},
    permission::Permission,
    smart_contract::ContractAddress,
    transaction::Executable,
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias},
    asset::{
        CanBurnAssetWithDefinition, CanMintAssetWithDefinition,
        CanModifyAssetMetadataWithDefinition, CanTransferAssetWithDefinition,
    },
    asset_definition::{CanModifyAssetDefinitionMetadata, CanUnregisterAssetDefinition},
    nexus::CanPublishSpaceDirectoryManifest,
};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};

use crate::{
    state::{State, StateReadOnly, StateView, WorldReadOnly},
    tx::AcceptedTransaction,
};
use thiserror::Error;

const AMX_POLICY_METADATA_KEY: &str = "amx_policy";
const AMX_POLICY_REJECT_CROSS_DATASPACE: &str = "reject_cross_dataspace";

/// Routing decision returned by a [`LaneRouter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct RoutingDecision {
    /// Lane assigned to the transaction.
    pub lane_id: LaneId,
    /// Dataspace assigned to the transaction.
    pub dataspace_id: DataSpaceId,
}

impl RoutingDecision {
    /// Create a new routing decision.
    #[must_use]
    pub const fn new(lane_id: LaneId, dataspace_id: DataSpaceId) -> Self {
        Self {
            lane_id,
            dataspace_id,
        }
    }
}

impl Default for RoutingDecision {
    fn default() -> Self {
        Self::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
    }
}

/// Role of one route in a transaction routing plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RouteLegRole {
    /// The route coordinates final admission and commit ordering for the plan.
    Coordinator,
    /// The route prepares or commits one dataspace-local leg of the plan.
    Participant,
}

/// One lane/dataspace leg in a transaction routing plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct RouteLeg {
    /// Lane and dataspace selected for this leg.
    pub route: RoutingDecision,
    /// Plan role assigned to the leg.
    pub role: RouteLegRole,
}

impl RouteLeg {
    /// Construct a new route leg.
    #[must_use]
    pub const fn new(route: RoutingDecision, role: RouteLegRole) -> Self {
        Self { route, role }
    }
}

/// Native AMX routing plan for a transaction that touches multiple dataspaces.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NativeAmxRoutingPlan {
    /// Stable digest of the coordinator and participant route set.
    pub plan_digest: Hash,
    /// Coordinator route for the native AMX plan.
    pub coordinator: RouteLeg,
    /// Dataspace-local participant routes sorted by dataspace and lane id.
    pub participants: Vec<RouteLeg>,
}

/// Complete routing plan for a transaction.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RoutingPlan {
    /// The transaction executes on one lane/dataspace route.
    Single(RouteLeg),
    /// The transaction requires native AMX coordination across dataspaces.
    NativeAmx(NativeAmxRoutingPlan),
}

impl RoutingPlan {
    /// Construct a single-route coordinator plan.
    #[must_use]
    pub const fn single(route: RoutingDecision) -> Self {
        Self::Single(RouteLeg::new(route, RouteLegRole::Coordinator))
    }

    /// Construct a canonical native AMX plan.
    #[must_use]
    pub fn native_amx(coordinator: RoutingDecision, mut participants: Vec<RouteLeg>) -> Self {
        participants.sort_by_key(|leg| (leg.route.dataspace_id, leg.route.lane_id));
        participants.dedup_by_key(|leg| (leg.route.dataspace_id, leg.route.lane_id));
        for leg in &mut participants {
            leg.role = RouteLegRole::Participant;
        }
        let plan_digest = native_amx_plan_digest(coordinator, &participants);
        Self::NativeAmx(NativeAmxRoutingPlan {
            plan_digest,
            coordinator: RouteLeg::new(coordinator, RouteLegRole::Coordinator),
            participants,
        })
    }

    /// Return the route that existing single-route queue machinery should use as coordinator.
    #[must_use]
    pub const fn coordinator_route(&self) -> RoutingDecision {
        match self {
            Self::Single(leg) => leg.route,
            Self::NativeAmx(plan) => plan.coordinator.route,
        }
    }

    /// Return the coordinator leg.
    #[must_use]
    pub const fn coordinator_leg(&self) -> RouteLeg {
        match self {
            Self::Single(leg) => *leg,
            Self::NativeAmx(plan) => plan.coordinator,
        }
    }

    /// Return all plan legs in deterministic coordinator-first order.
    #[must_use]
    pub fn legs(&self) -> Vec<RouteLeg> {
        match self {
            Self::Single(leg) => vec![*leg],
            Self::NativeAmx(plan) => {
                let mut legs = Vec::with_capacity(plan.participants.len().saturating_add(1));
                legs.push(plan.coordinator);
                legs.extend(plan.participants.iter().copied());
                legs
            }
        }
    }

    /// Return the deterministic digest for the plan.
    #[must_use]
    pub fn digest(&self) -> Hash {
        match self {
            Self::Single(leg) => routing_plan_digest(&[leg.route]),
            Self::NativeAmx(plan) => plan.plan_digest,
        }
    }
}

/// Deterministic routing resolution failure against configured Nexus catalogs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RoutingResolveError {
    /// lane {lane_id} is not present in the lane catalog
    #[error("lane {lane_id} is not present in the lane catalog")]
    UnknownLane {
        /// Lane selected by the routing policy.
        lane_id: LaneId,
    },
    /// dataspace {dataspace_id} is not present in the dataspace catalog
    #[error("dataspace {dataspace_id} is not present in the dataspace catalog")]
    UnknownDataspace {
        /// Dataspace selected by the routing policy.
        dataspace_id: DataSpaceId,
    },
    /// lane {lane_id} is bound to dataspace {lane_dataspace_id}, but resolved dataspace is {dataspace_id}
    #[error(
        "lane {lane_id} is bound to dataspace {lane_dataspace_id}, but resolved dataspace is {dataspace_id}"
    )]
    LaneDataspaceMismatch {
        /// Lane selected by the routing policy.
        lane_id: LaneId,
        /// Dataspace configured for the resolved lane.
        lane_dataspace_id: DataSpaceId,
        /// Dataspace selected by the routing policy.
        dataspace_id: DataSpaceId,
    },
    /// no lane is bound to dataspace {dataspace_id}
    #[error("no lane is bound to dataspace {dataspace_id}")]
    NoLaneForDataspace {
        /// Dataspace selected by the routing policy.
        dataspace_id: DataSpaceId,
    },
    /// transaction mixes dataspace-scoped permission targets {first_dataspace_id} and {second_dataspace_id}
    #[error(
        "transaction mixes dataspace-scoped permission targets {first_dataspace_id} and {second_dataspace_id}"
    )]
    ConflictingDataspaceScopedPermissions {
        /// First dataspace target found in the transaction.
        first_dataspace_id: DataSpaceId,
        /// Conflicting dataspace target found in the transaction.
        second_dataspace_id: DataSpaceId,
    },
    /// transaction mixes dataspace-routed write targets {first_dataspace_id} and {second_dataspace_id}
    #[error(
        "transaction mixes dataspace-routed write targets {first_dataspace_id} and {second_dataspace_id}"
    )]
    ConflictingTransactionDataspaceTargets {
        /// First dataspace-routed write target found in the transaction.
        first_dataspace_id: DataSpaceId,
        /// Conflicting dataspace-routed write target found in the transaction.
        second_dataspace_id: DataSpaceId,
    },
}

impl RoutingResolveError {
    /// Stable telemetry label for deterministic routing failures.
    #[must_use]
    pub const fn as_label(&self) -> &'static str {
        match self {
            Self::UnknownLane { .. } => "unknown_lane",
            Self::UnknownDataspace { .. } => "unknown_dataspace",
            Self::LaneDataspaceMismatch { .. } => "lane_dataspace_mismatch",
            Self::NoLaneForDataspace { .. } => "no_lane_for_dataspace",
            Self::ConflictingDataspaceScopedPermissions { .. } => {
                "conflicting_dataspace_scoped_permissions"
            }
            Self::ConflictingTransactionDataspaceTargets { .. } => {
                "conflicting_transaction_dataspace_targets"
            }
        }
    }
}

/// Evaluate the configured routing policy for a transaction, returning the lane and dataspace.
///
/// This does not validate the decision against the lane or dataspace catalogs. Use
/// [`evaluate_policy_with_catalog`] when catalog alignment is required.
pub fn evaluate_policy(
    policy: &LaneRoutingPolicy,
    tx: &AcceptedTransaction<'_>,
) -> RoutingDecision {
    if let Some(decision) =
        dataspace_scoped_permission_routing_decision(tx, None, None, None).unwrap_or(None)
    {
        return decision;
    }
    if let Some(decision) = settlement_routing_decision_without_catalog(tx) {
        return decision;
    }
    if let Some(account_id) = account_permission_holder_routing_target(tx) {
        return evaluate_query_policy_with_view(policy, account_id, None);
    }
    let target_dataspace = transaction_dataspace_routing_target(tx, None, None).unwrap_or(None);
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches(rule, tx, None));
    let lane_id = matched_rule.map_or(policy.default_lane, |rule| rule.lane);
    let dataspace_id = matched_rule
        .and_then(|rule| rule.dataspace)
        .or(target_dataspace)
        .unwrap_or(policy.default_dataspace);
    RoutingDecision::new(lane_id, dataspace_id)
}

fn evaluate_policy_with_view(
    policy: &LaneRoutingPolicy,
    tx: &AcceptedTransaction<'_>,
    state_view: &StateView<'_>,
) -> RoutingDecision {
    if let Some(decision) = dataspace_scoped_permission_routing_decision(
        tx,
        Some(&state_view.nexus().lane_catalog),
        Some(&state_view.nexus().dataspace_catalog),
        Some(state_view),
    )
    .unwrap_or(None)
    {
        return decision;
    }
    if let Some(decision) = settlement_routing_decision(
        tx,
        &state_view.nexus().lane_catalog,
        &state_view.nexus().dataspace_catalog,
        Some(state_view),
    )
    .unwrap_or(None)
    {
        return decision;
    }
    if let Some(account_id) = account_permission_holder_routing_target(tx) {
        return evaluate_query_policy_with_view(policy, account_id, Some(state_view));
    }
    let mut target = transaction_dataspace_routing_target_info(
        tx,
        Some(&state_view.nexus().dataspace_catalog),
        Some(state_view),
    )
    .unwrap_or_default();
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches(rule, tx, Some(state_view)));
    apply_authority_dataspace_target(
        &mut target,
        authority_dataspace_target(Some(state_view), tx),
        matched_rule.is_some_and(|rule| rule.matcher.account.is_some()),
    );
    let target_dataspace = target.dataspace_id;
    if matched_rule.is_none()
        && let Some(dataspace_id) = target_dataspace
    {
        return canonical_dataspace_route(
            dataspace_id,
            &state_view.nexus().lane_catalog,
            &state_view.nexus().dataspace_catalog,
        )
        .unwrap_or_else(|_| RoutingDecision::new(policy.default_lane, dataspace_id));
    }
    let lane_id = matched_rule.map_or(policy.default_lane, |rule| rule.lane);
    let dataspace_id = matched_rule
        .and_then(|rule| rule.dataspace)
        .or(target_dataspace)
        .unwrap_or(policy.default_dataspace);
    RoutingDecision::new(lane_id, dataspace_id)
}

fn evaluate_policy_with_catalog_hint(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
) -> RoutingDecision {
    if let Some(decision) = dataspace_scoped_permission_routing_decision(
        tx,
        Some(lane_catalog),
        Some(dataspace_catalog),
        None,
    )
    .unwrap_or(None)
    {
        return decision;
    }
    if let Some(decision) =
        settlement_routing_decision(tx, lane_catalog, dataspace_catalog, None).unwrap_or(None)
    {
        return decision;
    }
    if let Some(account_id) = account_permission_holder_routing_target(tx) {
        return evaluate_query_policy_with_view(policy, account_id, None);
    }
    let target_dataspace =
        transaction_dataspace_routing_target(tx, Some(dataspace_catalog), None).unwrap_or(None);
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches(rule, tx, None));
    if matched_rule.is_none()
        && let Some(dataspace_id) = target_dataspace
    {
        return canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog)
            .unwrap_or_else(|_| RoutingDecision::new(policy.default_lane, dataspace_id));
    }
    let lane_id = matched_rule.map_or(policy.default_lane, |rule| rule.lane);
    let dataspace_id = matched_rule
        .and_then(|rule| rule.dataspace)
        .or(target_dataspace)
        .unwrap_or(policy.default_dataspace);
    RoutingDecision::new(lane_id, dataspace_id)
}

/// Evaluate the routing policy and resolve it against the configured catalogs.
pub fn evaluate_policy_with_catalog(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
) -> Result<RoutingDecision, RoutingResolveError> {
    if let Some(decision) = dataspace_scoped_permission_routing_decision(
        tx,
        Some(lane_catalog),
        Some(dataspace_catalog),
        None,
    )? {
        return Ok(decision);
    }
    if let Some(decision) = settlement_routing_decision(tx, lane_catalog, dataspace_catalog, None)?
    {
        return Ok(decision);
    }
    if let Some(account_id) = account_permission_holder_routing_target(tx) {
        return resolve_query_routing_decision(
            policy,
            lane_catalog,
            dataspace_catalog,
            account_id,
            None,
        );
    }
    let target = transaction_dataspace_routing_target_info(tx, Some(dataspace_catalog), None)?;
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches(rule, tx, None));
    resolve_policy_routing_decision(
        policy,
        matched_rule,
        target.dataspace_id,
        target.coordinator_route,
        lane_catalog,
        dataspace_catalog,
    )
}

/// Evaluate the routing policy and resolve the full routing plan against the configured catalogs.
pub fn evaluate_policy_plan_with_catalog(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
) -> Result<RoutingPlan, RoutingResolveError> {
    if let Some(decision) = dataspace_scoped_permission_routing_decision(
        tx,
        Some(lane_catalog),
        Some(dataspace_catalog),
        None,
    )? {
        return Ok(RoutingPlan::single(decision));
    }
    if let Some(decision) = settlement_routing_decision(tx, lane_catalog, dataspace_catalog, None)?
    {
        return Ok(RoutingPlan::single(decision));
    }
    if let Some(account_id) = account_permission_holder_routing_target(tx) {
        return resolve_query_routing_decision(
            policy,
            lane_catalog,
            dataspace_catalog,
            account_id,
            None,
        )
        .map(RoutingPlan::single);
    }
    let target = transaction_dataspace_routing_target_info(tx, Some(dataspace_catalog), None)?;
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches(rule, tx, None));
    resolve_policy_routing_plan(
        policy,
        matched_rule,
        target,
        lane_catalog,
        dataspace_catalog,
    )
}

/// Evaluate the routing policy against catalogs, resolving opaque dataspace-scoped
/// permissions from the current world snapshot when possible.
pub fn evaluate_policy_with_catalog_and_world<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
    world: &W,
) -> Result<RoutingDecision, RoutingResolveError> {
    evaluate_policy_with_catalog_and_world_at_opt(
        policy,
        lane_catalog,
        dataspace_catalog,
        tx,
        world,
        None,
    )
}

/// Evaluate the routing policy against catalogs and the current world at a deterministic ledger time.
pub fn evaluate_policy_with_catalog_and_world_at<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
    world: &W,
    ledger_time_ms: u64,
) -> Result<RoutingDecision, RoutingResolveError> {
    evaluate_policy_with_catalog_and_world_at_opt(
        policy,
        lane_catalog,
        dataspace_catalog,
        tx,
        world,
        Some(ledger_time_ms),
    )
}

fn evaluate_policy_with_catalog_and_world_at_opt<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<RoutingDecision, RoutingResolveError> {
    if let Some(decision) = dataspace_scoped_permission_routing_decision_with_world(
        tx,
        Some(lane_catalog),
        Some(dataspace_catalog),
        world,
        ledger_time_ms,
    )? {
        return Ok(decision);
    }
    if let Some(decision) = settlement_routing_decision_with_world(
        tx,
        lane_catalog,
        dataspace_catalog,
        world,
        ledger_time_ms,
    )? {
        return Ok(decision);
    }
    if let Some(account_id) = account_permission_holder_routing_target(tx) {
        return resolve_query_routing_decision_with_world(
            policy,
            lane_catalog,
            dataspace_catalog,
            account_id,
            world,
            ledger_time_ms,
        );
    }
    let mut target = transaction_dataspace_routing_target_info_with_world(
        tx,
        Some(dataspace_catalog),
        world,
        ledger_time_ms,
    )?;
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches_with_world(rule, tx, dataspace_catalog, world, ledger_time_ms));
    apply_authority_dataspace_target(
        &mut target,
        authority_dataspace_target_with_world(Some(world), tx),
        matched_rule.is_some_and(|rule| rule.matcher.account.is_some()),
    );
    resolve_policy_routing_decision(
        policy,
        matched_rule,
        target.dataspace_id,
        target.coordinator_route,
        lane_catalog,
        dataspace_catalog,
    )
}

/// Evaluate the routing policy and resolve the full routing plan against catalogs/world state.
pub fn evaluate_policy_plan_with_catalog_and_world<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
    world: &W,
) -> Result<RoutingPlan, RoutingResolveError> {
    evaluate_policy_plan_with_catalog_and_world_at_opt(
        policy,
        lane_catalog,
        dataspace_catalog,
        tx,
        world,
        None,
    )
}

/// Evaluate the routing policy and resolve the full plan at a deterministic ledger time.
pub fn evaluate_policy_plan_with_catalog_and_world_at<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
    world: &W,
    ledger_time_ms: u64,
) -> Result<RoutingPlan, RoutingResolveError> {
    evaluate_policy_plan_with_catalog_and_world_at_opt(
        policy,
        lane_catalog,
        dataspace_catalog,
        tx,
        world,
        Some(ledger_time_ms),
    )
}

fn evaluate_policy_plan_with_catalog_and_world_at_opt<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<RoutingPlan, RoutingResolveError> {
    if let Some(decision) = dataspace_scoped_permission_routing_decision_with_world(
        tx,
        Some(lane_catalog),
        Some(dataspace_catalog),
        world,
        ledger_time_ms,
    )? {
        return Ok(RoutingPlan::single(decision));
    }
    if let Some(decision) = settlement_routing_decision_with_world(
        tx,
        lane_catalog,
        dataspace_catalog,
        world,
        ledger_time_ms,
    )? {
        return Ok(RoutingPlan::single(decision));
    }
    if let Some(account_id) = account_permission_holder_routing_target(tx) {
        return resolve_query_routing_decision_with_world(
            policy,
            lane_catalog,
            dataspace_catalog,
            account_id,
            world,
            ledger_time_ms,
        )
        .map(RoutingPlan::single);
    }
    let mut target = transaction_dataspace_routing_target_info_with_world(
        tx,
        Some(dataspace_catalog),
        world,
        ledger_time_ms,
    )?;
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches_with_world(rule, tx, dataspace_catalog, world, ledger_time_ms));
    apply_authority_dataspace_target(
        &mut target,
        authority_dataspace_target_with_world(Some(world), tx),
        matched_rule.is_some_and(|rule| rule.matcher.account.is_some()),
    );
    resolve_policy_routing_plan(
        policy,
        matched_rule,
        target,
        lane_catalog,
        dataspace_catalog,
    )
}

fn dataspace_scoped_permission_routing_decision(
    tx: &AcceptedTransaction<'_>,
    lane_catalog: Option<&LaneCatalog>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Result<Option<RoutingDecision>, RoutingResolveError> {
    let mut target_dataspace: Option<DataSpaceId> = None;
    let reject_cross_dataspace = amx_policy_rejects_cross_dataspace(tx);
    let Some(executable) = transaction_executable(tx) else {
        return Ok(None);
    };

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                merge_native_target_dataspace(
                    &mut target_dataspace,
                    instruction_dataspace_scoped_permission_target(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    ),
                    reject_cross_dataspace,
                    NativeDataspaceConflict::Permission,
                )?;
            }
        }
        Executable::ContractCall(call) => {
            merge_native_target_dataspace(
                &mut target_dataspace,
                contract_address_dataspace_target(&call.contract_address),
                reject_cross_dataspace,
                NativeDataspaceConflict::Transaction,
            )?;
        }
        Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                merge_native_target_dataspace(
                    &mut target_dataspace,
                    instruction_dataspace_scoped_permission_target(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    ),
                    reject_cross_dataspace,
                    NativeDataspaceConflict::Permission,
                )?;
            }
        }
    }

    let Some(dataspace_id) = target_dataspace else {
        return Ok(None);
    };

    match (lane_catalog, dataspace_catalog) {
        (Some(lane_catalog), Some(dataspace_catalog)) => {
            canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog).map(Some)
        }
        _ => Ok(None),
    }
}

fn dataspace_scoped_permission_routing_decision_with_world<W: WorldReadOnly>(
    tx: &AcceptedTransaction<'_>,
    lane_catalog: Option<&LaneCatalog>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<Option<RoutingDecision>, RoutingResolveError> {
    let mut target_dataspace: Option<DataSpaceId> = None;
    let reject_cross_dataspace = amx_policy_rejects_cross_dataspace(tx);
    let Some(executable) = transaction_executable(tx) else {
        return Ok(None);
    };

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                merge_native_target_dataspace(
                    &mut target_dataspace,
                    instruction_dataspace_scoped_permission_target_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    ),
                    reject_cross_dataspace,
                    NativeDataspaceConflict::Permission,
                )?;
            }
        }
        Executable::ContractCall(call) => {
            merge_native_target_dataspace(
                &mut target_dataspace,
                contract_address_dataspace_target(&call.contract_address),
                reject_cross_dataspace,
                NativeDataspaceConflict::Transaction,
            )?;
        }
        Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                merge_native_target_dataspace(
                    &mut target_dataspace,
                    instruction_dataspace_scoped_permission_target_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    ),
                    reject_cross_dataspace,
                    NativeDataspaceConflict::Permission,
                )?;
            }
        }
    }

    let Some(dataspace_id) = target_dataspace else {
        return Ok(None);
    };

    match (lane_catalog, dataspace_catalog) {
        (Some(lane_catalog), Some(dataspace_catalog)) => {
            canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog).map(Some)
        }
        _ => Ok(None),
    }
}

fn settlement_routing_decision_without_catalog(
    tx: &AcceptedTransaction<'_>,
) -> Option<RoutingDecision> {
    let dataspace_id = settlement_transaction_dataspace_target(tx, None, None)?;
    (dataspace_id == DataSpaceId::UNIVERSAL)
        .then(|| RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
}

fn settlement_routing_decision(
    tx: &AcceptedTransaction<'_>,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    state_view: Option<&StateView<'_>>,
) -> Result<Option<RoutingDecision>, RoutingResolveError> {
    let Some(dataspace_id) =
        settlement_transaction_dataspace_target(tx, Some(dataspace_catalog), state_view)
    else {
        return Ok(None);
    };
    canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog).map(Some)
}

fn settlement_routing_decision_with_world<W: WorldReadOnly>(
    tx: &AcceptedTransaction<'_>,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<Option<RoutingDecision>, RoutingResolveError> {
    let Some(dataspace_id) = settlement_transaction_dataspace_target_with_world(
        tx,
        Some(dataspace_catalog),
        world,
        ledger_time_ms,
    ) else {
        return Ok(None);
    };
    canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog).map(Some)
}

fn merge_settlement_target_dataspace(
    target_dataspace: &mut Option<DataSpaceId>,
    candidate: Option<DataSpaceId>,
) {
    let Some(candidate) = candidate else {
        return;
    };

    match *target_dataspace {
        Some(existing) if existing != candidate => {
            // Mixed settlement legs require a deterministic coordinator route instead of being
            // captured by the transaction authority's ordinary account rule.
            *target_dataspace = Some(DataSpaceId::UNIVERSAL);
        }
        Some(_) => {}
        None => *target_dataspace = Some(candidate),
    }
}

fn settlement_pair_dataspace_target(
    first: Option<DataSpaceId>,
    second: Option<DataSpaceId>,
) -> Option<DataSpaceId> {
    match (first, second) {
        (Some(first), Some(second)) if first == second => Some(first),
        (Some(_), Some(_)) => Some(DataSpaceId::UNIVERSAL),
        (Some(dataspace), None) | (None, Some(dataspace)) => Some(dataspace),
        (None, None) => None,
    }
}

fn asset_id_explicit_dataspace_target(
    asset_id: &iroha_data_model::asset::AssetId,
) -> Option<DataSpaceId> {
    match asset_id.scope() {
        iroha_data_model::asset::AssetBalanceScope::Dataspace(dataspace) => Some(*dataspace),
        iroha_data_model::asset::AssetBalanceScope::Global => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AssetBalanceDefinitionRouteTarget {
    dataspace_id: Option<DataSpaceId>,
    balance_scope_policy: Option<AssetBalancePolicy>,
}

fn merge_instruction_dataspace_targets<I>(targets: I) -> Option<DataSpaceId>
where
    I: IntoIterator<Item = Option<DataSpaceId>>,
{
    targets.into_iter().flatten().fold(None, |acc, target| {
        settlement_pair_dataspace_target(acc, Some(target))
    })
}

fn asset_balance_operation_dataspace_target(
    asset_definition_target: AssetBalanceDefinitionRouteTarget,
    explicit_asset_target: Option<DataSpaceId>,
    account_targets: impl IntoIterator<Item = Option<DataSpaceId>>,
) -> Option<DataSpaceId> {
    let effective_definition_target = if explicit_asset_target.is_some()
        && asset_definition_target.dataspace_id == Some(DataSpaceId::UNIVERSAL)
        && asset_definition_target.balance_scope_policy != Some(AssetBalancePolicy::Global)
    {
        None
    } else {
        asset_definition_target.dataspace_id
    };

    let authoritative_asset_target =
        settlement_pair_dataspace_target(effective_definition_target, explicit_asset_target);
    let ignore_universal_account_fallbacks = asset_definition_target.balance_scope_policy
        == Some(AssetBalancePolicy::DataspaceRestricted)
        && authoritative_asset_target.is_some_and(|target| target != DataSpaceId::UNIVERSAL);
    let account_targets = account_targets.into_iter().map(|target| {
        if ignore_universal_account_fallbacks && target == Some(DataSpaceId::UNIVERSAL) {
            None
        } else {
            target
        }
    });

    merge_instruction_dataspace_targets(
        core::iter::once(effective_definition_target)
            .chain(core::iter::once(explicit_asset_target))
            .chain(account_targets),
    )
}

fn settlement_transaction_dataspace_target(
    tx: &AcceptedTransaction<'_>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let Some(executable) = transaction_executable(tx) else {
        return None;
    };
    let mut target_dataspace = None;

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                merge_settlement_target_dataspace(
                    &mut target_dataspace,
                    instruction_settlement_dataspace_target(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    ),
                );
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                merge_settlement_target_dataspace(
                    &mut target_dataspace,
                    instruction_settlement_dataspace_target(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    ),
                );
            }
        }
    }

    target_dataspace
}

fn settlement_transaction_dataspace_target_with_world<W: WorldReadOnly>(
    tx: &AcceptedTransaction<'_>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let Some(executable) = transaction_executable(tx) else {
        return None;
    };
    let mut target_dataspace = None;

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                merge_settlement_target_dataspace(
                    &mut target_dataspace,
                    instruction_settlement_dataspace_target_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    ),
                );
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                merge_settlement_target_dataspace(
                    &mut target_dataspace,
                    instruction_settlement_dataspace_target_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    ),
                );
            }
        }
    }

    target_dataspace
}

fn instruction_settlement_dataspace_target(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let any = instruction.as_any();

    if let Some(dvp) = any.downcast_ref::<DvpIsi>() {
        return settlement_pair_dataspace_target(
            asset_balance_definition_dataspace_target(
                dvp.delivery_leg().asset_definition_id(),
                dataspace_catalog,
                state_view,
            ),
            asset_balance_definition_dataspace_target(
                dvp.payment_leg().asset_definition_id(),
                dataspace_catalog,
                state_view,
            ),
        );
    }

    if let Some(pvp) = any.downcast_ref::<PvpIsi>() {
        return settlement_pair_dataspace_target(
            asset_balance_definition_dataspace_target(
                pvp.primary_leg().asset_definition_id(),
                dataspace_catalog,
                state_view,
            ),
            asset_balance_definition_dataspace_target(
                pvp.counter_leg().asset_definition_id(),
                dataspace_catalog,
                state_view,
            ),
        );
    }

    if let Some(settlement) = any.downcast_ref::<SettlementInstructionBox>() {
        return match settlement {
            SettlementInstructionBox::Dvp(dvp) => settlement_pair_dataspace_target(
                asset_balance_definition_dataspace_target(
                    dvp.delivery_leg().asset_definition_id(),
                    dataspace_catalog,
                    state_view,
                ),
                asset_balance_definition_dataspace_target(
                    dvp.payment_leg().asset_definition_id(),
                    dataspace_catalog,
                    state_view,
                ),
            ),
            SettlementInstructionBox::Pvp(pvp) => settlement_pair_dataspace_target(
                asset_balance_definition_dataspace_target(
                    pvp.primary_leg().asset_definition_id(),
                    dataspace_catalog,
                    state_view,
                ),
                asset_balance_definition_dataspace_target(
                    pvp.counter_leg().asset_definition_id(),
                    dataspace_catalog,
                    state_view,
                ),
            ),
        };
    }

    None
}

fn instruction_settlement_dataspace_target_with_world<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let any = instruction.as_any();

    if let Some(dvp) = any.downcast_ref::<DvpIsi>() {
        return settlement_pair_dataspace_target(
            asset_balance_definition_dataspace_target_with_world(
                dvp.delivery_leg().asset_definition_id(),
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            asset_balance_definition_dataspace_target_with_world(
                dvp.payment_leg().asset_definition_id(),
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
        );
    }

    if let Some(pvp) = any.downcast_ref::<PvpIsi>() {
        return settlement_pair_dataspace_target(
            asset_balance_definition_dataspace_target_with_world(
                pvp.primary_leg().asset_definition_id(),
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            asset_balance_definition_dataspace_target_with_world(
                pvp.counter_leg().asset_definition_id(),
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
        );
    }

    if let Some(settlement) = any.downcast_ref::<SettlementInstructionBox>() {
        return match settlement {
            SettlementInstructionBox::Dvp(dvp) => settlement_pair_dataspace_target(
                asset_balance_definition_dataspace_target_with_world(
                    dvp.delivery_leg().asset_definition_id(),
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                ),
                asset_balance_definition_dataspace_target_with_world(
                    dvp.payment_leg().asset_definition_id(),
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                ),
            ),
            SettlementInstructionBox::Pvp(pvp) => settlement_pair_dataspace_target(
                asset_balance_definition_dataspace_target_with_world(
                    pvp.primary_leg().asset_definition_id(),
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                ),
                asset_balance_definition_dataspace_target_with_world(
                    pvp.counter_leg().asset_definition_id(),
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                ),
            ),
        };
    }

    None
}

fn transaction_executable<'tx>(tx: &'tx AcceptedTransaction<'tx>) -> Option<&'tx Executable> {
    match tx.entrypoint() {
        iroha_data_model::transaction::TransactionEntrypoint::External(signed) => {
            Some(signed.instructions())
        }
        iroha_data_model::transaction::TransactionEntrypoint::SealedCommitment(_) => None,
        iroha_data_model::transaction::TransactionEntrypoint::SealedReveal(reveal) => {
            Some(reveal.signed_transaction().instructions())
        }
        iroha_data_model::transaction::TransactionEntrypoint::PrivateKaigi(_) => None,
        iroha_data_model::transaction::TransactionEntrypoint::Time(_) => None,
    }
}

fn amx_policy_rejects_cross_dataspace(tx: &AcceptedTransaction<'_>) -> bool {
    tx.metadata()
        .and_then(|metadata| metadata.get(AMX_POLICY_METADATA_KEY))
        .and_then(|raw| raw.try_into_any_norito::<String>().ok())
        .is_some_and(|policy| {
            policy
                .trim()
                .eq_ignore_ascii_case(AMX_POLICY_REJECT_CROSS_DATASPACE)
        })
}

#[derive(Clone, Copy)]
enum NativeDataspaceConflict {
    Permission,
    Transaction,
}

fn native_dataspace_conflict_error(
    kind: NativeDataspaceConflict,
    first_dataspace_id: DataSpaceId,
    second_dataspace_id: DataSpaceId,
) -> RoutingResolveError {
    match kind {
        NativeDataspaceConflict::Permission => {
            RoutingResolveError::ConflictingDataspaceScopedPermissions {
                first_dataspace_id,
                second_dataspace_id,
            }
        }
        NativeDataspaceConflict::Transaction => {
            RoutingResolveError::ConflictingTransactionDataspaceTargets {
                first_dataspace_id,
                second_dataspace_id,
            }
        }
    }
}

fn merge_native_target_dataspace(
    target_dataspace: &mut Option<DataSpaceId>,
    candidate: Option<DataSpaceId>,
    reject_cross_dataspace: bool,
    conflict_kind: NativeDataspaceConflict,
) -> Result<(), RoutingResolveError> {
    // TODO: Materialize native AMX descriptors for coordinator-routed native batches once
    // per-dataspace prepare/commit records are wired into block receipts. The universal route
    // currently gives those batches one deterministic StateTransaction commit boundary.
    let Some(candidate) = candidate else {
        return Ok(());
    };

    match *target_dataspace {
        Some(existing) if existing == candidate => {}
        Some(existing) if existing == DataSpaceId::UNIVERSAL => {}
        Some(_) if candidate == DataSpaceId::UNIVERSAL => {
            *target_dataspace = Some(DataSpaceId::UNIVERSAL);
        }
        Some(existing) => {
            if reject_cross_dataspace {
                return Err(native_dataspace_conflict_error(
                    conflict_kind,
                    existing,
                    candidate,
                ));
            }
            *target_dataspace = Some(DataSpaceId::UNIVERSAL);
        }
        None => *target_dataspace = Some(candidate),
    }

    Ok(())
}

#[derive(Clone, Debug, Default)]
struct TransactionDataspaceTarget {
    dataspace_id: Option<DataSpaceId>,
    coordinator_route: bool,
    participants: BTreeSet<DataSpaceId>,
}

fn merge_transaction_target_dataspace(
    target: &mut TransactionDataspaceTarget,
    candidate: Option<DataSpaceId>,
    reject_cross_dataspace: bool,
) -> Result<(), RoutingResolveError> {
    let Some(candidate) = candidate else {
        return Ok(());
    };
    if candidate != DataSpaceId::UNIVERSAL {
        target.participants.insert(candidate);
    }

    match target.dataspace_id {
        Some(existing) if existing == candidate => {}
        Some(existing) => {
            if reject_cross_dataspace {
                return Err(native_dataspace_conflict_error(
                    NativeDataspaceConflict::Transaction,
                    existing,
                    candidate,
                ));
            }
            target.dataspace_id = Some(DataSpaceId::UNIVERSAL);
            target.coordinator_route =
                existing == DataSpaceId::UNIVERSAL || candidate == DataSpaceId::UNIVERSAL;
        }
        None => {
            target.dataspace_id = Some(candidate);
        }
    }

    Ok(())
}

fn apply_authority_dataspace_target(
    target: &mut TransactionDataspaceTarget,
    authority_target: Option<DataSpaceId>,
    allow_universal_override: bool,
) {
    let Some(authority_target) = authority_target else {
        return;
    };

    if target.dataspace_id.is_none() {
        target.dataspace_id = Some(authority_target);
        target.coordinator_route = authority_target == DataSpaceId::UNIVERSAL;
        return;
    }

    if target.dataspace_id == Some(DataSpaceId::UNIVERSAL)
        && authority_target != DataSpaceId::UNIVERSAL
        && allow_universal_override
        && !target.coordinator_route
        && target.participants.is_empty()
    {
        target.dataspace_id = Some(authority_target);
    }
}

fn transaction_dataspace_routing_target(
    tx: &AcceptedTransaction<'_>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Result<Option<DataSpaceId>, RoutingResolveError> {
    transaction_dataspace_routing_target_info(tx, dataspace_catalog, state_view)
        .map(|target| target.dataspace_id)
}

fn transaction_dataspace_routing_target_info(
    tx: &AcceptedTransaction<'_>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Result<TransactionDataspaceTarget, RoutingResolveError> {
    let Some(executable) = transaction_executable(tx) else {
        return Ok(TransactionDataspaceTarget::default());
    };
    let mut target = TransactionDataspaceTarget::default();
    let reject_cross_dataspace = amx_policy_rejects_cross_dataspace(tx);

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                let instruction_target = instruction_transaction_dataspace_target(
                    &**instruction,
                    dataspace_catalog,
                    state_view,
                );
                merge_transaction_target_dataspace(
                    &mut target,
                    instruction_target,
                    reject_cross_dataspace,
                )?;
                if instruction_target == Some(DataSpaceId::UNIVERSAL)
                    && instruction_transaction_target_requires_universal_coordinator(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    )
                {
                    target.coordinator_route = true;
                }
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                let instruction_target = instruction_transaction_dataspace_target(
                    &**instruction,
                    dataspace_catalog,
                    state_view,
                );
                merge_transaction_target_dataspace(
                    &mut target,
                    instruction_target,
                    reject_cross_dataspace,
                )?;
                if instruction_target == Some(DataSpaceId::UNIVERSAL)
                    && instruction_transaction_target_requires_universal_coordinator(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    )
                {
                    target.coordinator_route = true;
                }
            }
        }
    }

    Ok(target)
}

fn transaction_dataspace_routing_target_info_with_world<W: WorldReadOnly>(
    tx: &AcceptedTransaction<'_>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<TransactionDataspaceTarget, RoutingResolveError> {
    let Some(executable) = transaction_executable(tx) else {
        return Ok(TransactionDataspaceTarget::default());
    };
    let mut target = TransactionDataspaceTarget::default();
    let reject_cross_dataspace = amx_policy_rejects_cross_dataspace(tx);

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                let instruction_target = instruction_transaction_dataspace_target_with_world(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                );
                merge_transaction_target_dataspace(
                    &mut target,
                    instruction_target,
                    reject_cross_dataspace,
                )?;
                if instruction_target == Some(DataSpaceId::UNIVERSAL)
                    && instruction_transaction_target_requires_universal_coordinator_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                {
                    target.coordinator_route = true;
                }
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                let instruction_target = instruction_transaction_dataspace_target_with_world(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                );
                merge_transaction_target_dataspace(
                    &mut target,
                    instruction_target,
                    reject_cross_dataspace,
                )?;
                if instruction_target == Some(DataSpaceId::UNIVERSAL)
                    && instruction_transaction_target_requires_universal_coordinator_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                {
                    target.coordinator_route = true;
                }
            }
        }
    }

    Ok(target)
}

/// Return the concrete dataspace participants of a native AMX candidate.
///
/// This is intentionally narrower than route resolution: it preserves the
/// per-dataspace legs that make a native transaction require a coordinator
/// route so block commitments can expose deterministic prepare/commit evidence.
#[allow(dead_code)]
pub(crate) fn native_amx_participant_dataspaces_with_world<W: WorldReadOnly>(
    tx: &AcceptedTransaction<'_>,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
) -> Vec<DataSpaceId> {
    let mut dataspaces = std::collections::BTreeSet::new();
    let Some(executable) = transaction_executable(tx) else {
        return Vec::new();
    };

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                collect_instruction_native_amx_participants(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    &mut dataspaces,
                );
            }
        }
        Executable::ContractCall(call) => {
            insert_native_amx_participant(
                &mut dataspaces,
                contract_address_dataspace_target(&call.contract_address),
            );
        }
        Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                collect_instruction_native_amx_participants(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    &mut dataspaces,
                );
            }
        }
    }

    dataspaces.into_iter().collect()
}

fn insert_native_amx_participant(
    dataspaces: &mut std::collections::BTreeSet<DataSpaceId>,
    target: Option<DataSpaceId>,
) {
    if let Some(dataspace) = target
        && dataspace != DataSpaceId::UNIVERSAL
    {
        dataspaces.insert(dataspace);
    }
}

fn collect_asset_balance_native_amx_participants<I>(
    dataspaces: &mut std::collections::BTreeSet<DataSpaceId>,
    definition_target: AssetBalanceDefinitionRouteTarget,
    explicit_asset_target: Option<DataSpaceId>,
    account_targets: I,
) where
    I: IntoIterator<Item = Option<DataSpaceId>>,
{
    insert_native_amx_participant(dataspaces, definition_target.dataspace_id);
    if definition_target.balance_scope_policy == Some(AssetBalancePolicy::Global) {
        return;
    }
    insert_native_amx_participant(dataspaces, explicit_asset_target);
    for account_target in account_targets {
        insert_native_amx_participant(dataspaces, account_target);
    }
}

fn collect_instruction_native_amx_participants<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    dataspaces: &mut std::collections::BTreeSet<DataSpaceId>,
) {
    insert_native_amx_participant(
        dataspaces,
        instruction_dataspace_scoped_permission_target_with_world(
            instruction,
            Some(dataspace_catalog),
            world,
            None,
        ),
    );

    let any = instruction.as_any();
    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        if let TransferBox::Asset(transfer) = transfer {
            collect_asset_balance_native_amx_participants(
                dataspaces,
                asset_balance_definition_route_target_with_world(
                    &transfer.source.definition,
                    Some(dataspace_catalog),
                    world,
                    None,
                ),
                asset_id_explicit_dataspace_target(&transfer.source),
                [
                    account_dataspace_target(Some(world), &transfer.source.account),
                    account_dataspace_target(Some(world), &transfer.destination),
                ],
            );
            return;
        }
    }

    if let Some(mint) = any.downcast_ref::<MintBox>() {
        if let MintBox::Asset(mint) = mint {
            collect_asset_balance_native_amx_participants(
                dataspaces,
                asset_balance_definition_route_target_with_world(
                    &mint.destination.definition,
                    Some(dataspace_catalog),
                    world,
                    None,
                ),
                asset_id_explicit_dataspace_target(&mint.destination),
                [account_dataspace_target(
                    Some(world),
                    &mint.destination.account,
                )],
            );
            return;
        }
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>() {
        if let BurnBox::Asset(burn) = burn {
            collect_asset_balance_native_amx_participants(
                dataspaces,
                asset_balance_definition_route_target_with_world(
                    &burn.destination.definition,
                    Some(dataspace_catalog),
                    world,
                    None,
                ),
                asset_id_explicit_dataspace_target(&burn.destination),
                [account_dataspace_target(
                    Some(world),
                    &burn.destination.account,
                )],
            );
            return;
        }
    }

    insert_native_amx_participant(
        dataspaces,
        instruction_transaction_dataspace_target_with_world(
            instruction,
            Some(dataspace_catalog),
            world,
            None,
        ),
    );
}

enum AccountPermissionHolderTarget<'account> {
    Holder(&'account AccountId),
    Skip,
    Abort,
}

fn account_permission_holder_routing_target<'tx>(
    tx: &'tx AcceptedTransaction<'tx>,
) -> Option<&'tx AccountId> {
    let Some(executable) = transaction_executable(tx) else {
        return None;
    };

    match executable {
        Executable::Instructions(instructions) => account_permission_holder_from_instructions(
            instructions.iter().map(|instruction| &**instruction),
        ),
        Executable::ContractCall(_) | Executable::Ivm(_) => None,
        Executable::IvmProved(proved) => account_permission_holder_from_instructions(
            proved.overlay.iter().map(|instruction| &**instruction),
        ),
    }
}

fn account_permission_holder_from_instructions<'instruction, I>(
    instructions: I,
) -> Option<&'instruction AccountId>
where
    I: IntoIterator<Item = &'instruction dyn Instruction>,
{
    let mut holder: Option<&AccountId> = None;
    let mut saw_account_permission = false;

    for instruction in instructions {
        match instruction_account_permission_holder(instruction) {
            AccountPermissionHolderTarget::Holder(candidate) => {
                saw_account_permission = true;
                match holder {
                    Some(existing) if existing != candidate => return None,
                    Some(_) => {}
                    None => {
                        holder = Some(candidate);
                    }
                }
            }
            AccountPermissionHolderTarget::Skip | AccountPermissionHolderTarget::Abort => {
                return None;
            }
        }
    }

    if saw_account_permission { holder } else { None }
}

fn instruction_account_permission_holder(
    instruction: &dyn Instruction,
) -> AccountPermissionHolderTarget<'_> {
    let any = instruction.as_any();

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        return match grant {
            GrantBox::Permission(grant) => {
                if dataspace_scoped_permission_target(&grant.object, None, None).is_some() {
                    AccountPermissionHolderTarget::Skip
                } else {
                    AccountPermissionHolderTarget::Holder(&grant.destination)
                }
            }
            GrantBox::Role(_) | GrantBox::RolePermission(_) => AccountPermissionHolderTarget::Abort,
        };
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return match revoke {
            RevokeBox::Permission(revoke) => {
                if dataspace_scoped_permission_target(&revoke.object, None, None).is_some() {
                    AccountPermissionHolderTarget::Skip
                } else {
                    AccountPermissionHolderTarget::Holder(&revoke.destination)
                }
            }
            RevokeBox::Role(_) | RevokeBox::RolePermission(_) => {
                AccountPermissionHolderTarget::Abort
            }
        };
    }

    AccountPermissionHolderTarget::Abort
}

fn instruction_transaction_dataspace_target(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let any = instruction.as_any();

    if let Some(register) = any.downcast_ref::<RegisterBox>() {
        return match register {
            RegisterBox::Domain(register) => domain_dataspace_target_with_state(
                &register.object.id,
                dataspace_catalog,
                state_view,
            ),
            RegisterBox::Account(register) => {
                register.object.label.as_ref().map(|alias| alias.dataspace)
            }
            RegisterBox::AssetDefinition(register) => asset_definition_dataspace_target(
                &register.object.id,
                register.object.alias.as_ref(),
                Some(register.object.balance_scope_policy),
                dataspace_catalog,
                state_view,
            ),
            RegisterBox::Peer(_)
            | RegisterBox::Nft(_)
            | RegisterBox::Role(_)
            | RegisterBox::Trigger(_) => None,
        };
    }

    if let Some(unregister) = any.downcast_ref::<UnregisterBox>() {
        return match unregister {
            UnregisterBox::Domain(unregister) => domain_dataspace_target_with_state(
                &unregister.object,
                dataspace_catalog,
                state_view,
            ),
            UnregisterBox::AssetDefinition(unregister) => asset_definition_dataspace_target(
                &unregister.object,
                None,
                None,
                dataspace_catalog,
                state_view,
            ),
            UnregisterBox::Peer(_)
            | UnregisterBox::Account(_)
            | UnregisterBox::Nft(_)
            | UnregisterBox::Role(_)
            | UnregisterBox::Trigger(_) => None,
        };
    }

    if let Some(set_key_value) = any.downcast_ref::<SetKeyValueBox>() {
        return match set_key_value {
            SetKeyValueBox::Domain(set) => {
                domain_dataspace_target_with_state(&set.object, dataspace_catalog, state_view)
            }
            SetKeyValueBox::Account(set) => {
                account_dataspace_target(state_view.map(StateView::world), &set.object)
            }
            SetKeyValueBox::AssetDefinition(set) => asset_definition_dataspace_target(
                &set.object,
                None,
                None,
                dataspace_catalog,
                state_view,
            ),
            SetKeyValueBox::Nft(_) | SetKeyValueBox::Trigger(_) => None,
        };
    }

    if let Some(remove_key_value) = any.downcast_ref::<RemoveKeyValueBox>() {
        return match remove_key_value {
            RemoveKeyValueBox::Domain(remove) => {
                domain_dataspace_target_with_state(&remove.object, dataspace_catalog, state_view)
            }
            RemoveKeyValueBox::Account(remove) => {
                account_dataspace_target(state_view.map(StateView::world), &remove.object)
            }
            RemoveKeyValueBox::AssetDefinition(remove) => asset_definition_dataspace_target(
                &remove.object,
                None,
                None,
                dataspace_catalog,
                state_view,
            ),
            RemoveKeyValueBox::Nft(_) | RemoveKeyValueBox::Trigger(_) => None,
        };
    }

    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        return match transfer {
            TransferBox::Domain(transfer) => {
                domain_dataspace_target_with_state(&transfer.object, dataspace_catalog, state_view)
            }
            TransferBox::AssetDefinition(transfer) => asset_definition_dataspace_target(
                &transfer.object,
                None,
                None,
                dataspace_catalog,
                state_view,
            ),
            TransferBox::Asset(transfer) => asset_balance_operation_dataspace_target(
                asset_balance_definition_route_target(
                    &transfer.source.definition,
                    dataspace_catalog,
                    state_view,
                ),
                asset_id_explicit_dataspace_target(&transfer.source),
                [
                    account_dataspace_target(
                        state_view.map(StateView::world),
                        &transfer.source.account,
                    ),
                    account_dataspace_target(
                        state_view.map(StateView::world),
                        &transfer.destination,
                    ),
                ],
            ),
            TransferBox::Nft(_) => None,
        };
    }

    if let Some(mint) = any.downcast_ref::<MintBox>() {
        return match mint {
            MintBox::Asset(mint) => asset_balance_operation_dataspace_target(
                asset_balance_definition_route_target(
                    &mint.destination.definition,
                    dataspace_catalog,
                    state_view,
                ),
                asset_id_explicit_dataspace_target(&mint.destination),
                [account_dataspace_target(
                    state_view.map(StateView::world),
                    &mint.destination.account,
                )],
            ),
            MintBox::TriggerRepetitions(_) => None,
        };
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>() {
        return match burn {
            BurnBox::Asset(burn) => asset_balance_operation_dataspace_target(
                asset_balance_definition_route_target(
                    &burn.destination.definition,
                    dataspace_catalog,
                    state_view,
                ),
                asset_id_explicit_dataspace_target(&burn.destination),
                [account_dataspace_target(
                    state_view.map(StateView::world),
                    &burn.destination.account,
                )],
            ),
            BurnBox::TriggerRepetitions(_) => None,
        };
    }

    if let Some(set_policy) = any.downcast_ref::<SetAssetDefinitionBalancePolicy>() {
        return asset_definition_dataspace_target(
            &set_policy.asset_definition_id,
            None,
            None,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(publish) = any.downcast_ref::<PublishMusubiRelease>() {
        return musubi_package_dataspace_target_with_state(
            &publish.release.package.package,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(yank) = any.downcast_ref::<YankMusubiRelease>() {
        return musubi_package_dataspace_target_with_state(
            &yank.package.package,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(set_alias) = any.downcast_ref::<SetMusubiShortAlias>() {
        return musubi_package_dataspace_target_with_state(
            &set_alias.alias.target,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(assert_release) = any.downcast_ref::<AssertMusubiReleaseExists>() {
        return musubi_package_dataspace_target_with_state(
            &assert_release.package,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(activate) = any.downcast_ref::<ActivateContractInstance>() {
        return contract_address_dataspace_target(&activate.contract_address);
    }

    if let Some(deactivate) = any.downcast_ref::<DeactivateContractInstance>() {
        return contract_address_dataspace_target(&deactivate.contract_address);
    }

    if let Some(set_alias) = any.downcast_ref::<SetContractAlias>() {
        return contract_address_dataspace_target(&set_alias.contract_address);
    }

    if let Some(asset_definition_id) = offline_note_asset_definition_target(any) {
        return asset_definition_dataspace_target(
            asset_definition_id,
            None,
            None,
            dataspace_catalog,
            state_view,
        );
    }

    None
}

fn instruction_transaction_dataspace_target_with_world<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let any = instruction.as_any();

    if let Some(register) = any.downcast_ref::<RegisterBox>() {
        return match register {
            RegisterBox::Domain(register) => domain_dataspace_target_with_world(
                &register.object.id,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            RegisterBox::Account(register) => {
                register.object.label.as_ref().map(|alias| alias.dataspace)
            }
            RegisterBox::AssetDefinition(register) => asset_definition_dataspace_target_with_world(
                &register.object.id,
                register.object.alias.as_ref(),
                Some(register.object.balance_scope_policy),
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            RegisterBox::Peer(_)
            | RegisterBox::Nft(_)
            | RegisterBox::Role(_)
            | RegisterBox::Trigger(_) => None,
        };
    }

    if let Some(unregister) = any.downcast_ref::<UnregisterBox>() {
        return match unregister {
            UnregisterBox::Domain(unregister) => domain_dataspace_target_with_world(
                &unregister.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            UnregisterBox::AssetDefinition(unregister) => {
                asset_definition_dataspace_target_with_world(
                    &unregister.object,
                    None,
                    None,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                )
            }
            UnregisterBox::Peer(_)
            | UnregisterBox::Account(_)
            | UnregisterBox::Nft(_)
            | UnregisterBox::Role(_)
            | UnregisterBox::Trigger(_) => None,
        };
    }

    if let Some(set_key_value) = any.downcast_ref::<SetKeyValueBox>() {
        return match set_key_value {
            SetKeyValueBox::Domain(set) => domain_dataspace_target_with_world(
                &set.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            SetKeyValueBox::Account(set) => account_dataspace_target(Some(world), &set.object),
            SetKeyValueBox::AssetDefinition(set) => asset_definition_dataspace_target_with_world(
                &set.object,
                None,
                None,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            SetKeyValueBox::Nft(_) | SetKeyValueBox::Trigger(_) => None,
        };
    }

    if let Some(remove_key_value) = any.downcast_ref::<RemoveKeyValueBox>() {
        return match remove_key_value {
            RemoveKeyValueBox::Domain(remove) => domain_dataspace_target_with_world(
                &remove.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            RemoveKeyValueBox::Account(remove) => {
                account_dataspace_target(Some(world), &remove.object)
            }
            RemoveKeyValueBox::AssetDefinition(remove) => {
                asset_definition_dataspace_target_with_world(
                    &remove.object,
                    None,
                    None,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                )
            }
            RemoveKeyValueBox::Nft(_) | RemoveKeyValueBox::Trigger(_) => None,
        };
    }

    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        return match transfer {
            TransferBox::Domain(transfer) => domain_dataspace_target_with_world(
                &transfer.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            TransferBox::AssetDefinition(transfer) => asset_definition_dataspace_target_with_world(
                &transfer.object,
                None,
                None,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            TransferBox::Asset(transfer) => asset_balance_operation_dataspace_target(
                asset_balance_definition_route_target_with_world(
                    &transfer.source.definition,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                ),
                asset_id_explicit_dataspace_target(&transfer.source),
                [
                    account_dataspace_target(Some(world), &transfer.source.account),
                    account_dataspace_target(Some(world), &transfer.destination),
                ],
            ),
            TransferBox::Nft(_) => None,
        };
    }

    if let Some(mint) = any.downcast_ref::<MintBox>() {
        return match mint {
            MintBox::Asset(mint) => asset_balance_operation_dataspace_target(
                asset_balance_definition_route_target_with_world(
                    &mint.destination.definition,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                ),
                asset_id_explicit_dataspace_target(&mint.destination),
                [account_dataspace_target(
                    Some(world),
                    &mint.destination.account,
                )],
            ),
            MintBox::TriggerRepetitions(_) => None,
        };
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>() {
        return match burn {
            BurnBox::Asset(burn) => asset_balance_operation_dataspace_target(
                asset_balance_definition_route_target_with_world(
                    &burn.destination.definition,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                ),
                asset_id_explicit_dataspace_target(&burn.destination),
                [account_dataspace_target(
                    Some(world),
                    &burn.destination.account,
                )],
            ),
            BurnBox::TriggerRepetitions(_) => None,
        };
    }

    if let Some(set_policy) = any.downcast_ref::<SetAssetDefinitionBalancePolicy>() {
        return asset_definition_dataspace_target_with_world(
            &set_policy.asset_definition_id,
            None,
            None,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(publish) = any.downcast_ref::<PublishMusubiRelease>() {
        return musubi_package_dataspace_target_with_world(
            &publish.release.package.package,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(yank) = any.downcast_ref::<YankMusubiRelease>() {
        return musubi_package_dataspace_target_with_world(
            &yank.package.package,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(set_alias) = any.downcast_ref::<SetMusubiShortAlias>() {
        return musubi_package_dataspace_target_with_world(
            &set_alias.alias.target,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(assert_release) = any.downcast_ref::<AssertMusubiReleaseExists>() {
        return musubi_package_dataspace_target_with_world(
            &assert_release.package,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(activate) = any.downcast_ref::<ActivateContractInstance>() {
        return contract_address_dataspace_target(&activate.contract_address);
    }

    if let Some(deactivate) = any.downcast_ref::<DeactivateContractInstance>() {
        return contract_address_dataspace_target(&deactivate.contract_address);
    }

    if let Some(set_alias) = any.downcast_ref::<SetContractAlias>() {
        return contract_address_dataspace_target(&set_alias.contract_address);
    }

    if let Some(asset_definition_id) = offline_note_asset_definition_target(any) {
        return asset_definition_dataspace_target_with_world(
            asset_definition_id,
            None,
            None,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    None
}

fn offline_note_asset_definition_target(any: &dyn std::any::Any) -> Option<&AssetDefinitionId> {
    if let Some(issue) = any.downcast_ref::<IssueOfflineNote>() {
        return Some(issue.issue.asset.definition());
    }
    if let Some(redemption) = any.downcast_ref::<RedeemOfflineNote>() {
        return Some(redemption.redemption.asset.definition());
    }
    if let Some(audit) = any.downcast_ref::<AuditOfflineNote>() {
        return audit
            .audit
            .input_claims
            .first()
            .map(|claim| claim.asset.definition())
            .or_else(|| {
                audit
                    .audit
                    .output_claims
                    .first()
                    .map(|claim| claim.asset.definition())
            });
    }
    if let Some(transfer) = any.downcast_ref::<KagemushaTransfer>() {
        return Some(&transfer.asset);
    }
    None
}

fn instruction_transaction_target_requires_universal_coordinator(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> bool {
    let any = instruction.as_any();

    if let Some(transfer) = any.downcast_ref::<TransferBox>()
        && let TransferBox::Asset(transfer) = transfer
    {
        return asset_balance_definition_route_target(
            &transfer.source.definition,
            dataspace_catalog,
            state_view,
        )
        .balance_scope_policy
            == Some(AssetBalancePolicy::Global);
    }

    if let Some(mint) = any.downcast_ref::<MintBox>()
        && let MintBox::Asset(mint) = mint
    {
        return asset_balance_definition_route_target(
            &mint.destination.definition,
            dataspace_catalog,
            state_view,
        )
        .balance_scope_policy
            == Some(AssetBalancePolicy::Global);
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>()
        && let BurnBox::Asset(burn) = burn
    {
        return asset_balance_definition_route_target(
            &burn.destination.definition,
            dataspace_catalog,
            state_view,
        )
        .balance_scope_policy
            == Some(AssetBalancePolicy::Global);
    }

    false
}

fn instruction_transaction_target_requires_universal_coordinator_with_world<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> bool {
    let any = instruction.as_any();

    if let Some(transfer) = any.downcast_ref::<TransferBox>()
        && let TransferBox::Asset(transfer) = transfer
    {
        return asset_balance_definition_route_target_with_world(
            &transfer.source.definition,
            dataspace_catalog,
            world,
            ledger_time_ms,
        )
        .balance_scope_policy
            == Some(AssetBalancePolicy::Global);
    }

    if let Some(mint) = any.downcast_ref::<MintBox>()
        && let MintBox::Asset(mint) = mint
    {
        return asset_balance_definition_route_target_with_world(
            &mint.destination.definition,
            dataspace_catalog,
            world,
            ledger_time_ms,
        )
        .balance_scope_policy
            == Some(AssetBalancePolicy::Global);
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>()
        && let BurnBox::Asset(burn) = burn
    {
        return asset_balance_definition_route_target_with_world(
            &burn.destination.definition,
            dataspace_catalog,
            world,
            ledger_time_ms,
        )
        .balance_scope_policy
            == Some(AssetBalancePolicy::Global);
    }

    false
}

fn account_dataspace_target<W: WorldReadOnly>(
    world: Option<&W>,
    account_id: &AccountId,
) -> Option<DataSpaceId> {
    let world = world?;
    let hierarchy = world.account_scope_hierarchy(account_id).ok()?;
    if hierarchy.len() > 1 {
        return Some(DataSpaceId::UNIVERSAL);
    }
    let dataspace_id = *hierarchy.keys().next().expect("single dataspace");
    (dataspace_id != DataSpaceId::UNIVERSAL).then_some(dataspace_id)
}

fn authority_dataspace_target(
    state_view: Option<&StateView<'_>>,
    tx: &AcceptedTransaction<'_>,
) -> Option<DataSpaceId> {
    tx.authority_opt()
        .and_then(|authority| account_dataspace_target(state_view.map(StateView::world), authority))
}

fn authority_dataspace_target_with_world<W: WorldReadOnly>(
    world: Option<&W>,
    tx: &AcceptedTransaction<'_>,
) -> Option<DataSpaceId> {
    tx.authority_opt()
        .and_then(|authority| account_dataspace_target(world, authority))
}

fn domain_dataspace_target_with_state(
    domain_id: &DomainId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    dataspace_alias_target_with_state(
        domain_id.dataspace().as_ref(),
        dataspace_catalog,
        state_view,
    )
}

fn domain_dataspace_target_with_world<W: WorldReadOnly>(
    domain_id: &DomainId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    dataspace_alias_target_with_world(
        domain_id.dataspace().as_ref(),
        dataspace_catalog,
        world,
        ledger_time_ms,
    )
}

fn contract_address_dataspace_target(contract_address: &ContractAddress) -> Option<DataSpaceId> {
    contract_address.dataspace_id().ok()
}

fn musubi_namespace_dataspace_target_with_state(
    namespace: &MusubiNamespace,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    dataspace_alias_target_with_state(namespace.dataspace_segment(), dataspace_catalog, state_view)
}

fn musubi_namespace_dataspace_target_with_world<W: WorldReadOnly>(
    namespace: &MusubiNamespace,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    dataspace_alias_target_with_world(
        namespace.dataspace_segment(),
        dataspace_catalog,
        world,
        ledger_time_ms,
    )
}

fn dataspace_alias_target(
    dataspace_alias: &str,
    dataspace_catalog: Option<&DataSpaceCatalog>,
) -> Option<DataSpaceId> {
    if dataspace_alias.eq_ignore_ascii_case("universal") {
        return Some(DataSpaceId::UNIVERSAL);
    }
    dataspace_catalog?
        .by_alias(dataspace_alias)
        .map(|entry| entry.id)
}

fn dataspace_alias_target_with_state(
    dataspace_alias: &str,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let Some(view) = state_view else {
        return dataspace_alias_target(dataspace_alias, dataspace_catalog);
    };
    let catalog = dataspace_catalog?;
    crate::sns::active_dataspace_id_by_alias(
        view.world(),
        catalog,
        dataspace_alias,
        state_view_ledger_time_ms(view),
    )
    .or_else(|| dataspace_alias_target(dataspace_alias, Some(catalog)))
}

fn dataspace_alias_target_with_world<W: WorldReadOnly>(
    dataspace_alias: &str,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let catalog = dataspace_catalog?;
    ledger_time_ms
        .and_then(|now_ms| {
            crate::sns::active_dataspace_id_by_alias(world, catalog, dataspace_alias, now_ms)
        })
        .or_else(|| dataspace_alias_target(dataspace_alias, Some(catalog)))
}

fn state_view_ledger_time_ms(state_view: &StateView<'_>) -> u64 {
    state_view
        .latest_block()
        .as_ref()
        .map(|block| u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn musubi_package_dataspace_target_with_state(
    package: &MusubiPackageId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    musubi_namespace_dataspace_target_with_state(&package.namespace, dataspace_catalog, state_view)
}

fn musubi_package_dataspace_target_with_world<W: WorldReadOnly>(
    package: &MusubiPackageId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    musubi_namespace_dataspace_target_with_world(
        &package.namespace,
        dataspace_catalog,
        world,
        ledger_time_ms,
    )
}

fn asset_definition_target_from_parts_with_state(
    asset_definition_id: &AssetDefinitionId,
    alias: Option<&AssetDefinitionAlias>,
    balance_scope_policy: Option<AssetBalancePolicy>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let dataspace_alias = alias
        .map(|alias| alias.dataspace_segment().to_owned())
        .or_else(|| {
            asset_definition_id
                .try_domain()
                .map(|domain| domain.dataspace().as_ref().to_owned())
        });
    let Some(dataspace_alias) = dataspace_alias else {
        return balance_scope_policy
            .is_some_and(|policy| policy == AssetBalancePolicy::Global)
            .then_some(DataSpaceId::UNIVERSAL);
    };
    dataspace_alias_target_with_state(&dataspace_alias, dataspace_catalog, state_view)
}

fn asset_definition_target_from_parts_with_world<W: WorldReadOnly>(
    asset_definition_id: &AssetDefinitionId,
    alias: Option<&AssetDefinitionAlias>,
    balance_scope_policy: Option<AssetBalancePolicy>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let dataspace_alias = alias
        .map(|alias| alias.dataspace_segment().to_owned())
        .or_else(|| {
            asset_definition_id
                .try_domain()
                .map(|domain| domain.dataspace().as_ref().to_owned())
        });
    let Some(dataspace_alias) = dataspace_alias else {
        return balance_scope_policy
            .is_some_and(|policy| policy == AssetBalancePolicy::Global)
            .then_some(DataSpaceId::UNIVERSAL);
    };
    dataspace_alias_target_with_world(&dataspace_alias, dataspace_catalog, world, ledger_time_ms)
}

fn instruction_transaction_dataspace_target_needs_state(instruction: &dyn Instruction) -> bool {
    let any = instruction.as_any();

    if let Some(dvp) = any.downcast_ref::<DvpIsi>() {
        return dvp
            .delivery_leg()
            .asset_definition_id()
            .is_opaque_canonical()
            || dvp
                .payment_leg()
                .asset_definition_id()
                .is_opaque_canonical();
    }

    if let Some(pvp) = any.downcast_ref::<PvpIsi>() {
        return pvp
            .primary_leg()
            .asset_definition_id()
            .is_opaque_canonical()
            || pvp
                .counter_leg()
                .asset_definition_id()
                .is_opaque_canonical();
    }

    if let Some(settlement) = any.downcast_ref::<SettlementInstructionBox>() {
        return match settlement {
            SettlementInstructionBox::Dvp(dvp) => {
                dvp.delivery_leg()
                    .asset_definition_id()
                    .is_opaque_canonical()
                    || dvp
                        .payment_leg()
                        .asset_definition_id()
                        .is_opaque_canonical()
            }
            SettlementInstructionBox::Pvp(pvp) => {
                pvp.primary_leg()
                    .asset_definition_id()
                    .is_opaque_canonical()
                    || pvp
                        .counter_leg()
                        .asset_definition_id()
                        .is_opaque_canonical()
            }
        };
    }

    if let Some(unregister) = any.downcast_ref::<UnregisterBox>() {
        return matches!(unregister, UnregisterBox::AssetDefinition(_));
    }

    if let Some(set_key_value) = any.downcast_ref::<SetKeyValueBox>() {
        return matches!(set_key_value, SetKeyValueBox::Account(_))
            || matches!(set_key_value, SetKeyValueBox::AssetDefinition(_));
    }

    if let Some(remove_key_value) = any.downcast_ref::<RemoveKeyValueBox>() {
        return matches!(remove_key_value, RemoveKeyValueBox::Account(_))
            || matches!(remove_key_value, RemoveKeyValueBox::AssetDefinition(_));
    }

    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        return match transfer {
            TransferBox::AssetDefinition(_) | TransferBox::Asset(_) => true,
            TransferBox::Domain(_) | TransferBox::Nft(_) => false,
        };
    }

    if let Some(mint) = any.downcast_ref::<MintBox>() {
        return matches!(mint, MintBox::Asset(_));
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>() {
        return matches!(burn, BurnBox::Asset(_));
    }

    if any
        .downcast_ref::<SetAssetDefinitionBalancePolicy>()
        .is_some()
    {
        return true;
    }

    if let Some(asset_definition_id) = offline_note_asset_definition_target(any) {
        return asset_definition_id.is_opaque_canonical();
    }

    false
}

fn instruction_dataspace_scoped_permission_target(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let any = instruction.as_any();

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        return match grant {
            GrantBox::Permission(grant) => {
                dataspace_scoped_permission_target(&grant.object, dataspace_catalog, state_view)
            }
            GrantBox::Role(_) | GrantBox::RolePermission(_) => None,
        };
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return match revoke {
            RevokeBox::Permission(revoke) => {
                dataspace_scoped_permission_target(&revoke.object, dataspace_catalog, state_view)
            }
            RevokeBox::Role(_) | RevokeBox::RolePermission(_) => None,
        };
    }

    None
}

fn instruction_dataspace_scoped_permission_target_with_world<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let any = instruction.as_any();

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        return match grant {
            GrantBox::Permission(grant) => dataspace_scoped_permission_target_with_world(
                &grant.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            GrantBox::Role(_) | GrantBox::RolePermission(_) => None,
        };
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return match revoke {
            RevokeBox::Permission(revoke) => dataspace_scoped_permission_target_with_world(
                &revoke.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            RevokeBox::Role(_) | RevokeBox::RolePermission(_) => None,
        };
    }

    None
}

fn asset_definition_dataspace_target(
    asset_definition_id: &AssetDefinitionId,
    alias: Option<&AssetDefinitionAlias>,
    balance_scope_policy: Option<AssetBalancePolicy>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let resolved = state_view
        .and_then(|view| asset_definition_for_routing(&view.world, asset_definition_id))
        .map(|definition| {
            let balance_scope_policy = definition.balance_scope_policy();
            (definition.id, balance_scope_policy, definition.alias)
        });
    let effective_id = resolved
        .as_ref()
        .map(|(resolved_id, _, _)| resolved_id)
        .unwrap_or(asset_definition_id);
    let effective_alias = resolved
        .as_ref()
        .and_then(|(_, _, resolved_alias)| resolved_alias.as_ref())
        .or(alias);
    let effective_policy = resolved
        .as_ref()
        .map(|(_, policy, _)| *policy)
        .or(balance_scope_policy);
    asset_definition_target_from_parts_with_state(
        effective_id,
        effective_alias,
        effective_policy,
        dataspace_catalog,
        state_view,
    )
}

fn asset_definition_dataspace_target_with_world<W: WorldReadOnly>(
    asset_definition_id: &AssetDefinitionId,
    alias: Option<&AssetDefinitionAlias>,
    balance_scope_policy: Option<AssetBalancePolicy>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let resolved = asset_definition_for_routing(world, asset_definition_id).map(|definition| {
        let balance_scope_policy = definition.balance_scope_policy();
        (definition.id, balance_scope_policy, definition.alias)
    });
    let effective_id = resolved
        .as_ref()
        .map(|(resolved_id, _, _)| resolved_id)
        .unwrap_or(asset_definition_id);
    let effective_alias = resolved
        .as_ref()
        .and_then(|(_, _, resolved_alias)| resolved_alias.as_ref())
        .or(alias);
    let effective_policy = resolved
        .as_ref()
        .map(|(_, policy, _)| *policy)
        .or(balance_scope_policy);
    asset_definition_target_from_parts_with_world(
        effective_id,
        effective_alias,
        effective_policy,
        dataspace_catalog,
        world,
        ledger_time_ms,
    )
}

fn asset_balance_definition_route_target(
    asset_definition_id: &AssetDefinitionId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> AssetBalanceDefinitionRouteTarget {
    let resolved = state_view
        .and_then(|view| asset_definition_for_balance_routing(&view.world, asset_definition_id))
        .map(|definition| {
            let balance_scope_policy = definition.balance_scope_policy();
            (definition.id, balance_scope_policy, definition.alias)
        });
    let effective_id = resolved
        .as_ref()
        .map(|(resolved_id, _, _)| resolved_id)
        .unwrap_or(asset_definition_id);
    let effective_alias = resolved
        .as_ref()
        .and_then(|(_, _, resolved_alias)| resolved_alias.as_ref());
    let effective_policy = resolved.as_ref().map(|(_, policy, _)| *policy);
    let dataspace_id = if effective_policy == Some(AssetBalancePolicy::Global) {
        Some(DataSpaceId::UNIVERSAL)
    } else {
        asset_definition_target_from_parts_with_state(
            effective_id,
            effective_alias,
            effective_policy,
            dataspace_catalog,
            state_view,
        )
    };
    AssetBalanceDefinitionRouteTarget {
        dataspace_id,
        balance_scope_policy: effective_policy,
    }
}

fn asset_balance_definition_dataspace_target(
    asset_definition_id: &AssetDefinitionId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    asset_balance_definition_route_target(asset_definition_id, dataspace_catalog, state_view)
        .dataspace_id
}

fn asset_balance_definition_route_target_with_world<W: WorldReadOnly>(
    asset_definition_id: &AssetDefinitionId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> AssetBalanceDefinitionRouteTarget {
    let resolved =
        asset_definition_for_balance_routing(world, asset_definition_id).map(|definition| {
            let balance_scope_policy = definition.balance_scope_policy();
            (definition.id, balance_scope_policy, definition.alias)
        });
    let effective_id = resolved
        .as_ref()
        .map(|(resolved_id, _, _)| resolved_id)
        .unwrap_or(asset_definition_id);
    let effective_alias = resolved
        .as_ref()
        .and_then(|(_, _, resolved_alias)| resolved_alias.as_ref());
    let effective_policy = resolved.as_ref().map(|(_, policy, _)| *policy);
    let dataspace_id = if effective_policy == Some(AssetBalancePolicy::Global) {
        Some(DataSpaceId::UNIVERSAL)
    } else {
        asset_definition_target_from_parts_with_world(
            effective_id,
            effective_alias,
            effective_policy,
            dataspace_catalog,
            world,
            ledger_time_ms,
        )
    };
    AssetBalanceDefinitionRouteTarget {
        dataspace_id,
        balance_scope_policy: effective_policy,
    }
}

fn asset_balance_definition_dataspace_target_with_world<W: WorldReadOnly>(
    asset_definition_id: &AssetDefinitionId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    asset_balance_definition_route_target_with_world(
        asset_definition_id,
        dataspace_catalog,
        world,
        ledger_time_ms,
    )
    .dataspace_id
}

fn asset_definition_for_routing<W: WorldReadOnly>(
    world: &W,
    asset_definition_id: &AssetDefinitionId,
) -> Option<AssetDefinition> {
    world
        .asset_definition(asset_definition_id)
        .ok()
        .or_else(|| {
            world
                .asset_definitions_iter()
                .find(|definition| definition.id == *asset_definition_id)
                .cloned()
                .map(|mut definition| {
                    definition.alias = world
                        .asset_definition_alias_bindings()
                        .get(&definition.id)
                        .map(|binding| binding.alias.clone());
                    definition
                })
        })
}

fn asset_definition_for_balance_routing<W: WorldReadOnly>(
    world: &W,
    asset_definition_id: &AssetDefinitionId,
) -> Option<AssetDefinition> {
    let mut definition = world
        .asset_definitions_iter()
        .find(|definition| definition.id == *asset_definition_id)
        .cloned()
        .or_else(|| world.asset_definition(asset_definition_id).ok())?;

    if definition.balance_scope_policy() == AssetBalancePolicy::Global {
        definition.alias = None;
    } else if definition.alias.is_none() {
        definition.alias = world
            .asset_definition_alias_bindings()
            .get(&definition.id)
            .map(|binding| binding.alias.clone());
    }

    Some(definition)
}

fn account_alias_permission_scope_dataspace_target_with_state(
    scope: &AccountAliasPermissionScope,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    match scope {
        AccountAliasPermissionScope::Domain(domain_id) => {
            domain_dataspace_target_with_state(domain_id, dataspace_catalog, state_view)
        }
        AccountAliasPermissionScope::Dataspace(dataspace_id) => Some(*dataspace_id),
    }
}

fn account_alias_permission_scope_dataspace_target_with_world<W: WorldReadOnly>(
    scope: &AccountAliasPermissionScope,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    match scope {
        AccountAliasPermissionScope::Domain(domain_id) => {
            domain_dataspace_target_with_world(domain_id, dataspace_catalog, world, ledger_time_ms)
        }
        AccountAliasPermissionScope::Dataspace(dataspace_id) => Some(*dataspace_id),
    }
}

fn dataspace_scoped_permission_target_needs_state(permission: &Permission) -> bool {
    match permission.name() {
        "CanMintAssetWithDefinition" => permission
            .payload()
            .try_into_any_norito::<CanMintAssetWithDefinition>()
            .ok()
            .is_some(),
        "CanBurnAssetWithDefinition" => permission
            .payload()
            .try_into_any_norito::<CanBurnAssetWithDefinition>()
            .ok()
            .is_some(),
        "CanTransferAssetWithDefinition" => permission
            .payload()
            .try_into_any_norito::<CanTransferAssetWithDefinition>()
            .ok()
            .is_some(),
        "CanModifyAssetMetadataWithDefinition" => permission
            .payload()
            .try_into_any_norito::<CanModifyAssetMetadataWithDefinition>()
            .ok()
            .is_some(),
        "CanUnregisterAssetDefinition" => permission
            .payload()
            .try_into_any_norito::<CanUnregisterAssetDefinition>()
            .ok()
            .is_some(),
        "CanModifyAssetDefinitionMetadata" => permission
            .payload()
            .try_into_any_norito::<CanModifyAssetDefinitionMetadata>()
            .ok()
            .is_some(),
        _ => false,
    }
}

fn dataspace_scoped_permission_target(
    permission: &Permission,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    if permission.name() != "CanPublishSpaceDirectoryManifest" {
        return match permission.name() {
            "CanMintAssetWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanMintAssetWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        state_view,
                    )
                }),
            "CanBurnAssetWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanBurnAssetWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        state_view,
                    )
                }),
            "CanTransferAssetWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanTransferAssetWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        state_view,
                    )
                }),
            "CanModifyAssetMetadataWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanModifyAssetMetadataWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        state_view,
                    )
                }),
            "CanUnregisterAssetDefinition" => permission
                .payload()
                .try_into_any_norito::<CanUnregisterAssetDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        state_view,
                    )
                }),
            "CanModifyAssetDefinitionMetadata" => permission
                .payload()
                .try_into_any_norito::<CanModifyAssetDefinitionMetadata>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        state_view,
                    )
                }),
            "CanManageAccountAlias" => permission
                .payload()
                .try_into_any_norito::<CanManageAccountAlias>()
                .ok()
                .and_then(|token| {
                    account_alias_permission_scope_dataspace_target_with_state(
                        &token.scope,
                        dataspace_catalog,
                        state_view,
                    )
                }),
            "CanResolveAccountAlias" => permission
                .payload()
                .try_into_any_norito::<CanResolveAccountAlias>()
                .ok()
                .and_then(|token| {
                    account_alias_permission_scope_dataspace_target_with_state(
                        &token.scope,
                        dataspace_catalog,
                        state_view,
                    )
                }),
            _ => None,
        };
    }

    permission
        .payload()
        .try_into_any_norito::<CanPublishSpaceDirectoryManifest>()
        .ok()
        .map(|token| token.dataspace)
}

fn dataspace_scoped_permission_target_with_world<W: WorldReadOnly>(
    permission: &Permission,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    if permission.name() != "CanPublishSpaceDirectoryManifest" {
        return match permission.name() {
            "CanMintAssetWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanMintAssetWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }),
            "CanBurnAssetWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanBurnAssetWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }),
            "CanTransferAssetWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanTransferAssetWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }),
            "CanModifyAssetMetadataWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanModifyAssetMetadataWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }),
            "CanUnregisterAssetDefinition" => permission
                .payload()
                .try_into_any_norito::<CanUnregisterAssetDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }),
            "CanModifyAssetDefinitionMetadata" => permission
                .payload()
                .try_into_any_norito::<CanModifyAssetDefinitionMetadata>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        None,
                        None,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }),
            "CanManageAccountAlias" => permission
                .payload()
                .try_into_any_norito::<CanManageAccountAlias>()
                .ok()
                .and_then(|token| {
                    account_alias_permission_scope_dataspace_target_with_world(
                        &token.scope,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }),
            "CanResolveAccountAlias" => permission
                .payload()
                .try_into_any_norito::<CanResolveAccountAlias>()
                .ok()
                .and_then(|token| {
                    account_alias_permission_scope_dataspace_target_with_world(
                        &token.scope,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }),
            _ => None,
        };
    }

    permission
        .payload()
        .try_into_any_norito::<CanPublishSpaceDirectoryManifest>()
        .ok()
        .map(|token| token.dataspace)
}

fn instruction_dataspace_scoped_permission_target_needs_state(
    instruction: &dyn Instruction,
) -> bool {
    let any = instruction.as_any();

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        return match grant {
            GrantBox::Permission(grant) => {
                dataspace_scoped_permission_target_needs_state(&grant.object)
            }
            GrantBox::Role(_) | GrantBox::RolePermission(_) => false,
        };
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return match revoke {
            RevokeBox::Permission(revoke) => {
                dataspace_scoped_permission_target_needs_state(&revoke.object)
            }
            RevokeBox::Role(_) | RevokeBox::RolePermission(_) => false,
        };
    }

    false
}

fn dataspace_scoped_permission_routing_requires_state(tx: &AcceptedTransaction<'_>) -> bool {
    let Some(executable) = transaction_executable(tx) else {
        return false;
    };

    match executable {
        Executable::Instructions(instructions) => instructions.iter().any(|instruction| {
            instruction_dataspace_scoped_permission_target_needs_state(&**instruction)
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) => false,
        Executable::IvmProved(proved) => proved.overlay.iter().any(|instruction| {
            instruction_dataspace_scoped_permission_target_needs_state(&**instruction)
        }),
    }
}

fn canonical_dataspace_route(
    dataspace_id: DataSpaceId,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
) -> Result<RoutingDecision, RoutingResolveError> {
    let lane_id = lane_catalog
        .lanes()
        .iter()
        .filter(|lane| lane.dataspace_id == dataspace_id)
        .map(|lane| lane.id)
        .min()
        .or_else(|| legacy_single_lane_for_dataspace(dataspace_id, lane_catalog))
        .ok_or(RoutingResolveError::NoLaneForDataspace { dataspace_id })?;

    resolve_routing_decision(
        RoutingDecision::new(lane_id, dataspace_id),
        lane_catalog,
        dataspace_catalog,
    )
}

fn routing_plan_digest(routes: &[RoutingDecision]) -> Hash {
    let mut bytes = Vec::with_capacity(16 + routes.len() * 12);
    bytes.extend_from_slice(b"iroha:routing-plan:v1");
    for route in routes {
        bytes.extend_from_slice(&route.lane_id.as_u32().to_le_bytes());
        bytes.extend_from_slice(&route.dataspace_id.as_u64().to_le_bytes());
    }
    Hash::new(bytes)
}

fn native_amx_plan_digest(coordinator: RoutingDecision, participants: &[RouteLeg]) -> Hash {
    let mut bytes = Vec::with_capacity(24 + participants.len() * 13);
    bytes.extend_from_slice(b"iroha:native-amx-plan:v1");
    bytes.push(0);
    bytes.extend_from_slice(&coordinator.lane_id.as_u32().to_le_bytes());
    bytes.extend_from_slice(&coordinator.dataspace_id.as_u64().to_le_bytes());
    for participant in participants {
        bytes.push(1);
        bytes.extend_from_slice(&participant.route.lane_id.as_u32().to_le_bytes());
        bytes.extend_from_slice(&participant.route.dataspace_id.as_u64().to_le_bytes());
    }
    Hash::new(bytes)
}

fn resolve_policy_routing_plan(
    policy: &LaneRoutingPolicy,
    matched_rule: Option<&LaneRoutingRule>,
    mut target: TransactionDataspaceTarget,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
) -> Result<RoutingPlan, RoutingResolveError> {
    add_smart_contract_deploy_policy_participant(&mut target, matched_rule);

    if !target.participants.is_empty()
        && (target.participants.len() > 1 || target.coordinator_route)
    {
        let coordinator_dataspace = if target.coordinator_route {
            DataSpaceId::UNIVERSAL
        } else {
            *target
                .participants
                .iter()
                .next()
                .expect("non-empty participants has first dataspace")
        };
        let coordinator_route =
            canonical_dataspace_route(coordinator_dataspace, lane_catalog, dataspace_catalog)?;
        let participants = target
            .participants
            .iter()
            .copied()
            .map(|dataspace_id| {
                canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog)
                    .map(|route| RouteLeg::new(route, RouteLegRole::Participant))
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(RoutingPlan::native_amx(coordinator_route, participants));
    }

    let decision = resolve_policy_routing_decision(
        policy,
        matched_rule,
        target.dataspace_id,
        target.coordinator_route,
        lane_catalog,
        dataspace_catalog,
    )?;
    Ok(RoutingPlan::single(decision))
}

fn add_smart_contract_deploy_policy_participant(
    target: &mut TransactionDataspaceTarget,
    matched_rule: Option<&LaneRoutingRule>,
) {
    let Some(rule) = matched_rule else {
        return;
    };
    let Some(rule_dataspace) = rule.dataspace else {
        return;
    };
    if rule_dataspace == DataSpaceId::UNIVERSAL || !rule_matches_smart_contract_deploy(rule) {
        return;
    }
    if target.participants.is_empty()
        || target
            .participants
            .iter()
            .all(|participant| *participant == rule_dataspace)
    {
        return;
    }

    target.participants.insert(rule_dataspace);
    target.dataspace_id = Some(DataSpaceId::UNIVERSAL);
}

fn rule_matches_smart_contract_deploy(rule: &LaneRoutingRule) -> bool {
    rule.matcher.instruction.as_deref().is_some_and(|matcher| {
        let matcher = matcher.trim();
        matches_label(matcher, "smartcontract::deploy")
            || matches_label(matcher, "smart_contract::deploy")
    })
}

fn resolve_policy_routing_decision(
    policy: &LaneRoutingPolicy,
    matched_rule: Option<&LaneRoutingRule>,
    target_dataspace: Option<DataSpaceId>,
    target_is_coordinator_route: bool,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
) -> Result<RoutingDecision, RoutingResolveError> {
    if target_is_coordinator_route && target_dataspace == Some(DataSpaceId::UNIVERSAL) {
        return canonical_dataspace_route(DataSpaceId::UNIVERSAL, lane_catalog, dataspace_catalog);
    }

    if let Some(dataspace_id) = target_dataspace {
        if let Some(rule) = matched_rule {
            let decision = RoutingDecision::new(rule.lane, dataspace_id);
            if !lane_catalog.lanes().iter().any(|lane| lane.id == rule.lane) {
                return resolve_routing_decision(decision, lane_catalog, dataspace_catalog);
            }
            if let Some(rule_dataspace) = rule.dataspace
                && rule_dataspace != dataspace_id
            {
                return Err(RoutingResolveError::LaneDataspaceMismatch {
                    lane_id: rule.lane,
                    lane_dataspace_id: rule_dataspace,
                    dataspace_id,
                });
            }
            return resolve_routing_decision(decision, lane_catalog, dataspace_catalog);
        }
        return canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog);
    }

    if let Some(rule) = matched_rule {
        let decision = RoutingDecision::new(
            rule.lane,
            rule.dataspace.unwrap_or(policy.default_dataspace),
        );
        return resolve_routing_decision(decision, lane_catalog, dataspace_catalog);
    }

    resolve_routing_decision(
        RoutingDecision::new(policy.default_lane, policy.default_dataspace),
        lane_catalog,
        dataspace_catalog,
    )
}

fn evaluate_query_policy_with_view(
    policy: &LaneRoutingPolicy,
    authority: &AccountId,
    state_view: Option<&StateView<'_>>,
) -> RoutingDecision {
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| query_rule_matches(rule, authority, state_view));
    let lane_id = matched_rule.map_or(policy.default_lane, |rule| rule.lane);
    let dataspace_id = matched_rule
        .and_then(|rule| rule.dataspace)
        .unwrap_or(policy.default_dataspace);
    RoutingDecision::new(lane_id, dataspace_id)
}

/// Resolve the configured routing policy for a signed query authority.
///
/// Query routing intentionally ignores transaction-instruction matchers because
/// queries do not carry ISI batches. Rules without an instruction matcher still
/// participate, including account-scoped rules and explicit catch-all rules.
pub fn resolve_query_routing_decision(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    authority: &AccountId,
    state_view: Option<&StateView<'_>>,
) -> Result<RoutingDecision, RoutingResolveError> {
    if let Some(state_view) = state_view {
        let matched_rule = policy
            .rules
            .iter()
            .find(|rule| query_rule_matches(rule, authority, Some(state_view)));
        let target_dataspace = account_dataspace_target(Some(state_view.world()), authority);
        return resolve_policy_routing_decision(
            policy,
            matched_rule,
            target_dataspace,
            target_dataspace == Some(DataSpaceId::UNIVERSAL),
            lane_catalog,
            dataspace_catalog,
        );
    }
    let decision = evaluate_query_policy_with_view(policy, authority, state_view);
    resolve_routing_decision(decision, lane_catalog, dataspace_catalog)
}

fn resolve_query_routing_decision_with_world<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    authority: &AccountId,
    world: &W,
    _ledger_time_ms: Option<u64>,
) -> Result<RoutingDecision, RoutingResolveError> {
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| query_rule_matches_with_world(rule, authority, dataspace_catalog, world));
    let target_dataspace = account_dataspace_target(Some(world), authority);
    resolve_policy_routing_decision(
        policy,
        matched_rule,
        target_dataspace,
        target_dataspace == Some(DataSpaceId::UNIVERSAL),
        lane_catalog,
        dataspace_catalog,
    )
}

/// Resolve a policy decision against lane/dataspace catalogs without fallback.
///
/// This function intentionally rejects unresolved or ambiguous combinations
/// instead of silently rewriting them to defaults.
pub fn resolve_routing_decision(
    decision: RoutingDecision,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
) -> Result<RoutingDecision, RoutingResolveError> {
    let Some(lane) = lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == decision.lane_id)
    else {
        return Err(RoutingResolveError::UnknownLane {
            lane_id: decision.lane_id,
        });
    };

    let dataspace_known = dataspace_catalog
        .entries()
        .iter()
        .any(|entry| entry.id == decision.dataspace_id);
    let default_public_lane_for_dynamic_dataspace = decision.dataspace_id != DataSpaceId::UNIVERSAL
        && lane.id == LaneId::SINGLE
        && lane.dataspace_id == DataSpaceId::UNIVERSAL;
    if !dataspace_known && !default_public_lane_for_dynamic_dataspace {
        return Err(RoutingResolveError::UnknownDataspace {
            dataspace_id: decision.dataspace_id,
        });
    }

    if lane.dataspace_id != decision.dataspace_id {
        if lane.id == LaneId::SINGLE
            && lane.dataspace_id == DataSpaceId::UNIVERSAL
            && legacy_single_lane_for_dataspace(decision.dataspace_id, lane_catalog).is_some()
        {
            return Ok(decision);
        }
        return Err(RoutingResolveError::LaneDataspaceMismatch {
            lane_id: lane.id,
            lane_dataspace_id: lane.dataspace_id,
            dataspace_id: decision.dataspace_id,
        });
    }

    Ok(decision)
}

fn legacy_single_lane_for_dataspace(
    dataspace_id: DataSpaceId,
    lane_catalog: &LaneCatalog,
) -> Option<LaneId> {
    if dataspace_id == DataSpaceId::UNIVERSAL {
        return None;
    }
    let has_dataspace_lane = lane_catalog
        .lanes()
        .iter()
        .any(|lane| lane.dataspace_id == dataspace_id);
    if has_dataspace_lane {
        return None;
    }
    lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == LaneId::SINGLE && lane.dataspace_id == DataSpaceId::UNIVERSAL)
        .map(|lane| lane.id)
}

fn rule_matches(
    rule: &LaneRoutingRule,
    tx: &AcceptedTransaction<'_>,
    state_view: Option<&StateView<'_>>,
) -> bool {
    let matcher = &rule.matcher;

    if let Some(account) = matcher.account.as_deref()
        && !tx
            .authority_opt()
            .is_some_and(|authority| account_matches(account, authority, state_view))
    {
        return false;
    }

    if let Some(instruction) = matcher.instruction.as_deref()
        && !instructions_match(instruction, tx, state_view)
    {
        return false;
    }

    true
}

fn rule_matches_with_world<W: WorldReadOnly>(
    rule: &LaneRoutingRule,
    tx: &AcceptedTransaction<'_>,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    _ledger_time_ms: Option<u64>,
) -> bool {
    let matcher = &rule.matcher;

    if let Some(account) = matcher.account.as_deref()
        && !tx.authority_opt().is_some_and(|authority| {
            account_matches_with_world(account, authority, dataspace_catalog, world)
        })
    {
        return false;
    }

    if let Some(instruction) = matcher.instruction.as_deref()
        && !instructions_match_with_world(instruction, tx, dataspace_catalog, world)
    {
        return false;
    }

    true
}

fn query_rule_matches(
    rule: &LaneRoutingRule,
    authority: &AccountId,
    state_view: Option<&StateView<'_>>,
) -> bool {
    if rule.matcher.instruction.is_some() {
        return false;
    }

    rule.matcher.account.as_deref().map_or(true, |account| {
        account_matches(account, authority, state_view)
    })
}

fn query_rule_matches_with_world<W: WorldReadOnly>(
    rule: &LaneRoutingRule,
    authority: &AccountId,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
) -> bool {
    if rule.matcher.instruction.is_some() {
        return false;
    }

    rule.matcher.account.as_deref().map_or(true, |account| {
        account_matches_with_world(account, authority, dataspace_catalog, world)
    })
}

fn account_matches_literal_or_encoded(pattern: &str, authority: &AccountId) -> bool {
    if authority.to_string() == pattern {
        return true;
    }
    iroha_data_model::account::AccountId::parse_encoded(pattern)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .is_ok_and(|parsed| parsed == *authority)
}

fn account_matches(
    pattern: &str,
    authority: &iroha_data_model::account::AccountId,
    state_view: Option<&StateView<'_>>,
) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }

    if account_matches_literal_or_encoded(pattern, authority) {
        return true;
    }

    let Some(state_view) = state_view else {
        return false;
    };

    account_matches_with_world(
        pattern,
        authority,
        &state_view.nexus().dataspace_catalog,
        state_view.world(),
    )
}

fn account_matches_with_world<W: WorldReadOnly>(
    pattern: &str,
    authority: &AccountId,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }

    if account_matches_literal_or_encoded(pattern, authority) {
        return true;
    }

    if let Some(scope) = pattern.strip_prefix("*@") {
        return account_matches_alias_scope_with_world(scope, authority, dataspace_catalog, world);
    }

    AccountAlias::from_literal(pattern, dataspace_catalog)
        .ok()
        .is_some_and(|alias| {
            world
                .bound_account_aliases(authority)
                .into_iter()
                .any(|bound| bound == alias)
        })
}

fn account_matches_alias_scope(
    scope: &str,
    account_id: &AccountId,
    state_view: &StateView<'_>,
) -> bool {
    account_matches_alias_scope_with_world(
        scope,
        account_id,
        &state_view.nexus().dataspace_catalog,
        state_view.world(),
    )
}

fn account_matches_alias_scope_with_world<W: WorldReadOnly>(
    scope: &str,
    account_id: &AccountId,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
) -> bool {
    let scope = scope.trim().to_ascii_lowercase();
    if scope.is_empty() {
        return false;
    }

    if world
        .account_scope_hierarchy(account_id)
        .ok()
        .is_some_and(|hierarchy| {
            hierarchy.into_iter().any(|(dataspace_id, domains)| {
                dataspace_catalog
                    .by_id(dataspace_id)
                    .is_some_and(|entry| entry.alias.eq_ignore_ascii_case(scope.as_str()))
                    || domains
                        .into_iter()
                        .any(|domain| domain.to_string().eq_ignore_ascii_case(scope.as_str()))
            })
        })
    {
        return true;
    }

    world
        .bound_account_aliases(account_id)
        .into_iter()
        .any(|alias| {
            alias
                .to_literal(dataspace_catalog)
                .ok()
                .and_then(|literal| {
                    literal
                        .rsplit_once('@')
                        .map(|(_, alias_scope)| alias_scope == scope.as_str())
                })
                .unwrap_or(false)
        })
}

fn instructions_match(
    matcher: &str,
    tx: &AcceptedTransaction<'_>,
    state_view: Option<&StateView<'_>>,
) -> bool {
    let matcher_norm = matcher.trim().to_ascii_lowercase();
    if matcher_norm.is_empty() {
        return false;
    }
    let (matcher_label, destination_scope) = split_instruction_matcher(&matcher_norm);
    if matcher_label.is_empty() {
        return false;
    }

    match tx.entrypoint() {
        iroha_data_model::transaction::TransactionEntrypoint::External(signed) => {
            let executable = signed.instructions();
            let Executable::Instructions(batch) = executable else {
                return false;
            };

            batch.iter().any(|instruction| {
                instruction_matches(matcher_label, destination_scope, &**instruction, state_view)
            })
        }
        iroha_data_model::transaction::TransactionEntrypoint::SealedCommitment(_) => false,
        iroha_data_model::transaction::TransactionEntrypoint::SealedReveal(reveal) => {
            let executable = reveal.signed_transaction().instructions();
            let Executable::Instructions(batch) = executable else {
                return false;
            };

            batch.iter().any(|instruction| {
                instruction_matches(matcher_label, destination_scope, &**instruction, state_view)
            })
        }
        iroha_data_model::transaction::TransactionEntrypoint::PrivateKaigi(private) => {
            crate::smartcontracts::isi::kaigi::private_instruction_box(private)
                .map(|instruction| {
                    instruction_matches(matcher_label, destination_scope, &*instruction, state_view)
                })
                .unwrap_or(false)
        }
        iroha_data_model::transaction::TransactionEntrypoint::Time(_) => false,
    }
}

fn instructions_match_with_world<W: WorldReadOnly>(
    matcher: &str,
    tx: &AcceptedTransaction<'_>,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
) -> bool {
    let matcher_norm = matcher.trim().to_ascii_lowercase();
    if matcher_norm.is_empty() {
        return false;
    }
    let (matcher_label, destination_scope) = split_instruction_matcher(&matcher_norm);
    if matcher_label.is_empty() {
        return false;
    }

    match tx.entrypoint() {
        iroha_data_model::transaction::TransactionEntrypoint::External(signed) => {
            let executable = signed.instructions();
            let Executable::Instructions(batch) = executable else {
                return false;
            };

            batch.iter().any(|instruction| {
                instruction_matches_with_world(
                    matcher_label,
                    destination_scope,
                    &**instruction,
                    dataspace_catalog,
                    world,
                )
            })
        }
        iroha_data_model::transaction::TransactionEntrypoint::SealedCommitment(_) => false,
        iroha_data_model::transaction::TransactionEntrypoint::SealedReveal(reveal) => {
            let executable = reveal.signed_transaction().instructions();
            let Executable::Instructions(batch) = executable else {
                return false;
            };

            batch.iter().any(|instruction| {
                instruction_matches_with_world(
                    matcher_label,
                    destination_scope,
                    &**instruction,
                    dataspace_catalog,
                    world,
                )
            })
        }
        iroha_data_model::transaction::TransactionEntrypoint::PrivateKaigi(private) => {
            crate::smartcontracts::isi::kaigi::private_instruction_box(private)
                .map(|instruction| {
                    instruction_matches_with_world(
                        matcher_label,
                        destination_scope,
                        &*instruction,
                        dataspace_catalog,
                        world,
                    )
                })
                .unwrap_or(false)
        }
        iroha_data_model::transaction::TransactionEntrypoint::Time(_) => false,
    }
}

fn split_instruction_matcher(matcher: &str) -> (&str, Option<&str>) {
    if let Some((label, domain)) = matcher.rsplit_once('@')
        && label.starts_with("transfer")
    {
        let label = label.trim();
        let domain = domain.trim();
        if !label.is_empty() && !domain.is_empty() {
            return (label, Some(domain));
        }
    }

    (matcher, None)
}

fn instruction_matches(
    matcher: &str,
    destination_scope: Option<&str>,
    instruction: &dyn Instruction,
    state_view: Option<&StateView<'_>>,
) -> bool {
    if destination_scope.is_some_and(|scope| {
        !transfer_destination_matches_alias_scope(instruction, scope, state_view)
    }) {
        return false;
    }

    if instruction_label_matches(matcher, instruction) {
        return true;
    }

    let id = Instruction::id(instruction).to_ascii_lowercase();
    if matches_label(matcher, &id) {
        return true;
    }

    id.split("::").any(|segment| {
        matches_label(matcher, segment)
            || segment
                .strip_suffix("box")
                .is_some_and(|trimmed| !trimmed.is_empty() && matches_label(matcher, trimmed))
    })
}

fn instruction_matches_with_world<W: WorldReadOnly>(
    matcher: &str,
    destination_scope: Option<&str>,
    instruction: &dyn Instruction,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
) -> bool {
    if destination_scope.is_some_and(|scope| {
        !transfer_destination_matches_alias_scope_with_world(
            instruction,
            scope,
            dataspace_catalog,
            world,
        )
    }) {
        return false;
    }

    if instruction_label_matches(matcher, instruction) {
        return true;
    }

    let id = Instruction::id(instruction).to_ascii_lowercase();
    if matches_label(matcher, &id) {
        return true;
    }

    id.split("::").any(|segment| {
        matches_label(matcher, segment)
            || segment
                .strip_suffix("box")
                .is_some_and(|trimmed| !trimmed.is_empty() && matches_label(matcher, trimmed))
    })
}

fn transfer_destination_matches_alias_scope(
    instruction: &dyn Instruction,
    scope: &str,
    state_view: Option<&StateView<'_>>,
) -> bool {
    let scope = scope.trim();
    if scope.is_empty() {
        return false;
    }

    let any = instruction.as_any();
    let Some(transfer) = any.downcast_ref::<TransferBox>() else {
        return false;
    };

    let destination = match transfer {
        TransferBox::Domain(transfer) => {
            return domain_scope_matches(scope, &transfer.object);
        }
        TransferBox::AssetDefinition(transfer) => {
            return asset_definition_scope_matches(scope, &transfer.object, state_view);
        }
        TransferBox::Asset(transfer) => &transfer.destination,
        TransferBox::Nft(transfer) => &transfer.destination,
    };
    let Some(state_view) = state_view else {
        return false;
    };
    account_matches_alias_scope(scope, destination, state_view)
}

fn transfer_destination_matches_alias_scope_with_world<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    scope: &str,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
) -> bool {
    let scope = scope.trim();
    if scope.is_empty() {
        return false;
    }

    let any = instruction.as_any();
    let Some(transfer) = any.downcast_ref::<TransferBox>() else {
        return false;
    };

    let destination = match transfer {
        TransferBox::Domain(transfer) => {
            return domain_scope_matches(scope, &transfer.object);
        }
        TransferBox::AssetDefinition(transfer) => {
            return asset_definition_scope_matches_with_world(scope, &transfer.object, world);
        }
        TransferBox::Asset(transfer) => &transfer.destination,
        TransferBox::Nft(transfer) => &transfer.destination,
    };
    account_matches_alias_scope_with_world(scope, destination, dataspace_catalog, world)
}

fn domain_scope_matches(scope: &str, domain_id: &DomainId) -> bool {
    scope.eq_ignore_ascii_case(domain_id.to_string().as_str())
        || scope.eq_ignore_ascii_case(domain_id.dataspace().as_ref())
}

fn asset_definition_scope_matches(
    scope: &str,
    asset_definition_id: &AssetDefinitionId,
    state_view: Option<&StateView<'_>>,
) -> bool {
    asset_definition_id
        .try_domain()
        .cloned()
        .or_else(|| {
            state_view.and_then(|view| {
                view.world
                    .asset_definition(asset_definition_id)
                    .ok()
                    .and_then(|definition| definition.id.try_domain().cloned())
            })
        })
        .is_some_and(|domain_id| domain_scope_matches(scope, &domain_id))
}

fn asset_definition_scope_matches_with_world<W: WorldReadOnly>(
    scope: &str,
    asset_definition_id: &AssetDefinitionId,
    world: &W,
) -> bool {
    asset_definition_id
        .try_domain()
        .cloned()
        .or_else(|| {
            world
                .asset_definition(asset_definition_id)
                .ok()
                .and_then(|definition| definition.id.try_domain().cloned())
        })
        .is_some_and(|domain_id| domain_scope_matches(scope, &domain_id))
}

fn instruction_label_matches(matcher: &str, instruction: &dyn Instruction) -> bool {
    let any = instruction.as_any();

    if let Some(register) = any.downcast_ref::<RegisterBox>() {
        let variant = match register {
            RegisterBox::Peer(_) => "register::peer",
            RegisterBox::Domain(_) => "register::domain",
            RegisterBox::Account(_) => "register::account",
            RegisterBox::AssetDefinition(_) => "register::asset_definition",
            RegisterBox::Nft(_) => "register::nft",
            RegisterBox::Role(_) => "register::role",
            RegisterBox::Trigger(_) => "register::trigger",
        };
        return matches_box_variant(matcher, "register", variant);
    }

    if let Some(unregister) = any.downcast_ref::<UnregisterBox>() {
        let variant = match unregister {
            UnregisterBox::Peer(_) => "unregister::peer",
            UnregisterBox::Domain(_) => "unregister::domain",
            UnregisterBox::Account(_) => "unregister::account",
            UnregisterBox::AssetDefinition(_) => "unregister::asset_definition",
            UnregisterBox::Nft(_) => "unregister::nft",
            UnregisterBox::Role(_) => "unregister::role",
            UnregisterBox::Trigger(_) => "unregister::trigger",
        };
        return matches_box_variant(matcher, "unregister", variant);
    }

    if let Some(mint) = any.downcast_ref::<MintBox>() {
        let variant = match mint {
            MintBox::Asset(_) => "mint::asset",
            MintBox::TriggerRepetitions(_) => "mint::trigger_repetitions",
        };
        return matches_box_variant(matcher, "mint", variant);
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>() {
        let variant = match burn {
            BurnBox::Asset(_) => "burn::asset",
            BurnBox::TriggerRepetitions(_) => "burn::trigger_repetitions",
        };
        return matches_box_variant(matcher, "burn", variant);
    }

    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        let variant = match transfer {
            TransferBox::Domain(_) => "transfer::domain",
            TransferBox::AssetDefinition(_) => "transfer::asset_definition",
            TransferBox::Asset(_) => "transfer::asset",
            TransferBox::Nft(_) => "transfer::nft",
        };
        return matches_box_variant(matcher, "transfer", variant);
    }

    if let Some(set_key_value) = any.downcast_ref::<SetKeyValueBox>() {
        let variant = match set_key_value {
            SetKeyValueBox::Domain(_) => "set_key_value::domain",
            SetKeyValueBox::Account(_) => "set_key_value::account",
            SetKeyValueBox::AssetDefinition(_) => "set_key_value::asset_definition",
            SetKeyValueBox::Nft(_) => "set_key_value::nft",
            SetKeyValueBox::Trigger(_) => "set_key_value::trigger",
        };
        return matches_box_variant(matcher, "set_key_value", variant);
    }

    if let Some(remove_key_value) = any.downcast_ref::<RemoveKeyValueBox>() {
        let variant = match remove_key_value {
            RemoveKeyValueBox::Domain(_) => "remove_key_value::domain",
            RemoveKeyValueBox::Account(_) => "remove_key_value::account",
            RemoveKeyValueBox::AssetDefinition(_) => "remove_key_value::asset_definition",
            RemoveKeyValueBox::Nft(_) => "remove_key_value::nft",
            RemoveKeyValueBox::Trigger(_) => "remove_key_value::trigger",
        };
        return matches_box_variant(matcher, "remove_key_value", variant);
    }

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        let variant = match grant {
            GrantBox::Permission(_) => "grant::permission",
            GrantBox::Role(_) => "grant::role",
            GrantBox::RolePermission(_) => "grant::role_permission",
        };
        return matches_box_variant(matcher, "grant", variant);
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        let variant = match revoke {
            RevokeBox::Permission(_) => "revoke::permission",
            RevokeBox::Role(_) => "revoke::role",
            RevokeBox::RolePermission(_) => "revoke::role_permission",
        };
        return matches_box_variant(matcher, "revoke", variant);
    }

    if any.is::<RegisterSmartContractCode>() || any.is::<RegisterSmartContractBytes>() {
        return matches_label(matcher, "smartcontract::deploy")
            || matches_label(matcher, "smart_contract::deploy");
    }

    false
}

fn matches_box_variant(matcher: &str, base: &str, variant: &str) -> bool {
    matches_label(matcher, base) || matches_label(matcher, variant)
}

fn matches_label(matcher: &str, label: &str) -> bool {
    label == matcher || eq_ignoring_underscores(label, matcher)
}

fn eq_ignoring_underscores(left: &str, right: &str) -> bool {
    let mut left_iter = left.bytes().filter(|byte| *byte != b'_');
    let mut right_iter = right.bytes().filter(|byte| *byte != b'_');
    loop {
        match (left_iter.next(), right_iter.next()) {
            (None, None) => return true,
            (Some(left_byte), Some(right_byte)) if left_byte == right_byte => {}
            _ => return false,
        }
    }
}

/// Strategy object that derives lane/dataspace assignments for queued transactions.
pub trait LaneRouter: Send + Sync + 'static {
    /// Route the given transaction without requiring a state snapshot.
    fn route(&self, tx: &AcceptedTransaction<'_>) -> RoutingDecision;

    /// Route the given transaction using an already acquired state view.
    ///
    /// Routers that require dynamic world-state can override this method and
    /// [`LaneRouter::route_without_state`].
    fn route_with_view(
        &self,
        tx: &AcceptedTransaction<'_>,
        _state_view: &StateView<'_>,
    ) -> RoutingDecision {
        self.route(tx)
    }

    /// Route the given transaction with narrow state access when possible.
    ///
    /// The default implementation prefers [`LaneRouter::route_without_state`]
    /// and only falls back to taking a short-lived [`StateView`] when needed.
    fn route_with_state(&self, tx: &AcceptedTransaction<'_>, state: &State) -> RoutingDecision {
        if let Some(decision) = self.route_without_state(tx) {
            return decision;
        }
        let state_view = state.view();
        self.route_with_view(tx, &state_view)
    }

    /// Route the given transaction without a state snapshot when possible.
    ///
    /// Routers that do not depend on dynamic world-state can override this to
    /// avoid taking a full [`StateView`] in hot requeue paths.
    fn route_without_state(&self, tx: &AcceptedTransaction<'_>) -> Option<RoutingDecision> {
        Some(self.route(tx))
    }

    /// Route the given transaction and return deterministic route-resolution errors.
    fn try_route(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        Ok(self.route(tx))
    }

    /// Route with an existing state view and return deterministic route-resolution errors.
    fn try_route_with_view(
        &self,
        tx: &AcceptedTransaction<'_>,
        state_view: &StateView<'_>,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        Ok(self.route_with_view(tx, state_view))
    }

    /// Route with narrow state access and return deterministic route-resolution errors.
    fn try_route_with_state(
        &self,
        tx: &AcceptedTransaction<'_>,
        state: &State,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        if let Some(decision) = self.try_route_without_state(tx)? {
            return Ok(decision);
        }
        let state_view = state.view();
        self.try_route_with_view(tx, &state_view)
    }

    /// Route without state snapshot when possible and return deterministic route errors.
    fn try_route_without_state(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<Option<RoutingDecision>, RoutingResolveError> {
        Ok(self.route_without_state(tx))
    }

    /// Build the full routing plan for a transaction and return deterministic errors.
    fn try_route_plan(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<RoutingPlan, RoutingResolveError> {
        self.try_route(tx)
            .map(|route| RoutingPlan::Single(RouteLeg::new(route, RouteLegRole::Coordinator)))
    }

    /// Build the full routing plan with an existing state view.
    fn try_route_plan_with_view(
        &self,
        tx: &AcceptedTransaction<'_>,
        state_view: &StateView<'_>,
    ) -> Result<RoutingPlan, RoutingResolveError> {
        self.try_route_with_view(tx, state_view)
            .map(|route| RoutingPlan::Single(RouteLeg::new(route, RouteLegRole::Coordinator)))
    }

    /// Build the full routing plan with narrow state access when possible.
    fn try_route_plan_with_state(
        &self,
        tx: &AcceptedTransaction<'_>,
        state: &State,
    ) -> Result<RoutingPlan, RoutingResolveError> {
        if let Some(plan) = self.try_route_plan_without_state(tx)? {
            return Ok(plan);
        }
        let state_view = state.view();
        self.try_route_plan_with_view(tx, &state_view)
    }

    /// Build the full routing plan without state when possible.
    fn try_route_plan_without_state(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<Option<RoutingPlan>, RoutingResolveError> {
        Ok(self
            .try_route_without_state(tx)?
            .map(|route| RoutingPlan::Single(RouteLeg::new(route, RouteLegRole::Coordinator))))
    }
}

/// Trivial router that keeps the single-lane/universal-dataspace behaviour.
#[derive(Copy, Clone, Debug, Default)]
pub struct SingleLaneRouter;

impl SingleLaneRouter {
    /// Create a router that always selects the default single lane/universal dataspace.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl LaneRouter for SingleLaneRouter {
    fn route(&self, _tx: &AcceptedTransaction<'_>) -> RoutingDecision {
        RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
    }
}

/// Router that applies the declarative policy derived from configuration.
#[derive(Debug, Clone)]
pub struct ConfigLaneRouter {
    policy: Arc<LaneRoutingPolicy>,
    dataspace_catalog: Arc<DataSpaceCatalog>,
    lane_catalog: Arc<LaneCatalog>,
}

impl ConfigLaneRouter {
    /// Build a router from the validated runtime configuration.
    #[must_use]
    pub fn new(
        policy: LaneRoutingPolicy,
        dataspace_catalog: DataSpaceCatalog,
        lane_catalog: LaneCatalog,
    ) -> Self {
        Self {
            policy: Arc::new(policy),
            dataspace_catalog: Arc::new(dataspace_catalog),
            lane_catalog: Arc::new(lane_catalog),
        }
    }

    fn catalog_only_routing_decision(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<Option<RoutingDecision>, RoutingResolveError> {
        if let Some(decision) = dataspace_scoped_permission_routing_decision(
            tx,
            Some(self.lane_catalog.as_ref()),
            Some(self.dataspace_catalog.as_ref()),
            None,
        )? {
            return Ok(Some(decision));
        }
        if let Some(decision) = settlement_routing_decision(
            tx,
            self.lane_catalog.as_ref(),
            self.dataspace_catalog.as_ref(),
            None,
        )? {
            return Ok(Some(decision));
        }
        Ok(None)
    }
}

impl LaneRouter for ConfigLaneRouter {
    fn route(&self, tx: &AcceptedTransaction<'_>) -> RoutingDecision {
        evaluate_policy(&self.policy, tx)
    }

    fn route_with_view(
        &self,
        tx: &AcceptedTransaction<'_>,
        state_view: &StateView<'_>,
    ) -> RoutingDecision {
        evaluate_policy_with_view(&state_view.nexus().routing_policy, tx, state_view)
    }

    fn route_without_state(&self, tx: &AcceptedTransaction<'_>) -> Option<RoutingDecision> {
        if dataspace_scoped_permission_routing_requires_state(tx)
            || transaction_target_routing_requires_state(tx)
        {
            return None;
        }
        if let Ok(Some(decision)) = self.catalog_only_routing_decision(tx) {
            return Some(decision);
        }
        if policy_needs_state(self.policy.as_ref()) {
            return None;
        }
        if self.authority_scope_routing_requires_state(tx).ok()? {
            return None;
        }
        Some(evaluate_policy_with_catalog_hint(
            &self.policy,
            self.lane_catalog.as_ref(),
            self.dataspace_catalog.as_ref(),
            tx,
        ))
    }

    fn try_route(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        if let Some(decision) = dataspace_scoped_permission_routing_decision(
            tx,
            Some(self.lane_catalog.as_ref()),
            Some(self.dataspace_catalog.as_ref()),
            None,
        )? {
            return Ok(decision);
        }
        if let Some(decision) = settlement_routing_decision(
            tx,
            self.lane_catalog.as_ref(),
            self.dataspace_catalog.as_ref(),
            None,
        )? {
            return Ok(decision);
        }
        if let Some(account_id) = account_permission_holder_routing_target(tx) {
            return resolve_query_routing_decision(
                &self.policy,
                self.lane_catalog.as_ref(),
                self.dataspace_catalog.as_ref(),
                account_id,
                None,
            );
        }
        let target = transaction_dataspace_routing_target_info(
            tx,
            Some(self.dataspace_catalog.as_ref()),
            None,
        )?;
        let matched_rule = self
            .policy
            .rules
            .iter()
            .find(|rule| rule_matches(rule, tx, None));
        resolve_policy_routing_decision(
            &self.policy,
            matched_rule,
            target.dataspace_id,
            target.coordinator_route,
            self.lane_catalog.as_ref(),
            self.dataspace_catalog.as_ref(),
        )
    }

    fn try_route_plan(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<RoutingPlan, RoutingResolveError> {
        if let Some(decision) = dataspace_scoped_permission_routing_decision(
            tx,
            Some(self.lane_catalog.as_ref()),
            Some(self.dataspace_catalog.as_ref()),
            None,
        )? {
            return Ok(RoutingPlan::Single(RouteLeg::new(
                decision,
                RouteLegRole::Coordinator,
            )));
        }
        if let Some(decision) = settlement_routing_decision(
            tx,
            self.lane_catalog.as_ref(),
            self.dataspace_catalog.as_ref(),
            None,
        )? {
            return Ok(RoutingPlan::Single(RouteLeg::new(
                decision,
                RouteLegRole::Coordinator,
            )));
        }
        if let Some(account_id) = account_permission_holder_routing_target(tx) {
            return resolve_query_routing_decision(
                &self.policy,
                self.lane_catalog.as_ref(),
                self.dataspace_catalog.as_ref(),
                account_id,
                None,
            )
            .map(|decision| {
                RoutingPlan::Single(RouteLeg::new(decision, RouteLegRole::Coordinator))
            });
        }
        let target = transaction_dataspace_routing_target_info(
            tx,
            Some(self.dataspace_catalog.as_ref()),
            None,
        )?;
        let matched_rule = self
            .policy
            .rules
            .iter()
            .find(|rule| rule_matches(rule, tx, None));
        resolve_policy_routing_plan(
            &self.policy,
            matched_rule,
            target,
            self.lane_catalog.as_ref(),
            self.dataspace_catalog.as_ref(),
        )
    }

    fn try_route_with_view(
        &self,
        tx: &AcceptedTransaction<'_>,
        state_view: &StateView<'_>,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        let nexus = state_view.nexus();
        if let Some(decision) = dataspace_scoped_permission_routing_decision(
            tx,
            Some(&nexus.lane_catalog),
            Some(&nexus.dataspace_catalog),
            Some(state_view),
        )? {
            return Ok(decision);
        }
        if let Some(decision) = settlement_routing_decision(
            tx,
            &nexus.lane_catalog,
            &nexus.dataspace_catalog,
            Some(state_view),
        )? {
            return Ok(decision);
        }
        if let Some(account_id) = account_permission_holder_routing_target(tx) {
            return resolve_query_routing_decision(
                &nexus.routing_policy,
                &nexus.lane_catalog,
                &nexus.dataspace_catalog,
                account_id,
                Some(state_view),
            );
        }
        let mut target = transaction_dataspace_routing_target_info(
            tx,
            Some(&nexus.dataspace_catalog),
            Some(state_view),
        )?;
        let matched_rule = nexus
            .routing_policy
            .rules
            .iter()
            .find(|rule| rule_matches(rule, tx, Some(state_view)));
        apply_authority_dataspace_target(
            &mut target,
            authority_dataspace_target(Some(state_view), tx),
            matched_rule.is_some_and(|rule| rule.matcher.account.is_some()),
        );
        resolve_policy_routing_decision(
            &nexus.routing_policy,
            matched_rule,
            target.dataspace_id,
            target.coordinator_route,
            &nexus.lane_catalog,
            &nexus.dataspace_catalog,
        )
    }

    fn try_route_plan_with_view(
        &self,
        tx: &AcceptedTransaction<'_>,
        state_view: &StateView<'_>,
    ) -> Result<RoutingPlan, RoutingResolveError> {
        let nexus = state_view.nexus();
        if let Some(decision) = dataspace_scoped_permission_routing_decision(
            tx,
            Some(&nexus.lane_catalog),
            Some(&nexus.dataspace_catalog),
            Some(state_view),
        )? {
            return Ok(RoutingPlan::Single(RouteLeg::new(
                decision,
                RouteLegRole::Coordinator,
            )));
        }
        if let Some(decision) = settlement_routing_decision(
            tx,
            &nexus.lane_catalog,
            &nexus.dataspace_catalog,
            Some(state_view),
        )? {
            return Ok(RoutingPlan::Single(RouteLeg::new(
                decision,
                RouteLegRole::Coordinator,
            )));
        }
        if let Some(account_id) = account_permission_holder_routing_target(tx) {
            return resolve_query_routing_decision(
                &nexus.routing_policy,
                &nexus.lane_catalog,
                &nexus.dataspace_catalog,
                account_id,
                Some(state_view),
            )
            .map(|decision| {
                RoutingPlan::Single(RouteLeg::new(decision, RouteLegRole::Coordinator))
            });
        }
        let mut target = transaction_dataspace_routing_target_info(
            tx,
            Some(&nexus.dataspace_catalog),
            Some(state_view),
        )?;
        let matched_rule = nexus
            .routing_policy
            .rules
            .iter()
            .find(|rule| rule_matches(rule, tx, Some(state_view)));
        apply_authority_dataspace_target(
            &mut target,
            authority_dataspace_target(Some(state_view), tx),
            matched_rule.is_some_and(|rule| rule.matcher.account.is_some()),
        );
        resolve_policy_routing_plan(
            &nexus.routing_policy,
            matched_rule,
            target,
            &nexus.lane_catalog,
            &nexus.dataspace_catalog,
        )
    }

    fn try_route_without_state(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<Option<RoutingDecision>, RoutingResolveError> {
        if dataspace_scoped_permission_routing_requires_state(tx)
            || transaction_target_routing_requires_state(tx)
        {
            return Ok(None);
        }
        if let Some(decision) = self.catalog_only_routing_decision(tx)? {
            return Ok(Some(decision));
        }
        if policy_needs_state(self.policy.as_ref()) {
            return Ok(None);
        }
        if self.policy.rules.is_empty() {
            let target = transaction_dataspace_routing_target(
                tx,
                Some(self.dataspace_catalog.as_ref()),
                None,
            )?;
            if target.is_none() {
                return Ok(None);
            }
        }
        if let Some(account_id) = account_permission_holder_routing_target(tx)
            && !self
                .policy
                .rules
                .iter()
                .any(|rule| query_rule_matches(rule, account_id, None))
        {
            return Ok(None);
        }
        if self.authority_scope_routing_requires_state(tx)? {
            return Ok(None);
        }
        self.try_route(tx).map(Some)
    }

    fn try_route_plan_without_state(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<Option<RoutingPlan>, RoutingResolveError> {
        if dataspace_scoped_permission_routing_requires_state(tx)
            || transaction_target_routing_requires_state(tx)
        {
            return Ok(None);
        }
        if let Some(decision) = self.catalog_only_routing_decision(tx)? {
            return Ok(Some(RoutingPlan::single(decision)));
        }
        if policy_needs_state(self.policy.as_ref()) {
            return Ok(None);
        }
        if self.policy.rules.is_empty() {
            let target = transaction_dataspace_routing_target(
                tx,
                Some(self.dataspace_catalog.as_ref()),
                None,
            )?;
            if target.is_none() {
                return Ok(None);
            }
        }
        if let Some(account_id) = account_permission_holder_routing_target(tx)
            && !self
                .policy
                .rules
                .iter()
                .any(|rule| query_rule_matches(rule, account_id, None))
        {
            return Ok(None);
        }
        if self.authority_scope_routing_requires_state(tx)? {
            return Ok(None);
        }
        self.try_route_plan(tx).map(Some)
    }
}

impl ConfigLaneRouter {
    fn authority_scope_routing_requires_state(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<bool, RoutingResolveError> {
        if tx.authority_opt().is_none() || account_permission_holder_routing_target(tx).is_some() {
            return Ok(false);
        }
        if dataspace_scoped_permission_routing_decision(
            tx,
            Some(self.lane_catalog.as_ref()),
            Some(self.dataspace_catalog.as_ref()),
            None,
        )?
        .is_some()
        {
            return Ok(false);
        }
        if settlement_routing_decision(
            tx,
            self.lane_catalog.as_ref(),
            self.dataspace_catalog.as_ref(),
            None,
        )?
        .is_some()
        {
            return Ok(false);
        }
        if transaction_dataspace_routing_target(tx, Some(self.dataspace_catalog.as_ref()), None)?
            .is_some()
        {
            return Ok(false);
        }
        let matched_rule = self
            .policy
            .rules
            .iter()
            .any(|rule| rule_matches(rule, tx, None));
        Ok(!matched_rule)
    }
}

fn policy_needs_state(policy: &LaneRoutingPolicy) -> bool {
    policy
        .rules
        .iter()
        .any(|rule| matcher_needs_state(&rule.matcher))
}

fn matcher_needs_state(matcher: &LaneRoutingMatcher) -> bool {
    let account_needs_state = matcher.account.as_deref().is_some_and(|account| {
        let account = account.trim();
        account.contains('@')
    });

    let instruction_needs_state = matcher.instruction.as_deref().is_some_and(|instruction| {
        let instruction = instruction.trim().to_ascii_lowercase();
        instruction.starts_with("transfer") && instruction.rsplit_once('@').is_some()
    });

    account_needs_state || instruction_needs_state
}

fn transaction_target_routing_requires_state(tx: &AcceptedTransaction<'_>) -> bool {
    let Some(executable) = transaction_executable(tx) else {
        return false;
    };

    match executable {
        Executable::Instructions(instructions) => instructions.iter().any(|instruction| {
            instruction_transaction_dataspace_target_needs_state(&**instruction)
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) => false,
        Executable::IvmProved(proved) => proved.overlay.iter().any(|instruction| {
            instruction_transaction_dataspace_target_needs_state(&**instruction)
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use iroha_config::parameters::actual::{LaneRoutingMatcher, LaneRoutingRule};
    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
    use iroha_data_model::{
        Encode, IntoKeyValue,
        account::{AccountAddress, AccountAliasDomain},
        asset::{
            AssetDefinitionAlias, Mintable, NewAssetDefinition, definition::AssetConfidentialPolicy,
        },
        isi::{
            offline::{IssueOfflineNote, KagemushaTransfer},
            prelude::{Mint, Register, Transfer},
            settlement::{
                DvpIsi, PvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementLeg,
                SettlementPlan,
            },
            smart_contract_code::RegisterSmartContractBytes,
        },
        metadata::Metadata,
        nexus::{LaneConfig, UniversalAccountId},
        offline::{OfflineNoteIssue, OfflineNoteKeyCertificate},
        permission::Permission,
        prelude::*,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        sns::{NameControllerV1, NameRecordV1},
        transaction::TransactionBuilder,
    };
    use iroha_executor_data_model::permission::{
        account::{AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias},
        nexus::{CanPublishSpaceDirectoryManifest, CanUseFeeSponsor},
        trigger::CanRegisterTrigger,
    };
    use iroha_primitives::numeric::{Numeric, NumericSpec};
    use iroha_test_samples::gen_account_in;
    use nonzero_ext::nonzero;

    use super::*;

    fn sample_transaction(
        authority: &AccountId,
        signer: &iroha_crypto::PrivateKey,
        instructions: Vec<InstructionBox>,
    ) -> AcceptedTransaction<'static> {
        sample_transaction_with_metadata(authority, signer, instructions, Metadata::default())
    }

    fn sample_transaction_with_metadata(
        authority: &AccountId,
        signer: &iroha_crypto::PrivateKey,
        instructions: Vec<InstructionBox>,
        metadata: Metadata,
    ) -> AcceptedTransaction<'static> {
        let chain_id = ChainId::from("chain");
        let tx = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_instructions(instructions)
            .with_metadata(metadata)
            .sign(signer);
        let default_limits = TransactionParameters::default();
        let params = TransactionParameters::with_max_signatures(
            nonzero!(16_u64),
            nonzero!(4096_u64),
            nonzero!(4096_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        AcceptedTransaction::accept(
            tx,
            &chain_id,
            core::time::Duration::from_secs(30),
            params,
            &crypto_cfg,
        )
        .expect("tx should be accepted")
    }

    fn sample_executable_transaction(
        authority: &AccountId,
        signer: &iroha_crypto::PrivateKey,
        executable: iroha_data_model::transaction::Executable,
    ) -> AcceptedTransaction<'static> {
        sample_executable_transaction_with_metadata(
            authority,
            signer,
            executable,
            Metadata::default(),
        )
    }

    fn sample_executable_transaction_with_metadata(
        authority: &AccountId,
        signer: &iroha_crypto::PrivateKey,
        executable: iroha_data_model::transaction::Executable,
        mut metadata: Metadata,
    ) -> AcceptedTransaction<'static> {
        let chain_id = ChainId::from("chain");
        metadata.insert(
            "gas_limit".parse().expect("gas_limit key"),
            iroha_primitives::json::Json::new(10_000_u64),
        );
        let tx = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_executable(executable)
            .with_metadata(metadata)
            .sign(signer);
        let default_limits = TransactionParameters::default();
        let params = TransactionParameters::with_max_signatures(
            nonzero!(16_u64),
            nonzero!(4096_u64),
            nonzero!(4096_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        AcceptedTransaction::accept(
            tx,
            &chain_id,
            core::time::Duration::from_secs(30),
            params,
            &crypto_cfg,
        )
        .expect("tx should be accepted")
    }

    fn sample_proved_executable(
        overlay: Vec<InstructionBox>,
    ) -> iroha_data_model::transaction::Executable {
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version: 1,
        };
        let mut bytecode = meta.encode();
        bytecode.extend_from_slice(b"LTLB");
        bytecode.extend_from_slice(&0u32.to_le_bytes());
        bytecode.extend_from_slice(&0u32.to_le_bytes());
        bytecode.extend_from_slice(&0u32.to_le_bytes());
        bytecode.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());

        iroha_data_model::transaction::Executable::IvmProved(
            iroha_data_model::transaction::IvmProved {
                bytecode: iroha_data_model::transaction::IvmBytecode::from_compiled(bytecode),
                overlay: overlay.into(),
                events_commitment: Hash::new(b"events"),
                gas_policy_commitment: Hash::new(b"gas-policy"),
            },
        )
    }

    fn catalog_with_lane_dataspaces(entries: &[(LaneId, DataSpaceId)]) -> LaneCatalog {
        let max_lane = entries
            .iter()
            .map(|(lane, _)| lane.as_u32())
            .max()
            .unwrap_or(0);
        let lane_count =
            std::num::NonZeroU32::new(max_lane + 1).expect("catalog requires nonzero lanes");
        let lanes = entries
            .iter()
            .map(|(lane_id, dataspace_id)| LaneConfig {
                id: *lane_id,
                dataspace_id: *dataspace_id,
                alias: format!("lane-{}", lane_id.as_u32()),
                ..LaneConfig::default()
            })
            .collect();
        LaneCatalog::new(lane_count, lanes).expect("valid lane catalog")
    }

    fn catalog_with_lanes(lanes: &[LaneId]) -> LaneCatalog {
        let entries: Vec<(LaneId, DataSpaceId)> = lanes
            .iter()
            .map(|lane_id| (*lane_id, DataSpaceId::UNIVERSAL))
            .collect();
        catalog_with_lane_dataspaces(&entries)
    }

    fn blank_state() -> crate::state::State {
        let world = crate::state::World::default();
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let telemetry = crate::telemetry::StateTelemetry::default();
        #[cfg(feature = "telemetry")]
        return crate::state::State::with_telemetry(world, kura, query, telemetry);
        #[cfg(not(feature = "telemetry"))]
        crate::state::State::new(world, kura, query)
    }

    fn install_router_nexus(state: &crate::state::State, router: &ConfigLaneRouter) {
        let mut nexus = state.nexus.write();
        nexus.routing_policy = router.policy.as_ref().clone();
        nexus.dataspace_catalog = router.dataspace_catalog.as_ref().clone();
        nexus.lane_catalog = router.lane_catalog.as_ref().clone();
    }

    fn dataspace_catalog(entries: &[(DataSpaceId, &str)]) -> DataSpaceCatalog {
        let mut metadata = vec![iroha_data_model::nexus::DataSpaceMetadata::default()];
        metadata.extend(entries.iter().map(|(id, alias)| {
            iroha_data_model::nexus::DataSpaceMetadata {
                id: *id,
                alias: (*alias).to_string(),
                description: None,
                fault_tolerance: 1,
            }
        }));
        DataSpaceCatalog::new(metadata).expect("valid dataspace catalog")
    }

    fn world_with_dynamic_dataspace_until(
        alias: &str,
        owner: &AccountId,
        expires_at_ms: u64,
    ) -> crate::state::World {
        let selector = crate::sns::selector_for_dataspace_alias(alias).expect("selector");
        let address = AccountAddress::from_account_id(owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            0,
            expires_at_ms,
            expires_at_ms.saturating_add(100),
            expires_at_ms.saturating_add(200),
            Metadata::default(),
        );
        let mut world = crate::state::World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(crate::sns::record_storage_key(&selector), record.encode());
        world
    }

    fn world_with_dynamic_dataspace(alias: &str, owner: &AccountId) -> crate::state::World {
        world_with_dynamic_dataspace_until(alias, owner, u64::MAX)
    }

    fn account_alias(literal: &str, catalog: &DataSpaceCatalog) -> AccountAlias {
        AccountAlias::from_literal(literal, catalog).expect("valid account alias")
    }

    fn state_with_account_aliases(
        accounts: &[(AccountId, AccountAlias)],
        dataspace_catalog: DataSpaceCatalog,
    ) -> crate::state::State {
        let mut world = crate::state::World::default();
        for (account_id, alias) in accounts {
            let account = Account::new(account_id.clone())
                .with_label(Some(alias.clone()))
                .build(account_id);
            let (account_id, account_value) = account.into_key_value();
            world.accounts.insert(account_id.clone(), account_value);
            world
                .account_aliases
                .insert(alias.clone(), account_id.clone());
            world
                .account_aliases_by_account
                .insert(account_id, BTreeSet::from([alias.clone()]));
        }
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let telemetry = crate::telemetry::StateTelemetry::default();
        #[cfg(feature = "telemetry")]
        let state = crate::state::State::with_telemetry(world, kura, query, telemetry);
        #[cfg(not(feature = "telemetry"))]
        let state = crate::state::State::new(world, kura, query);
        state.nexus.write().dataspace_catalog = dataspace_catalog;
        state
    }

    fn state_with_account_scope_entries(
        accounts: &[(
            AccountId,
            crate::nexus::space_directory::AccountScopeDirectoryEntry,
        )],
        dataspace_catalog: DataSpaceCatalog,
    ) -> crate::state::State {
        let mut state = blank_state();
        state.nexus.write().dataspace_catalog = dataspace_catalog;
        for (account_id, entry) in accounts {
            let account = Account::new(account_id.clone()).build(account_id);
            let (account_id, account_value) = account.into_key_value();
            state
                .world
                .accounts
                .insert(account_id.clone(), account_value);
            state
                .world
                .account_scope_directory
                .insert(account_id, entry.clone());
        }
        state
    }

    fn state_with_asset_definitions(
        asset_definitions: Vec<AssetDefinition>,
        dataspace_catalog: DataSpaceCatalog,
        lane_catalog: LaneCatalog,
    ) -> crate::state::State {
        let mut state = blank_state();
        {
            let mut nexus = state.nexus.write();
            nexus.dataspace_catalog = dataspace_catalog;
            nexus.lane_catalog = lane_catalog;
        }
        for asset_definition in asset_definitions {
            state
                .world
                .asset_definitions
                .insert(asset_definition.id.clone(), asset_definition);
        }
        state
    }

    fn bind_asset_definition_alias(
        state: &mut crate::state::State,
        asset_definition: &AssetDefinitionId,
        alias: &str,
    ) {
        let alias: AssetDefinitionAlias = alias.parse().expect("asset alias");
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition.clone(),
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );
    }

    fn add_unlabeled_account_with_alias(
        state: &mut crate::state::State,
        account_id: &AccountId,
        alias: AccountAlias,
    ) {
        let account = Account::new(account_id.clone()).build(account_id);
        let (account_id, account_value) = account.into_key_value();
        state
            .world
            .accounts
            .insert(account_id.clone(), account_value);
        state
            .world
            .account_aliases
            .insert(alias.clone(), account_id.clone());
        state
            .world
            .account_aliases_by_account
            .insert(account_id, BTreeSet::from([alias]));
    }

    fn scope_account_to_dataspace(
        state: &mut crate::state::State,
        account_id: &AccountId,
        dataspace_id: DataSpaceId,
    ) {
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        state
            .world
            .account_scope_directory
            .insert(account_id.clone(), scope_entry);
    }

    fn sample_signature(seed: u8) -> Signature {
        let mut payload = [0u8; 64];
        for (idx, byte) in payload.iter_mut().enumerate() {
            *byte = seed.wrapping_add(u8::try_from(idx).expect("index fits into u8"));
        }
        Signature::from_bytes(&payload)
    }

    fn sample_offline_certificate(account_id: AccountId) -> OfflineNoteKeyCertificate {
        let keypair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
        let (_algorithm, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid");
        OfflineNoteKeyCertificate {
            version: iroha_data_model::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
            platform: "ios-appattest".to_owned(),
            key_id: "one-use-key".to_owned(),
            device_id: "device-1".to_owned(),
            account_id,
            public_key: public_key.to_vec(),
            assertion_scheme: "apple-appattest-counter".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: sample_signature(0x44),
        }
    }

    #[test]
    fn applies_account_and_instruction_rules() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (_bob_id, _) = gen_account_in("wonderland");

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(0),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![
                LaneRoutingRule {
                    lane: LaneId::new(1),
                    dataspace: None,
                    matcher: LaneRoutingMatcher {
                        account: Some(alice_id.to_string()),
                        instruction: Some("Mint".into()),
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: None,
                    matcher: LaneRoutingMatcher {
                        account: Some(
                            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53".into(),
                        ),
                        instruction: None,
                        description: None,
                    },
                },
            ],
        };
        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1), LaneId::new(2)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);

        let asset_definition: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_definition.clone(), alice_id.clone());
        let mint = Mint::asset_numeric(1u32, asset_id);
        let register = Register::asset_definition(
            AssetDefinition::numeric(asset_definition.clone())
                .with_name(asset_definition.name().to_string()),
        );

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(mint), InstructionBox::from(register)],
        );

        let state = blank_state();
        install_router_nexus(&state, &router);
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id.as_u32(), 1);
        assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);

        // Non-matching instruction should fall back to default lane.
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("fallback", "universal").expect("domain"),
            )))],
        );
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id.as_u32(), 0);
    }

    #[test]
    fn single_lane_router_supports_state_free_routing() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("single", "universal").expect("domain"),
            )))],
        );
        let state = blank_state();
        let router = SingleLaneRouter::new();
        let with_view = router.route_with_view(&tx, &state.view());
        let without_view = router.route_without_state(&tx);
        assert_eq!(without_view, Some(with_view));
    }

    #[test]
    fn config_lane_router_state_free_path_matches_view_path() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(3),
                dataspace: Some(DataSpaceId::new(7)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: Some("register::domain".to_string()),
                    description: None,
                },
            }],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(3), DataSpaceId::new(7)),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "alpha".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");
        let router = ConfigLaneRouter::new(policy, dataspace_catalog, lane_catalog);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("statefree", "universal").expect("domain"),
            )))],
        );
        let state = blank_state();
        install_router_nexus(&state, &router);
        let with_view = router.route_with_view(&tx, &state.view());
        let without_view = router.route_without_state(&tx);
        assert_eq!(without_view, Some(with_view));
    }

    #[test]
    fn resolve_policy_rejects_rule_dataspace_override_for_target() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(5),
                dataspace: Some(DataSpaceId::new(7)),
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("register".to_string()),
                    description: None,
                },
            }],
        };

        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "alpha".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");

        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(5), DataSpaceId::new(7)),
        ]);
        assert_eq!(
            resolve_policy_routing_decision(
                &policy,
                Some(&policy.rules[0]),
                Some(DataSpaceId::UNIVERSAL),
                false,
                &lane_catalog,
                &dataspace_catalog,
            ),
            Err(RoutingResolveError::LaneDataspaceMismatch {
                lane_id: LaneId::new(5),
                lane_dataspace_id: DataSpaceId::new(7),
                dataspace_id: DataSpaceId::UNIVERSAL,
            })
        );
    }

    #[test]
    fn settlement_routes_to_common_leg_dataspace_before_account_policy() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(7)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };
        let router = ConfigLaneRouter::new(
            policy,
            dataspace_catalog(&[(DataSpaceId::new(7), "restricted")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(1), DataSpaceId::new(7)),
            ]),
        );
        let delivery_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "universal").expect("domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let payment_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "universal").expect("domain id"),
            "cash".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(DvpIsi::new(
                "commonroute".parse().expect("settlement id"),
                SettlementLeg::new(
                    delivery_definition,
                    Numeric::from(1_u32),
                    alice_id.clone(),
                    bob_id.clone(),
                ),
                SettlementLeg::new(
                    payment_definition,
                    Numeric::from(1_u32),
                    bob_id,
                    alice_id.clone(),
                ),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            ))],
        );

        assert_eq!(
            router.try_route(&tx).expect("settlement route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn settlement_with_different_leg_dataspaces_routes_to_universal_coordinator() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(1),
            default_dataspace: DataSpaceId::new(7),
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(7)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };
        let router = ConfigLaneRouter::new(
            policy,
            dataspace_catalog(&[
                (DataSpaceId::new(7), "delivery"),
                (DataSpaceId::new(9), "payment"),
            ]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(1), DataSpaceId::new(7)),
                (LaneId::new(2), DataSpaceId::new(9)),
            ]),
        );
        let delivery_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "delivery").expect("domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let payment_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "payment").expect("domain id"),
            "cash".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(DvpIsi::new(
                "crossroute".parse().expect("settlement id"),
                SettlementLeg::new(
                    delivery_definition,
                    Numeric::from(1_u32),
                    alice_id.clone(),
                    bob_id.clone(),
                ),
                SettlementLeg::new(
                    payment_definition,
                    Numeric::from(1_u32),
                    bob_id,
                    alice_id.clone(),
                ),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            ))],
        );

        assert_eq!(
            router.try_route(&tx).expect("settlement route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn settlement_pvp_with_different_leg_dataspaces_routes_to_universal_coordinator() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(1),
            default_dataspace: DataSpaceId::new(7),
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(7)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };
        let router = ConfigLaneRouter::new(
            policy,
            dataspace_catalog(&[
                (DataSpaceId::new(7), "primary"),
                (DataSpaceId::new(9), "counter"),
            ]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(1), DataSpaceId::new(7)),
                (LaneId::new(2), DataSpaceId::new(9)),
            ]),
        );
        let primary_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "primary").expect("domain id"),
            "usd".parse().expect("asset definition name"),
        );
        let counter_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "counter").expect("domain id"),
            "eur".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(PvpIsi::new(
                "pvpcrossroute".parse().expect("settlement id"),
                SettlementLeg::new(
                    primary_definition,
                    Numeric::from(1_u32),
                    alice_id.clone(),
                    bob_id.clone(),
                ),
                SettlementLeg::new(
                    counter_definition,
                    Numeric::from(1_u32),
                    bob_id,
                    alice_id.clone(),
                ),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            ))],
        );

        assert_eq!(
            router.try_route(&tx).expect("settlement route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn route_resolution_rejects_lane_dataspace_mismatch() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(4),
                dataspace: Some(DataSpaceId::new(9)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "beta".to_string(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: DataSpaceId::new(9),
                alias: "gamma".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");

        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(4), DataSpaceId::new(7)),
        ]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog.clone());
        let state = blank_state();
        install_router_nexus(&state, &router);

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("override", "universal").expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(4));
        assert_eq!(decision.dataspace_id, DataSpaceId::new(9));

        let helper_err =
            evaluate_policy_with_catalog(router.policy.as_ref(), &lane_catalog, &catalog, &tx)
                .expect_err("mismatched lane/dataspace must be rejected");
        assert!(matches!(
            helper_err,
            RoutingResolveError::LaneDataspaceMismatch { .. }
        ));
    }

    #[test]
    fn route_resolution_rejects_unknown_lane() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(9),
                dataspace: Some(DataSpaceId::new(7)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "alpha".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");

        let lane_catalog =
            catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog.clone());
        let state = blank_state();
        install_router_nexus(&state, &router);

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("fallback", "universal").expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(9));
        assert_eq!(decision.dataspace_id, DataSpaceId::new(7));

        let helper_err =
            evaluate_policy_with_catalog(router.policy.as_ref(), &lane_catalog, &catalog, &tx)
                .expect_err("unknown lane must be rejected");
        assert!(matches!(
            helper_err,
            RoutingResolveError::UnknownLane { .. }
        ));
    }

    #[test]
    fn route_resolution_rejects_missing_default_lane() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(9),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(11),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "alpha".to_string(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: DataSpaceId::new(9),
                alias: "beta".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");

        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::new(2), DataSpaceId::new(7)),
            (LaneId::new(4), DataSpaceId::new(9)),
        ]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog.clone());
        let state = blank_state();
        install_router_nexus(&state, &router);

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("fallback", "universal").expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(11));
        assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);

        let helper_err =
            evaluate_policy_with_catalog(router.policy.as_ref(), &lane_catalog, &catalog, &tx)
                .expect_err("missing default lane must be rejected");
        assert!(matches!(
            helper_err,
            RoutingResolveError::UnknownLane { .. }
        ));
    }

    #[test]
    fn route_resolution_rejects_missing_default_dataspace() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::new(11),
            rules: vec![LaneRoutingRule {
                lane: LaneId::SINGLE,
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: DataSpaceId::new(7),
            alias: "alpha".to_string(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("valid dataspace catalog");

        let lane_catalog = catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::new(9))]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog.clone());

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("fallback", "universal").expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::SINGLE);
        assert_eq!(decision.dataspace_id, DataSpaceId::UNIVERSAL);

        let helper_err =
            evaluate_policy_with_catalog(router.policy.as_ref(), &lane_catalog, &catalog, &tx)
                .expect_err("missing default dataspace must be rejected");
        assert!(matches!(
            helper_err,
            RoutingResolveError::UnknownDataspace { .. }
        ));
    }

    #[test]
    fn route_resolution_accepts_dynamic_dataspace_on_default_public_lane() {
        let dynamic_dataspace = DataSpaceId::new(4_242);
        let lane_catalog =
            catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
        let catalog = dataspace_catalog(&[]);
        let decision = RoutingDecision::new(LaneId::SINGLE, dynamic_dataspace);

        assert_eq!(
            resolve_routing_decision(decision, &lane_catalog, &catalog),
            Ok(decision)
        );
    }

    #[test]
    fn route_resolution_rejects_unknown_dataspace_on_non_default_universal_lane() {
        let dynamic_dataspace = DataSpaceId::new(4_242);
        let lane_catalog =
            catalog_with_lane_dataspaces(&[(LaneId::new(2), DataSpaceId::UNIVERSAL)]);
        let catalog = dataspace_catalog(&[]);
        let err = resolve_routing_decision(
            RoutingDecision::new(LaneId::new(2), dynamic_dataspace),
            &lane_catalog,
            &catalog,
        )
        .expect_err("non-default universal lanes must not accept unknown dataspaces");

        assert!(matches!(
            err,
            RoutingResolveError::UnknownDataspace { dataspace_id }
                if dataspace_id == dynamic_dataspace
        ));
    }

    #[test]
    fn route_resolution_rejects_dynamic_dataspace_on_dataspace_scoped_lane() {
        let configured_dataspace = DataSpaceId::new(7);
        let dynamic_dataspace = DataSpaceId::new(4_242);
        let lane_catalog = catalog_with_lane_dataspaces(&[(LaneId::SINGLE, configured_dataspace)]);
        let catalog = dataspace_catalog(&[(configured_dataspace, "configured")]);
        let err = resolve_routing_decision(
            RoutingDecision::new(LaneId::SINGLE, dynamic_dataspace),
            &lane_catalog,
            &catalog,
        )
        .expect_err("dataspace-scoped lanes must not accept unknown dataspaces");

        assert!(matches!(
            err,
            RoutingResolveError::UnknownDataspace { dataspace_id }
                if dataspace_id == dynamic_dataspace
        ));
    }

    #[test]
    fn route_resolution_rejects_universal_when_catalog_omits_universal() {
        let lane_catalog =
            catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
        let catalog = DataSpaceCatalog::new(vec![iroha_data_model::nexus::DataSpaceMetadata {
            id: DataSpaceId::new(7),
            alias: "configured".to_owned(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("catalog without universal");
        let err = resolve_routing_decision(
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            &lane_catalog,
            &catalog,
        )
        .expect_err("reserved universal dataspace still needs a catalog entry");

        assert!(matches!(
            err,
            RoutingResolveError::UnknownDataspace { dataspace_id }
                if dataspace_id == DataSpaceId::UNIVERSAL
        ));
    }

    #[test]
    fn dataspace_alias_target_with_world_resolves_active_sns_dataspace() {
        let (authority_id, _) = gen_account_in("wonderland");
        let catalog = dataspace_catalog(&[]);
        let world = world_with_dynamic_dataspace("alpha", &authority_id);
        let view = world.view();
        let expected = crate::sns::dataspace_id_for_sns_alias("alpha").expect("dynamic id");

        assert_eq!(
            dataspace_alias_target_with_world("alpha", Some(&catalog), &view, Some(0)),
            Some(expected)
        );
        assert_eq!(
            dataspace_alias_target_with_world("missing", Some(&catalog), &view, Some(0)),
            None
        );
        assert_eq!(
            dataspace_alias_target_with_world("alpha", Some(&catalog), &view, None),
            None
        );
    }

    #[test]
    fn dataspace_alias_target_with_world_rejects_inactive_sns_dataspace_at_ledger_time() {
        let (authority_id, _) = gen_account_in("wonderland");
        let catalog = dataspace_catalog(&[]);
        let world = world_with_dynamic_dataspace_until("alpha", &authority_id, 10);
        let view = world.view();
        let expected = crate::sns::dataspace_id_for_sns_alias("alpha").expect("dynamic id");

        assert_eq!(
            dataspace_alias_target_with_world("alpha", Some(&catalog), &view, Some(9)),
            Some(expected)
        );
        assert_eq!(
            dataspace_alias_target_with_world("alpha", Some(&catalog), &view, Some(10)),
            None
        );
    }

    #[test]
    fn evaluate_policy_with_catalog_and_world_resolves_static_alias_without_ledger_time() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let static_dataspace = DataSpaceId::new(7);
        let catalog = dataspace_catalog(&[(static_dataspace, "alpha")]);
        let lane_catalog =
            catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("static", "alpha").expect("domain"),
            )))],
        );
        let world = crate::state::World::default();
        let view = world.view();

        assert_eq!(
            evaluate_policy_with_catalog_and_world(&policy, &lane_catalog, &catalog, &tx, &view)
                .expect("static alias should resolve without a ledger time"),
            RoutingDecision::new(LaneId::SINGLE, static_dataspace)
        );
    }

    #[test]
    fn evaluate_policy_with_catalog_and_world_at_respects_dynamic_sns_ledger_time() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let catalog = dataspace_catalog(&[]);
        let lane_catalog =
            catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("dynamic", "alpha").expect("domain"),
            )))],
        );
        let world = world_with_dynamic_dataspace_until("alpha", &authority_id, 10);
        let view = world.view();
        let expected = crate::sns::dataspace_id_for_sns_alias("alpha").expect("dynamic id");

        assert_eq!(
            evaluate_policy_with_catalog_and_world_at(
                &policy,
                &lane_catalog,
                &catalog,
                &tx,
                &view,
                9,
            )
            .expect("active dynamic route should resolve"),
            RoutingDecision::new(LaneId::SINGLE, expected)
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world_at(
                &policy,
                &lane_catalog,
                &catalog,
                &tx,
                &view,
                10,
            )
            .expect("inactive dynamic route should fall back to default"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(&policy, &lane_catalog, &catalog, &tx, &view)
                .expect("no-time world route should fall back to static catalog only"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn evaluate_policy_plan_with_catalog_and_world_at_respects_dynamic_sns_ledger_time() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let catalog = dataspace_catalog(&[]);
        let lane_catalog =
            catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("dynamic", "alpha").expect("domain"),
            )))],
        );
        let world = world_with_dynamic_dataspace_until("alpha", &authority_id, 10);
        let view = world.view();
        let expected = crate::sns::dataspace_id_for_sns_alias("alpha").expect("dynamic id");

        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world_at(
                &policy,
                &lane_catalog,
                &catalog,
                &tx,
                &view,
                9,
            )
            .expect("active dynamic plan should resolve"),
            RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, expected))
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world_at(
                &policy,
                &lane_catalog,
                &catalog,
                &tx,
                &view,
                10,
            )
            .expect("inactive dynamic plan should fall back to default"),
            RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                &lane_catalog,
                &catalog,
                &tx,
                &view
            )
            .expect("no-time world plan should fall back to static catalog only"),
            RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
        );
    }

    #[test]
    fn matches_register_domain_rule() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("register::domain".to_string()),
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("castle", "universal").expect("domain id"),
            )))],
        );

        let state = blank_state();
        install_router_nexus(&state, &router);
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn matches_smartcontract_deploy_rule() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("smartcontract::deploy".to_string()),
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let register = RegisterSmartContractBytes {
            code_hash: Hash::new(&code),
            code,
        };
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(register)],
        );

        let state = blank_state();
        install_router_nexus(&state, &router);
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn contract_call_routes_to_contract_address_dataspace_without_explicit_rule() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (lane_id, dataspace_id),
            ]),
        );
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            0,
            &alice_id,
            0,
            dataspace_id,
        )
        .expect("contract address");
        let invocation = iroha_data_model::transaction::executable::ContractInvocation {
            contract_address,
            entrypoint: "transfer".to_owned(),
            payload: None,
        };
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            iroha_data_model::transaction::Executable::ContractCall(invocation),
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("contract call route must resolve"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_proved_coverage_overlay_domain_routes_to_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (lane_id, dataspace_id),
            ]),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("merchant", "paynet").expect("domain id"),
            )))]),
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("proved overlay domain route must resolve"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_proved_coverage_overlay_conflicting_domains_route_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(10);
        let second_dataspace = DataSpaceId::new(11);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(first_dataspace, "paynet"), (second_dataspace, "cbuae")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "paynet").expect("domain id"),
                ))),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("issuer", "cbuae").expect("domain id"),
                ))),
            ]),
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("mixed proved overlay domain targets should route to AMX coordinator"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_home_proved_coverage_overlay_permission_routes_to_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (lane_id, dataspace_id),
            ]),
        );
        let permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: dataspace_id,
        }
        .into();
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(Grant::account_permission(
                permission, bob_id,
            ))]),
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("proved overlay dataspace permission route must resolve"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_proved_coverage_overlay_conflicting_permissions_route_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(10);
        let second_dataspace = DataSpaceId::new(11);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(first_dataspace, "paynet"), (second_dataspace, "cbuae")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let first_permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: first_dataspace,
        }
        .into();
        let second_permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: second_dataspace,
        }
        .into();
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![
                InstructionBox::from(Grant::account_permission(
                    first_permission,
                    alice_id.clone(),
                )),
                InstructionBox::from(Revoke::account_permission(
                    second_permission,
                    alice_id.clone(),
                )),
            ]),
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("mixed proved overlay permissions should route to AMX coordinator"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_home_proved_state_coverage_overlay_mint_global_binding_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))]),
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        bind_asset_definition_alias(&mut state, &asset_definition, "pkr#paynet");
        let state_view = state.view();

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("proved overlay mint needs state for stored alias"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state_view)
                .expect("proved overlay mint route must resolve with state"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                router.policy.as_ref(),
                router.lane_catalog.as_ref(),
                router.dataspace_catalog.as_ref(),
                &tx,
                state_view.world(),
            )
            .expect("proved overlay mint route must resolve with world"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_home_proved_state_coverage_overlay_permission_uses_stored_alias_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition {
                    asset_definition: asset_definition.clone(),
                },
                bob_id,
            ))]),
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        bind_asset_definition_alias(&mut state, &asset_definition, "pkr#paynet");
        let state_view = state.view();

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("proved overlay permission needs state for stored alias"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state_view)
                .expect("proved overlay permission route must resolve with state"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                router.policy.as_ref(),
                router.lane_catalog.as_ref(),
                router.dataspace_catalog.as_ref(),
                &tx,
                state_view.world(),
            )
            .expect("proved overlay permission route must resolve with world"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_proved_state_coverage_overlay_state_resolved_conflict_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let paynet = DataSpaceId::new(10);
        let cbuae = DataSpaceId::new(11);
        let dataspace_catalog = dataspace_catalog(&[(paynet, "paynet"), (cbuae, "cbuae")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(2), paynet),
            (LaneId::new(3), cbuae),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![
                InstructionBox::from(Mint::asset_numeric(
                    1_u32,
                    AssetId::of(asset_definition.clone(), alice_id.clone()),
                )),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("issuer", "cbuae").expect("domain id"),
                ))),
            ]),
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        bind_asset_definition_alias(&mut state, &asset_definition, "pkr#paynet");
        let state_view = state.view();

        assert_eq!(
            router
                .try_route_with_view(&tx, &state_view)
                .expect("state-resolved proved overlay targets should route to AMX coordinator"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                router.policy.as_ref(),
                router.lane_catalog.as_ref(),
                router.dataspace_catalog.as_ref(),
                &tx,
                state_view.world(),
            )
            .expect("state-resolved world overlay targets should route to AMX coordinator"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_home_proved_settlement_coverage_overlay_dvp_routes_to_common_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (lane_id, dataspace_id),
            ]),
        );
        let delivery_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "paynet").expect("domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let payment_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "paynet").expect("domain id"),
            "cash".parse().expect("asset definition name"),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(DvpIsi::new(
                "proved-dvp-common".parse().expect("settlement id"),
                SettlementLeg::new(
                    delivery_definition,
                    Numeric::from(1_u32),
                    alice_id.clone(),
                    bob_id.clone(),
                ),
                SettlementLeg::new(
                    payment_definition,
                    Numeric::from(1_u32),
                    bob_id,
                    alice_id.clone(),
                ),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            ))]),
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("proved DVP route should be state-free"),
            Some(RoutingDecision::new(lane_id, dataspace_id))
        );
        assert_eq!(
            router
                .try_route(&tx)
                .expect("proved DVP route must resolve"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_proved_settlement_coverage_overlay_pvp_cross_dataspace_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let paynet = DataSpaceId::new(10);
        let cbuae = DataSpaceId::new(11);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::new(2),
                default_dataspace: paynet,
                rules: vec![],
            },
            dataspace_catalog(&[(paynet, "paynet"), (cbuae, "cbuae")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), paynet),
                (LaneId::new(3), cbuae),
            ]),
        );
        let primary_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "paynet").expect("domain id"),
            "usd".parse().expect("asset definition name"),
        );
        let counter_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "cbuae").expect("domain id"),
            "aed".parse().expect("asset definition name"),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(PvpIsi::new(
                "proved-pvp-cross".parse().expect("settlement id"),
                SettlementLeg::new(
                    primary_definition,
                    Numeric::from(1_u32),
                    alice_id.clone(),
                    bob_id.clone(),
                ),
                SettlementLeg::new(
                    counter_definition,
                    Numeric::from(1_u32),
                    bob_id,
                    alice_id.clone(),
                ),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            ))]),
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("proved cross-dataspace PVP route should be state-free"),
            Some(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
        );
        assert_eq!(
            router
                .try_route(&tx)
                .expect("proved cross-dataspace PVP route must resolve"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_home_proved_settlement_overlay_dvp_global_bindings_route_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let delivery_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "universal").expect("domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let payment_definition = AssetDefinitionId::new(
            DomainId::try_new("settlement", "universal").expect("domain id"),
            "cash".parse().expect("asset definition name"),
        );
        let opaque_delivery =
            AssetDefinitionId::parse_address_literal(&delivery_definition.canonical_address())
                .expect("opaque delivery definition id");
        let opaque_payment =
            AssetDefinitionId::parse_address_literal(&payment_definition.canonical_address())
                .expect("opaque payment definition id");
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(DvpIsi::new(
                "proved-dvp-alias".parse().expect("settlement id"),
                SettlementLeg::new(
                    opaque_delivery,
                    Numeric::from(1_u32),
                    alice_id.clone(),
                    bob_id.clone(),
                ),
                SettlementLeg::new(
                    opaque_payment,
                    Numeric::from(1_u32),
                    bob_id,
                    alice_id.clone(),
                ),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            ))]),
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(delivery_definition.clone())
                    .with_name("bond".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
                AssetDefinition::numeric(payment_definition.clone())
                    .with_name("cash".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        bind_asset_definition_alias(&mut state, &delivery_definition, "bond#paynet");
        bind_asset_definition_alias(&mut state, &payment_definition, "cash#paynet");
        let state_view = state.view();

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("proved opaque DVP needs state for stored aliases"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state_view)
                .expect("proved opaque DVP route must resolve with state"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                router.policy.as_ref(),
                router.lane_catalog.as_ref(),
                router.dataspace_catalog.as_ref(),
                &tx,
                state_view.world(),
            )
            .expect("proved opaque DVP route must resolve with world"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_home_proved_account_coverage_overlay_permission_routes_by_destination_account() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![
                LaneRoutingRule {
                    lane: LaneId::new(1),
                    dataspace: Some(DataSpaceId::new(1)),
                    matcher: LaneRoutingMatcher {
                        account: Some(alice_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: Some(DataSpaceId::new(2)),
                    matcher: LaneRoutingMatcher {
                        account: Some(bob_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
            ],
        };
        let router = ConfigLaneRouter::new(
            policy,
            dataspace_catalog(&[(DataSpaceId::new(1), "alice"), (DataSpaceId::new(2), "bob")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(1), DataSpaceId::new(1)),
                (LaneId::new(2), DataSpaceId::new(2)),
            ]),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
                    account: alice_id.clone(),
                },
                bob_id,
            ))]),
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("proved account permission route should be state-free"),
            Some(RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)))
        );
        assert_eq!(
            router
                .try_route(&tx)
                .expect("proved account permission route must resolve"),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2))
        );
    }

    #[test]
    fn contract_instance_activation_routes_to_contract_address_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (lane_id, dataspace_id),
            ]),
        );
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            0,
            &alice_id,
            0,
            dataspace_id,
        )
        .expect("contract address");
        let instruction = iroha_data_model::isi::smart_contract_code::ActivateContractInstance {
            contract_address,
            code_hash: Hash::new(b"contract-code"),
        };
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(instruction)],
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("contract activation route must resolve"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn smart_contract_deploy_rule_with_target_dataspace_builds_native_amx_plan() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let zk_dataspace = DataSpaceId::new(2);
        let contract_dataspace = DataSpaceId::new(10);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: Some(zk_dataspace),
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("smartcontract::deploy".to_owned()),
                        description: None,
                    },
                }],
            },
            dataspace_catalog(&[(zk_dataspace, "zk"), (contract_dataspace, "sbp")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), zk_dataspace),
                (LaneId::new(3), contract_dataspace),
            ]),
        );
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            0,
            &alice_id,
            0,
            contract_dataspace,
        )
        .expect("contract address");
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![
                InstructionBox::from(RegisterSmartContractBytes {
                    code_hash: Hash::new(&code),
                    code,
                }),
                InstructionBox::from(
                    iroha_data_model::isi::smart_contract_code::ActivateContractInstance {
                        contract_address,
                        code_hash: Hash::new(b"contract-code"),
                    },
                ),
            ],
        );

        let plan = router
            .try_route_plan(&tx)
            .expect("contract deploy rule and target dataspace should build a native AMX plan");
        let RoutingPlan::NativeAmx(plan) = plan else {
            panic!("contract deploy should not collapse to a single mismatched route");
        };
        assert_eq!(
            plan.coordinator.route,
            RoutingDecision::new(LaneId::new(2), zk_dataspace)
        );
        assert_eq!(
            plan.participants,
            vec![
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(2), zk_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(3), contract_dataspace),
                    RouteLegRole::Participant,
                ),
            ]
        );
    }

    #[test]
    fn musubi_package_alias_routes_to_package_namespace_dataspace_without_explicit_rule() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (lane_id, dataspace_id),
            ]),
        );
        let target = iroha_data_model::musubi::MusubiPackageId::from_parts("mibank.paynet", "fx")
            .expect("package id");
        let alias =
            iroha_data_model::musubi::MusubiShortAlias::new("fx".parse().expect("alias"), target);
        let instruction = iroha_data_model::isi::musubi::SetMusubiShortAlias::new(alias);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(instruction)],
        );

        assert_eq!(
            router.try_route(&tx).expect("musubi route must resolve"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn untargeted_authority_transaction_routes_to_single_scope_dataspace_with_state() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            catalog.clone(),
            lane_catalog,
        );
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(RegisterSmartContractBytes {
                code_hash: Hash::new(&code),
                code,
            })],
        );

        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(&[(alice_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = router.lane_catalog.as_ref().clone();

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("untargeted authority transaction should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("authority single-scope route must resolve"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn untargeted_universal_authority_transaction_uses_default_lane_with_state() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let default_lane = LaneId::new(1);
        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, default_lane]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            DataSpaceCatalog::default(),
            lane_catalog,
        );
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(RegisterSmartContractBytes {
                code_hash: Hash::new(&code),
                code,
            })],
        );

        let mut state = blank_state();
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let (account_id, account) = account.into_key_value();
        state.world.accounts.insert(account_id, account);
        install_router_nexus(&state, &router);

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("universal-only account should use configured default lane"),
            RoutingDecision::new(default_lane, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn matches_set_key_value_rule_without_underscores() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("setkeyvalue::account".to_string()),
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let instruction = SetKeyValue::account(
            alice_id.clone(),
            "flag".parse().expect("metadata key"),
            Json::new("on"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(instruction)],
        );

        let state = blank_state();
        install_router_nexus(&state, &router);
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn matches_account_alias_scope_rule() {
        let (uae_id, uae_keypair) = gen_account_in("uae");
        let (bank_id, bank_keypair) = gen_account_in("banka");
        let catalog = DataSpaceCatalog::default();

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: Some("*@uae.universal".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);

        let uae_tx = sample_transaction(
            &uae_id,
            uae_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("uae-match", "universal").expect("domain id"),
            )))],
        );
        let bank_tx = sample_transaction(
            &bank_id,
            bank_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("bank-no-match", "universal").expect("domain id"),
            )))],
        );

        let state = state_with_account_aliases(
            &[
                (
                    uae_id.clone(),
                    account_alias("central@uae.universal", &catalog),
                ),
                (
                    bank_id.clone(),
                    account_alias("settler@banka.universal", &catalog),
                ),
            ],
            catalog,
        );
        install_router_nexus(&state, &router);
        let uae_decision = router.route_with_view(&uae_tx, &state.view());
        let bank_decision = router.route_with_view(&bank_tx, &state.view());

        assert_eq!(uae_decision.lane_id, LaneId::new(1));
        assert_eq!(bank_decision.lane_id, LaneId::SINGLE);
    }

    #[test]
    fn matches_transfer_destination_alias_scope_rule() {
        let (sender_id, sender_keypair) = gen_account_in("banka");
        let (receiver_id, _) = gen_account_in("acme");
        let catalog = DataSpaceCatalog::default();

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("transfer::asset@acme.universal".to_string()),
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);

        let asset_definition: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("uae", "universal").unwrap(),
            "aed".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_definition, sender_id.clone());
        let transfer = Transfer::asset_numeric(asset_id, 1_u32, receiver_id.clone());
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );

        let state = state_with_account_aliases(
            &[
                (
                    sender_id.clone(),
                    account_alias("settler@banka.universal", &catalog),
                ),
                (
                    receiver_id.clone(),
                    account_alias("merchant@acme.universal", &catalog),
                ),
            ],
            catalog,
        );
        install_router_nexus(&state, &router);
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn matches_transferred_domain_scope_rule() {
        let (sender_id, sender_keypair) = gen_account_in("banka");
        let (receiver_id, _) = gen_account_in("acme");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("transfer::domain@merchant.acme".to_string()),
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let transfer = Transfer::domain(
            sender_id.clone(),
            DomainId::try_new("merchant", "acme").expect("domain id"),
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );

        let state = blank_state();
        install_router_nexus(&state, &router);
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn routes_domain_write_to_target_dataspace_without_explicit_rule() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(7);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "acme")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), dataspace_id),
            ]),
        );
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("merchant", "acme").expect("domain id"),
            )))],
        );

        assert_eq!(
            router.try_route(&tx).expect("domain route must resolve"),
            RoutingDecision::new(LaneId::new(2), dataspace_id)
        );
    }

    #[test]
    fn routes_asset_transfer_to_asset_definition_dataspace_without_explicit_rule() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), dataspace_id),
            ]),
        );
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let transfer = Transfer::asset_numeric(
            AssetId::of(asset_definition, sender_id.clone()),
            1_u32,
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("asset transfer route must resolve"),
            RoutingDecision::new(LaneId::new(2), dataspace_id)
        );
    }

    #[test]
    fn opaque_asset_transfer_defers_to_state_for_asset_definition_dataspace() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(2), dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition =
            AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
                .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_numeric(
            AssetId::of(opaque_asset_definition, sender_id.clone()),
            1_u32,
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque asset transfer should defer to state"),
            None
        );

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("opaque asset transfer route must resolve with state"),
            RoutingDecision::new(LaneId::new(2), dataspace_id)
        );
    }

    #[test]
    fn opaque_offline_note_issue_defers_to_state_for_asset_definition_dataspace() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(2), dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let projected_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "unit".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &projected_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let issue = IssueOfflineNote::new(OfflineNoteIssue {
            note_commitment: Hash::new(b"offline-note-route-test"),
            key_certificate: sample_offline_certificate(sender_id.clone()),
            asset: AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            amount: Numeric::new(25, 0),
        });
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(issue)],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition.clone())
                    .with_name("unit".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        bind_asset_definition_alias(&mut state, &opaque_asset_definition, "unit#paynet");

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque offline issue should defer to state"),
            None
        );

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("offline issue route must resolve with state"),
            RoutingDecision::new(LaneId::new(2), dataspace_id)
        );

        let kagemusha = KagemushaTransfer::new(
            opaque_asset_definition,
            vec![[0x11; 32]],
            vec![[0x22; 32]],
            ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
                VerifyingKeyId::new("halo2/ipa", "offline-kagemusha-transfer"),
            ),
            Some([0x33; 32]),
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(kagemusha)],
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque Kagemusha transfer should defer to state"),
            None
        );

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("Kagemusha route must resolve with state"),
            RoutingDecision::new(LaneId::new(2), dataspace_id)
        );
    }

    #[test]
    fn opaque_asset_transfer_uses_stored_asset_alias_dataspace() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(2), dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_numeric(
            AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            1_u32,
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );
        let alias: iroha_data_model::asset::AssetDefinitionAlias =
            "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), opaque_asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            opaque_asset_definition.clone(),
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("opaque asset transfer route must resolve with alias"),
            RoutingDecision::new(LaneId::new(2), dataspace_id)
        );
    }

    #[test]
    fn dataspace_restricted_asset_transfer_ignores_universal_account_fallback_scope() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "sbp")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_numeric(
            AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            1_u32,
            receiver_id.clone(),
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&sender_id),
            ],
            dataspace_catalog.clone(),
            lane_catalog,
        );
        bind_asset_definition_alias(&mut state, &opaque_asset_definition, "pkr#sbp");
        add_unlabeled_account_with_alias(
            &mut state,
            &sender_id,
            account_alias("sender@ubl.sbp", &dataspace_catalog),
        );
        add_unlabeled_account_with_alias(
            &mut state,
            &receiver_id,
            account_alias("receiver@hbl.sbp", &dataspace_catalog),
        );

        let state_view = state.view();
        for account_id in [&sender_id, &receiver_id] {
            assert_eq!(
                state_view
                    .world()
                    .account_scope_hierarchy(account_id)
                    .expect("account hierarchy")
                    .into_keys()
                    .collect::<Vec<_>>(),
                vec![DataSpaceId::UNIVERSAL, dataspace_id]
            );
        }

        assert_eq!(
            router
                .try_route_with_view(&tx, &state_view)
                .expect("dataspace-restricted transfer must route to the asset dataspace"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_definition_registration_routes_by_declared_alias_dataspace() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog,
            lane_catalog,
        );
        let transparent_asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let definition = NewAssetDefinition {
            id: opaque_asset_definition,
            name: "pkr".to_owned(),
            description: None,
            alias: Some(alias),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Register::asset_definition(definition))],
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("declared alias route should not need state"),
            Some(RoutingDecision::new(lane_id, dataspace_id))
        );
    }

    #[test]
    fn asset_definition_registration_opaque_global_without_alias_routes_to_universal() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), dataspace_id),
            ]),
        );
        let transparent_asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let definition = NewAssetDefinition {
            id: opaque_asset_definition,
            name: "pkr".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Register::asset_definition(definition))],
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque global definition without alias should route from policy"),
            Some(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
        );
    }

    #[test]
    fn asset_home_extra_coverage_registration_universal_alias_routes_to_universal() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), dataspace_id),
            ]),
        );
        let transparent_asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let definition = NewAssetDefinition {
            id: opaque_asset_definition,
            name: "pkr".to_owned(),
            description: None,
            alias: Some("pkr#universal".parse().expect("asset alias")),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Register::asset_definition(definition))],
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("universal alias route should not need state"),
            Some(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
        );
    }

    #[test]
    fn asset_home_extra_coverage_opaque_restricted_without_alias_uses_default_route() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(2), dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let definition = NewAssetDefinition {
            id: opaque_asset_definition,
            name: "pkr".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: AssetBalancePolicy::DataspaceRestricted,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Register::asset_definition(definition))],
        );
        let state = state_with_asset_definitions(Vec::new(), dataspace_catalog, lane_catalog);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque restricted definition should defer before fallback"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("opaque restricted definition without home should fall back to default"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_home_more_coverage_alias_home_without_lane_returns_no_lane_error() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::new(4),
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace_id, "paynet")]),
            catalog_with_lane_dataspaces(&[(LaneId::new(4), DataSpaceId::UNIVERSAL)]),
        );
        let definition = NewAssetDefinition {
            id: AssetDefinitionId::new(
                DomainId::try_new("cash", "universal").expect("asset definition domain"),
                "pkr".parse().expect("asset definition name"),
            ),
            name: "pkr".to_owned(),
            description: None,
            alias: Some("pkr#paynet".parse().expect("asset alias")),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Register::asset_definition(definition))],
        );

        let err = router
            .try_route_without_state(&tx)
            .expect_err("alias home without lane should fail");

        assert_eq!(
            err,
            RoutingResolveError::NoLaneForDataspace { dataspace_id }
        );
    }

    #[test]
    fn asset_definition_registration_routes_mixed_declared_alias_dataspaces_to_universal() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let paynet = DataSpaceId::new(10);
        let cbuae = DataSpaceId::new(11);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(paynet, "paynet"), (cbuae, "cbuae")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), paynet),
                (LaneId::new(3), cbuae),
            ]),
        );
        let pkr_id = AssetDefinitionId::parse_address_literal(
            &AssetDefinitionId::new(
                DomainId::try_new("cash", "universal").expect("asset definition domain"),
                "pkr".parse().expect("asset definition name"),
            )
            .canonical_address(),
        )
        .expect("opaque pkr definition id");
        let aed_id = AssetDefinitionId::parse_address_literal(
            &AssetDefinitionId::new(
                DomainId::try_new("cash", "universal").expect("asset definition domain"),
                "aed".parse().expect("asset definition name"),
            )
            .canonical_address(),
        )
        .expect("opaque aed definition id");
        let pkr = NewAssetDefinition {
            id: pkr_id,
            name: "pkr".to_owned(),
            description: None,
            alias: Some("pkr#paynet".parse().expect("asset alias")),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };
        let aed = NewAssetDefinition {
            id: aed_id,
            name: "aed".to_owned(),
            description: None,
            alias: Some("aed#cbuae".parse().expect("asset alias")),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![
                InstructionBox::from(Register::asset_definition(pkr)),
                InstructionBox::from(Register::asset_definition(aed)),
            ],
        );

        let route = router
            .try_route(&tx)
            .expect("mixed declared aliases should route to AMX coordinator");

        assert_eq!(
            route,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn global_asset_transfer_alias_binding_routes_to_universal() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let transfer = Transfer::asset_numeric(
            AssetId::of(asset_definition.clone(), sender_id.clone()),
            1_u32,
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );
        scope_account_to_dataspace(&mut state, &sender_id, dataspace_id);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("stored alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias route must resolve with state"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_definition_permission_grant_uses_stored_alias_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition {
                    asset_definition: asset_definition.clone(),
                },
                bob_id,
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );
        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("asset-definition permission alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias permission route must resolve with state"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_definition_permission_revoke_uses_stored_alias_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Revoke::account_permission(
                iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition {
                    asset_definition: asset_definition.clone(),
                },
                bob_id,
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("asset-definition permission alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias permission revoke route must resolve with state"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn set_asset_definition_balance_policy_uses_stored_alias_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(SetAssetDefinitionBalancePolicy::new(
                asset_definition.clone(),
                AssetBalancePolicy::DataspaceRestricted,
                Some(dataspace_id),
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("balance-policy alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias balance-policy route must resolve with state"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_coverage_mint_global_binding_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("mint alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias mint route must resolve with state"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn global_asset_mint_to_private_scoped_account_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );
        scope_account_to_dataspace(&mut state, &alice_id, dataspace_id);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("mint alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("global asset mint route must ignore destination account scope"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn global_asset_burn_from_private_scoped_account_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Burn::asset_numeric(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        bind_asset_definition_alias(&mut state, &asset_definition, "pkr#paynet");
        scope_account_to_dataspace(&mut state, &alice_id, dataspace_id);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("burn alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("global asset burn route must ignore holder account scope"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn global_asset_mint_with_projected_private_home_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        scope_account_to_dataspace(&mut state, &alice_id, dataspace_id);

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("projected private home must not override a global asset mint route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn global_asset_burn_with_projected_private_home_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Burn::asset_numeric(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        scope_account_to_dataspace(&mut state, &alice_id, dataspace_id);

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("projected private home must not override a global asset burn route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn global_asset_transfer_with_projected_private_home_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Transfer::asset_numeric(
                AssetId::of(asset_definition.clone(), alice_id.clone()),
                1_u32,
                bob_id,
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        scope_account_to_dataspace(&mut state, &alice_id, dataspace_id);

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("projected private home must not override a global asset transfer route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn explicit_dataspace_scoped_mint_routes_to_private_bucket() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "sbp")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let scoped_asset_id = AssetId::with_scope(
            asset_definition.clone(),
            alice_id.clone(),
            AssetBalanceScope::Dataspace(dataspace_id),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                scoped_asset_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("explicit scoped mint without state should defer for definition policy"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("explicit scoped mint must route to the asset balance bucket"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn explicit_dataspace_scoped_transfer_routes_to_private_bucket() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "sbp")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let scoped_asset_id = AssetId::with_scope(
            asset_definition.clone(),
            alice_id.clone(),
            AssetBalanceScope::Dataspace(dataspace_id),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Transfer::asset_numeric(
                scoped_asset_id,
                1_u32,
                bob_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );

        assert_eq!(
            router.try_route_without_state(&tx).expect(
                "explicit scoped transfer without state should defer for definition policy"
            ),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("explicit transfer source scope must route to the asset balance bucket"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn global_asset_explicit_dataspace_scoped_mint_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "sbp")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let scoped_asset_id = AssetId::with_scope(
            asset_definition.clone(),
            alice_id.clone(),
            AssetBalanceScope::Dataspace(dataspace_id),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                scoped_asset_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );

        assert_eq!(
            router.try_route_without_state(&tx).expect(
                "explicit global asset mint without state should defer for definition policy"
            ),
            None
        );
        assert_eq!(
            router.try_route_with_view(&tx, &state.view()).expect(
                "explicit scope must not override a global definition's authoritative route"
            ),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn global_asset_explicit_dataspace_scoped_burn_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "sbp")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let scoped_asset_id = AssetId::with_scope(
            asset_definition.clone(),
            alice_id.clone(),
            AssetBalanceScope::Dataspace(dataspace_id),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Burn::asset_numeric(
                1_u32,
                scoped_asset_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("explicit burn scope must not override global asset route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn global_asset_explicit_dataspace_scoped_transfer_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "sbp")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let scoped_asset_id = AssetId::with_scope(
            asset_definition.clone(),
            alice_id.clone(),
            AssetBalanceScope::Dataspace(dataspace_id),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Transfer::asset_numeric(
                scoped_asset_id,
                1_u32,
                bob_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("explicit transfer source scope must not override global asset route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn native_amx_participants_ignore_private_scopes_for_global_asset_write() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(2), dataspace_id),
        ]);
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                AssetId::with_scope(
                    asset_definition.clone(),
                    alice_id.clone(),
                    AssetBalanceScope::Dataspace(dataspace_id),
                ),
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        bind_asset_definition_alias(&mut state, &asset_definition, "pkr#paynet");
        scope_account_to_dataspace(&mut state, &alice_id, dataspace_id);

        let view = state.view();
        assert_eq!(
            native_amx_participant_dataspaces_with_world(
                &tx,
                &view.nexus().dataspace_catalog,
                view.world()
            ),
            Vec::<DataSpaceId>::new()
        );
    }

    #[test]
    fn native_amx_participants_preserve_restricted_asset_private_scope() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(2), dataspace_id),
        ]);
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                AssetId::with_scope(
                    asset_definition.clone(),
                    alice_id.clone(),
                    AssetBalanceScope::Dataspace(dataspace_id),
                ),
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        bind_asset_definition_alias(&mut state, &asset_definition, "pkr#paynet");
        scope_account_to_dataspace(&mut state, &alice_id, dataspace_id);

        let view = state.view();
        assert_eq!(
            native_amx_participant_dataspaces_with_world(
                &tx,
                &view.nexus().dataspace_catalog,
                view.world()
            ),
            vec![dataspace_id]
        );
    }

    #[test]
    fn asset_home_coverage_burn_global_binding_routes_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Burn::asset_numeric(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("burn alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias burn route must resolve with state"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_home_coverage_modify_asset_metadata_permission_uses_stored_alias_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::asset::CanModifyAssetMetadataWithDefinition {
                    asset_definition: asset_definition.clone(),
                },
                bob_id,
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("asset metadata permission alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias metadata permission route must resolve with state"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_coverage_unregister_asset_definition_permission_uses_stored_alias_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::asset_definition::CanUnregisterAssetDefinition {
                    asset_definition: asset_definition.clone(),
                },
                bob_id,
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("unregister permission alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias unregister permission route must resolve with state"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_extra_coverage_mint_permission_uses_stored_alias_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::asset::CanMintAssetWithDefinition {
                    asset_definition: asset_definition.clone(),
                },
                bob_id,
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("mint permission alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias mint permission route must resolve with state"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_extra_coverage_burn_permission_uses_stored_alias_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::asset::CanBurnAssetWithDefinition {
                    asset_definition: asset_definition.clone(),
                },
                bob_id,
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("burn permission alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias burn permission route must resolve with state"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_more_coverage_modify_definition_metadata_permission_uses_stored_alias_dataspace()
    {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::asset_definition::CanModifyAssetDefinitionMetadata {
                    asset_definition: asset_definition.clone(),
                },
                bob_id,
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("definition metadata permission alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias definition metadata permission route must resolve"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn asset_home_more_coverage_revoke_modify_definition_metadata_permission_uses_stored_alias() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Revoke::account_permission(
                iroha_executor_data_model::permission::asset_definition::CanModifyAssetDefinitionMetadata {
                    asset_definition: asset_definition.clone(),
                },
                bob_id,
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("definition metadata revoke alias lookup should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("stored alias definition metadata revoke route must resolve"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn known_opaque_global_asset_without_home_alias_routes_to_universal() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "xor".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_numeric(
            AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            1_u32,
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition)
                    .with_name("xor".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .account_scope_directory
            .insert(sender_id.clone(), scope_entry);

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("known global asset route must resolve"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn known_opaque_global_asset_mint_without_home_alias_routes_to_universal() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "xor".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition)
                    .with_name("xor".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .account_scope_directory
            .insert(sender_id.clone(), scope_entry);

        assert_eq!(
            router
                .try_route_with_state(&tx, &state)
                .expect("known opaque global mint must use state-aware routing"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn known_opaque_global_asset_mint_with_stored_private_home_alias_routes_to_universal() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "xor".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            ))],
        );
        let alias: AssetDefinitionAlias = "xor#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition.clone())
                    .with_name("xor".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), opaque_asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            opaque_asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("known opaque global mint should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_state(&tx, &state)
                .expect("global mint must ignore the stored private home alias"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn known_opaque_global_asset_mint_ignores_authority_account_rule_override() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: lane_id,
                    dataspace: Some(dataspace_id),
                    matcher: LaneRoutingMatcher {
                        account: Some(sender_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                }],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "xor".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition)
                    .with_name("xor".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .account_scope_directory
            .insert(sender_id.clone(), scope_entry);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("known opaque global mint should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_state(&tx, &state)
                .expect("global mint must not route to the authority account dataspace"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("global mint plan must keep the universal coordinator")
                .coordinator_route(),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn known_opaque_global_asset_transfer_ignores_authority_account_rule_override() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: lane_id,
                    dataspace: Some(dataspace_id),
                    matcher: LaneRoutingMatcher {
                        account: Some(sender_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                }],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "xor".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Transfer::asset_numeric(
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
                1_u32,
                receiver_id,
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition)
                    .with_name("xor".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .account_scope_directory
            .insert(sender_id.clone(), scope_entry);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("known opaque global transfer should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_state(&tx, &state)
                .expect("global transfer must not route to the authority account dataspace"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("global transfer plan must keep the universal coordinator")
                .coordinator_route(),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn known_opaque_global_asset_burn_ignores_authority_account_rule_override() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: lane_id,
                    dataspace: Some(dataspace_id),
                    matcher: LaneRoutingMatcher {
                        account: Some(sender_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                }],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "xor".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(Burn::asset_numeric(
                1_u32,
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition)
                    .with_name("xor".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .account_scope_directory
            .insert(sender_id.clone(), scope_entry);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("known opaque global burn should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_state(&tx, &state)
                .expect("global burn must not route to the authority account dataspace"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("global burn plan must keep the universal coordinator")
                .coordinator_route(),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn opaque_asset_transfer_uses_single_lane_legacy_dataspace_fallback() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog =
            catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_numeric(
            AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            1_u32,
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );
        let alias: iroha_data_model::asset::AssetDefinitionAlias =
            "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&sender_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), opaque_asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            opaque_asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("legacy single-lane route must preserve dataspace target"),
            RoutingDecision::new(LaneId::SINGLE, dataspace_id)
        );
    }

    #[test]
    fn opaque_asset_transfer_routes_to_sender_single_scope_when_asset_definition_unresolved() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_numeric(
            AssetId::of(opaque_asset_definition, sender_id.clone()),
            1_u32,
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );

        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(
            &[(sender_id.clone(), scope_entry)],
            dataspace_catalog,
        );
        state.nexus.write().lane_catalog = lane_catalog;

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque asset transfer should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("opaque asset transfer should fall back to sender account scope"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn opaque_asset_transfer_with_universal_and_private_account_scope_uses_default_route() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let uaid = UniversalAccountId::from_hash(Hash::new(b"router::uaid-bound-sender"));
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let dataspace_catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_numeric(
            AssetId::of(opaque_asset_definition, sender_id.clone()),
            1_u32,
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );

        let mut state = blank_state();
        state.nexus.write().dataspace_catalog = dataspace_catalog;
        state.nexus.write().lane_catalog = lane_catalog;
        let sender = Account::new(sender_id.clone())
            .with_uaid(Some(uaid))
            .build(&sender_id);
        let (account_id, account_value) = sender.into_key_value();
        state
            .world
            .accounts
            .insert(account_id.clone(), account_value);
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(DataSpaceId::UNIVERSAL);
        scope_entry.ensure_dataspace(dataspace_id);
        state
            .world
            .account_scope_directory
            .insert(account_id.clone(), scope_entry);
        let mut bindings = crate::nexus::space_directory::UaidDataspaceBindings::default();
        bindings.bind_account(dataspace_id, account_id);
        state.world.uaid_dataspaces.insert(uaid, bindings);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque asset transfer should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("ambiguous account scope should use the default route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn opaque_asset_transfer_with_multiple_private_account_bindings_uses_default_route() {
        let (sender_id, sender_keypair) = gen_account_in("wonderland");
        let (receiver_id, _) = gen_account_in("wonderland");
        let uaid = UniversalAccountId::from_hash(Hash::new(b"router::multi-private-uaid"));
        let first_dataspace = DataSpaceId::new(10);
        let second_dataspace = DataSpaceId::new(11);
        let dataspace_catalog =
            dataspace_catalog(&[(first_dataspace, "paynet"), (second_dataspace, "bankb")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(2), first_dataspace),
            (LaneId::new(3), second_dataspace),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let transparent_asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("cash", "paynet").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_numeric(
            AssetId::of(opaque_asset_definition, sender_id.clone()),
            1_u32,
            receiver_id,
        );
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );

        let mut state = blank_state();
        state.nexus.write().dataspace_catalog = dataspace_catalog;
        state.nexus.write().lane_catalog = lane_catalog;
        let sender = Account::new(sender_id.clone())
            .with_uaid(Some(uaid))
            .build(&sender_id);
        let (account_id, account_value) = sender.into_key_value();
        state
            .world
            .accounts
            .insert(account_id.clone(), account_value);
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(first_dataspace);
        scope_entry.ensure_dataspace(second_dataspace);
        state
            .world
            .account_scope_directory
            .insert(account_id.clone(), scope_entry);
        let mut bindings = crate::nexus::space_directory::UaidDataspaceBindings::default();
        bindings.bind_account(first_dataspace, account_id.clone());
        bindings.bind_account(second_dataspace, account_id.clone());
        state.world.uaid_dataspaces.insert(uaid, bindings);

        assert_eq!(
            state.view().world().dataspace_for_account(&account_id),
            Some(first_dataspace)
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("multi-dataspace account should use the default route"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn explicit_lane_rule_infers_target_dataspace_for_domain_write() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(7);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: LaneId::new(3),
                    dataspace: None,
                    matcher: LaneRoutingMatcher {
                        account: Some(authority_id.to_string()),
                        instruction: Some("register::domain".to_string()),
                        description: None,
                    },
                }],
            },
            dataspace_catalog(&[(dataspace_id, "acme")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(3), dataspace_id),
            ]),
        );
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("merchant", "acme").expect("domain id"),
            )))],
        );

        assert_eq!(
            router.try_route(&tx).expect("domain route must resolve"),
            RoutingDecision::new(LaneId::new(3), dataspace_id)
        );
    }

    #[test]
    fn mixed_domain_write_targets_across_dataspaces_build_native_amx_plan() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(first_dataspace, "acme"), (second_dataspace, "bank")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "acme").expect("domain id"),
                ))),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("treasury", "bank").expect("domain id"),
                ))),
            ],
        );

        let plan = router
            .try_route_plan(&tx)
            .expect("mixed domain writes should build a native AMX plan");
        let RoutingPlan::NativeAmx(plan) = plan else {
            panic!("mixed domain writes should not collapse to a single route");
        };
        assert_eq!(
            plan.coordinator.route,
            RoutingDecision::new(LaneId::new(2), first_dataspace)
        );
        assert_eq!(
            plan.participants,
            vec![
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(2), first_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(3), second_dataspace),
                    RouteLegRole::Participant,
                ),
            ]
        );
    }

    #[test]
    fn mixed_domain_write_targets_keep_object_dataspaces_over_rule_dataspace() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: Some(first_dataspace),
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("register::domain".to_owned()),
                        description: None,
                    },
                }],
            },
            dataspace_catalog(&[(first_dataspace, "acme"), (second_dataspace, "bank")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "acme").expect("domain id"),
                ))),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("treasury", "bank").expect("domain id"),
                ))),
            ],
        );

        let plan = router
            .try_route_plan(&tx)
            .expect("matched rules must not override AMX participant dataspaces");
        let RoutingPlan::NativeAmx(plan) = plan else {
            panic!("mixed domain writes should build a native AMX plan");
        };
        assert_eq!(
            plan.participants
                .iter()
                .map(|leg| leg.route.dataspace_id)
                .collect::<Vec<_>>(),
            vec![first_dataspace, second_dataspace]
        );
    }

    #[test]
    fn strict_amx_policy_rejects_mixed_domain_write_targets() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(first_dataspace, "acme"), (second_dataspace, "bank")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let mut metadata = Metadata::default();
        metadata.insert(
            AMX_POLICY_METADATA_KEY.parse().expect("amx policy key"),
            iroha_primitives::json::Json::new(AMX_POLICY_REJECT_CROSS_DATASPACE),
        );
        let tx = sample_transaction_with_metadata(
            &authority_id,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "acme").expect("domain id"),
                ))),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("treasury", "bank").expect("domain id"),
                ))),
            ],
            metadata,
        );

        assert_eq!(
            router.try_route(&tx),
            Err(
                RoutingResolveError::ConflictingTransactionDataspaceTargets {
                    first_dataspace_id: first_dataspace,
                    second_dataspace_id: second_dataspace,
                }
            )
        );
    }

    #[test]
    fn strict_amx_policy_rejects_mixed_proved_overlay_write_targets() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(first_dataspace, "acme"), (second_dataspace, "bank")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let mut metadata = Metadata::default();
        metadata.insert(
            AMX_POLICY_METADATA_KEY.parse().expect("amx policy key"),
            iroha_primitives::json::Json::new(AMX_POLICY_REJECT_CROSS_DATASPACE),
        );
        let tx = sample_executable_transaction_with_metadata(
            &authority_id,
            authority_keypair.private_key(),
            sample_proved_executable(vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "acme").expect("domain id"),
                ))),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("treasury", "bank").expect("domain id"),
                ))),
            ]),
            metadata,
        );

        assert_eq!(
            router.try_route(&tx),
            Err(
                RoutingResolveError::ConflictingTransactionDataspaceTargets {
                    first_dataspace_id: first_dataspace,
                    second_dataspace_id: second_dataspace,
                }
            )
        );
    }

    #[test]
    fn strict_amx_policy_value_is_trimmed_and_case_insensitive() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(first_dataspace, "acme"), (second_dataspace, "bank")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let mut metadata = Metadata::default();
        metadata.insert(
            AMX_POLICY_METADATA_KEY.parse().expect("amx policy key"),
            iroha_primitives::json::Json::new("  ReJeCt_CrOsS_DaTaSpAcE  "),
        );
        let tx = sample_transaction_with_metadata(
            &authority_id,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "acme").expect("domain id"),
                ))),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("treasury", "bank").expect("domain id"),
                ))),
            ],
            metadata,
        );

        assert_eq!(
            router.try_route(&tx),
            Err(
                RoutingResolveError::ConflictingTransactionDataspaceTargets {
                    first_dataspace_id: first_dataspace,
                    second_dataspace_id: second_dataspace,
                }
            )
        );
    }

    #[test]
    fn strict_amx_policy_rejects_mixed_dataspace_scoped_permissions() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(first_dataspace, "acme"), (second_dataspace, "bank")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let mut metadata = Metadata::default();
        metadata.insert(
            AMX_POLICY_METADATA_KEY.parse().expect("amx policy key"),
            iroha_primitives::json::Json::new(AMX_POLICY_REJECT_CROSS_DATASPACE),
        );
        let first_permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: first_dataspace,
        }
        .into();
        let second_permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: second_dataspace,
        }
        .into();
        let tx = sample_transaction_with_metadata(
            &authority_id,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Grant::account_permission(
                    first_permission,
                    authority_id.clone(),
                )),
                InstructionBox::from(Revoke::account_permission(
                    second_permission,
                    authority_id.clone(),
                )),
            ],
            metadata,
        );

        assert_eq!(
            router.try_route(&tx),
            Err(RoutingResolveError::ConflictingDataspaceScopedPermissions {
                first_dataspace_id: first_dataspace,
                second_dataspace_id: second_dataspace,
            })
        );
    }

    #[test]
    fn mixed_dataspace_scoped_permissions_without_universal_lane_fail_closed() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::new(2),
                default_dataspace: first_dataspace,
                rules: vec![],
            },
            dataspace_catalog(&[(first_dataspace, "acme"), (second_dataspace, "bank")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let first_permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: first_dataspace,
        }
        .into();
        let second_permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: second_dataspace,
        }
        .into();
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Grant::account_permission(
                    first_permission,
                    authority_id.clone(),
                )),
                InstructionBox::from(Revoke::account_permission(
                    second_permission,
                    authority_id.clone(),
                )),
            ],
        );

        assert_eq!(
            router.try_route(&tx),
            Err(RoutingResolveError::NoLaneForDataspace {
                dataspace_id: DataSpaceId::UNIVERSAL,
            })
        );
    }

    #[test]
    fn mixed_domain_write_targets_do_not_require_universal_lane() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::new(2),
                default_dataspace: first_dataspace,
                rules: vec![],
            },
            dataspace_catalog(&[(first_dataspace, "acme"), (second_dataspace, "bank")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::new(2), first_dataspace),
                (LaneId::new(3), second_dataspace),
            ]),
        );
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "acme").expect("domain id"),
                ))),
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("treasury", "bank").expect("domain id"),
                ))),
            ],
        );

        let plan = router
            .try_route_plan(&tx)
            .expect("native AMX should coordinate on a participant route");
        let RoutingPlan::NativeAmx(plan) = plan else {
            panic!("mixed domain writes should build a native AMX plan");
        };
        assert_eq!(
            plan.coordinator.route,
            RoutingDecision::new(LaneId::new(2), first_dataspace)
        );
        assert_eq!(plan.participants.len(), 2);
    }

    #[test]
    fn account_rule_takes_precedence_over_transfer_destination_rule() {
        let (uae_sender_id, uae_sender_keypair) = gen_account_in("uae");
        let (bank_sender_id, bank_sender_keypair) = gen_account_in("banka");
        let (acme_receiver_id, _) = gen_account_in("acme");

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![
                LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: None,
                    matcher: LaneRoutingMatcher {
                        account: Some("*@uae.universal".to_string()),
                        instruction: Some("transfer".to_string()),
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: LaneId::new(1),
                    dataspace: None,
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("transfer::asset@acme.universal".to_string()),
                        description: None,
                    },
                },
            ],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1), LaneId::new(2)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);

        let asset_definition: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("uae", "universal").unwrap(),
            "aed".parse().unwrap(),
        );
        let uae_transfer = Transfer::asset_numeric(
            AssetId::of(asset_definition.clone(), uae_sender_id.clone()),
            1_u32,
            acme_receiver_id.clone(),
        );
        let bank_transfer = Transfer::asset_numeric(
            AssetId::of(asset_definition, bank_sender_id.clone()),
            1_u32,
            acme_receiver_id.clone(),
        );

        let uae_tx = sample_transaction(
            &uae_sender_id,
            uae_sender_keypair.private_key(),
            vec![InstructionBox::from(uae_transfer)],
        );
        let bank_tx = sample_transaction(
            &bank_sender_id,
            bank_sender_keypair.private_key(),
            vec![InstructionBox::from(bank_transfer)],
        );

        let catalog = DataSpaceCatalog::default();
        let state = state_with_account_aliases(
            &[
                (
                    uae_sender_id.clone(),
                    account_alias("central@uae.universal", &catalog),
                ),
                (
                    bank_sender_id.clone(),
                    account_alias("settler@banka.universal", &catalog),
                ),
                (
                    acme_receiver_id.clone(),
                    account_alias("merchant@acme.universal", &catalog),
                ),
            ],
            catalog,
        );
        install_router_nexus(&state, &router);
        let uae_decision = router.route_with_view(&uae_tx, &state.view());
        let bank_decision = router.route_with_view(&bank_tx, &state.view());
        assert_eq!(uae_decision.lane_id, LaneId::new(2));
        assert_eq!(bank_decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn matches_dataspace_root_account_alias_scope_rule() {
        let (dataspace_id, dataspace_keypair) = gen_account_in("wonderland");
        let (domain_id, domain_keypair) = gen_account_in("wonderland");
        let catalog = dataspace_catalog(&[(DataSpaceId::new(10), "paynet")]);

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(10)),
                matcher: LaneRoutingMatcher {
                    account: Some("*@paynet".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(1), DataSpaceId::new(10)),
        ]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog);

        let dataspace_tx = sample_transaction(
            &dataspace_id,
            dataspace_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("paynet-match", "universal").expect("domain id"),
            )))],
        );
        let domain_tx = sample_transaction(
            &domain_id,
            domain_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("banka-no-match", "universal").expect("domain id"),
            )))],
        );

        let state = state_with_account_aliases(
            &[
                (
                    dataspace_id.clone(),
                    account_alias("issuer@paynet", &catalog),
                ),
                (
                    domain_id.clone(),
                    account_alias("operator@banka.paynet", &catalog),
                ),
            ],
            catalog,
        );
        install_router_nexus(&state, &router);

        assert_eq!(
            router.route_with_view(&dataspace_tx, &state.view()),
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(10))
        );
        assert_eq!(
            router.route_with_view(&domain_tx, &state.view()),
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(10))
        );
    }

    #[test]
    fn try_route_with_view_resolves_against_same_state_catalog_snapshot() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let state_lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: lane_id,
                dataspace: Some(dataspace_id),
                matcher: LaneRoutingMatcher {
                    account: Some("*@paynet".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let stale_router_lane_catalog =
            catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
        let router = ConfigLaneRouter::new(
            policy,
            DataSpaceCatalog::default(),
            stale_router_lane_catalog,
        );
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("state-catalog-route", "universal").expect("domain id"),
            )))],
        );
        let state = state_with_account_aliases(
            &[(
                authority_id.clone(),
                account_alias("operator@paynet", &catalog),
            )],
            catalog,
        );
        {
            let mut nexus = state.nexus.write();
            nexus.routing_policy = router.policy.as_ref().clone();
            nexus.lane_catalog = state_lane_catalog;
        }

        let decision = router
            .try_route_with_view(&tx, &state.view())
            .expect("state-aware routing must resolve against the same state catalogs it matched");

        assert_eq!(decision, RoutingDecision::new(lane_id, dataspace_id));
    }

    #[test]
    fn legacy_bare_domain_account_scope_does_not_match() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let catalog = dataspace_catalog(&[(DataSpaceId::new(10), "paynet")]);

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(10)),
                matcher: LaneRoutingMatcher {
                    account: Some("*@banka".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(1), DataSpaceId::new(10)),
        ]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog);

        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("legacy-no-match", "universal").expect("domain id"),
            )))],
        );
        let state = state_with_account_aliases(
            &[(
                authority_id.clone(),
                account_alias("operator@banka.paynet", &catalog),
            )],
            catalog,
        );

        assert_eq!(
            router.route_with_view(&tx, &state.view()),
            RoutingDecision::default()
        );
    }

    #[test]
    fn resolve_query_routing_decision_matches_authority_rule() {
        let (alice_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(0),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(2),
                dataspace: Some(DataSpaceId::new(2)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::new(0), DataSpaceId::UNIVERSAL),
            (LaneId::new(2), DataSpaceId::new(2)),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::UNIVERSAL,
                alias: "universal".to_owned(),
                ..Default::default()
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(2),
                alias: "ds2".to_owned(),
                ..Default::default()
            },
        ])
        .expect("dataspace catalog");

        let decision = resolve_query_routing_decision(
            &policy,
            &lane_catalog,
            &dataspace_catalog,
            &alice_id,
            None,
        )
        .expect("query route must resolve");

        assert_eq!(
            decision,
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2))
        );
    }

    #[test]
    fn resolve_query_routing_decision_ignores_instruction_matchers() {
        let (alice_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(0),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(1)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: Some("mint".to_owned()),
                    description: None,
                },
            }],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::new(0), DataSpaceId::UNIVERSAL),
            (LaneId::new(1), DataSpaceId::new(1)),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::UNIVERSAL,
                alias: "universal".to_owned(),
                ..Default::default()
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(1),
                alias: "ds1".to_owned(),
                ..Default::default()
            },
        ])
        .expect("dataspace catalog");

        let decision = resolve_query_routing_decision(
            &policy,
            &lane_catalog,
            &dataspace_catalog,
            &alice_id,
            None,
        )
        .expect("query route must resolve");

        assert_eq!(
            decision,
            RoutingDecision::new(LaneId::new(0), DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn dataspace_scoped_permission_grant_routes_by_permission_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace = DataSpaceId::new(7);
        let lane = LaneId::new(3);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(1)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane, dataspace),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: dataspace,
                alias: "manifest".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let router = ConfigLaneRouter::new(policy, dataspace_catalog, lane_catalog);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                CanPublishSpaceDirectoryManifest { dataspace },
                alice_id.clone(),
            ))],
        );

        let decision = router
            .try_route(&tx)
            .expect("dataspace-scoped permission should resolve");

        assert_eq!(decision, RoutingDecision::new(lane, dataspace));
    }

    #[test]
    fn account_permission_grant_routes_by_destination_account_policy() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![
                LaneRoutingRule {
                    lane: LaneId::new(1),
                    dataspace: Some(DataSpaceId::new(1)),
                    matcher: LaneRoutingMatcher {
                        account: Some(alice_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: Some(DataSpaceId::new(2)),
                    matcher: LaneRoutingMatcher {
                        account: Some(bob_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
            ],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(1), DataSpaceId::new(1)),
            (LaneId::new(2), DataSpaceId::new(2)),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(1),
                alias: "alice".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(2),
                alias: "bob".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let router = ConfigLaneRouter::new(policy, dataspace_catalog, lane_catalog);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
                    account: alice_id.clone(),
                },
                bob_id.clone(),
            ))],
        );

        let decision = router
            .try_route(&tx)
            .expect("account permission should route to destination account lane");

        assert_eq!(
            decision,
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2))
        );
    }

    #[test]
    fn asset_definition_permission_grant_routes_by_asset_definition_dataspace_policy() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![
                LaneRoutingRule {
                    lane: LaneId::new(1),
                    dataspace: Some(DataSpaceId::new(1)),
                    matcher: LaneRoutingMatcher {
                        account: Some(alice_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: Some(DataSpaceId::new(2)),
                    matcher: LaneRoutingMatcher {
                        account: Some(bob_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
            ],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(1), DataSpaceId::new(1)),
            (LaneId::new(2), DataSpaceId::new(2)),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(1),
                alias: "alice".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(2),
                alias: "bob".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let router = ConfigLaneRouter::new(policy, dataspace_catalog, lane_catalog);
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("nexus", "universal").unwrap(),
            "ds1".parse().unwrap(),
        );
        let opaque_asset_definition =
            AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
                .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition {
                    asset_definition: opaque_asset_definition,
                },
                bob_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("ds1".to_owned())
                    .build(&alice_id),
            ],
            router.dataspace_catalog.as_ref().clone(),
            router.lane_catalog.as_ref().clone(),
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque asset-definition permission should defer to state"),
            None
        );

        let decision = router
            .try_route_with_view(&tx, &state.view())
            .expect("asset-definition permission should route to the asset-definition dataspace");

        assert_eq!(
            decision,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_definition_permission_revoke_routes_by_asset_definition_dataspace_policy() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![
                LaneRoutingRule {
                    lane: LaneId::new(1),
                    dataspace: Some(DataSpaceId::new(1)),
                    matcher: LaneRoutingMatcher {
                        account: Some(alice_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: Some(DataSpaceId::new(2)),
                    matcher: LaneRoutingMatcher {
                        account: Some(bob_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
            ],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(1), DataSpaceId::new(1)),
            (LaneId::new(2), DataSpaceId::new(2)),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(1),
                alias: "alice".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(2),
                alias: "bob".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let router = ConfigLaneRouter::new(policy, dataspace_catalog, lane_catalog);
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("nexus", "universal").unwrap(),
            "ds1".parse().unwrap(),
        );
        let opaque_asset_definition =
            AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
                .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Revoke::account_permission(
                iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition {
                    asset_definition: opaque_asset_definition,
                },
                bob_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("ds1".to_owned())
                    .build(&alice_id),
            ],
            router.dataspace_catalog.as_ref().clone(),
            router.lane_catalog.as_ref().clone(),
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque asset-definition revoke should defer to state"),
            None
        );

        let decision = router.try_route_with_view(&tx, &state.view()).expect(
            "asset-definition permission revoke should route to the asset-definition dataspace",
        );

        assert_eq!(
            decision,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn asset_definition_permission_grant_routes_by_named_dataspace_alias() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (bob_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![
                LaneRoutingRule {
                    lane: LaneId::new(1),
                    dataspace: Some(DataSpaceId::new(1)),
                    matcher: LaneRoutingMatcher {
                        account: Some(alice_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: Some(DataSpaceId::new(2)),
                    matcher: LaneRoutingMatcher {
                        account: Some(bob_id.to_string()),
                        instruction: None,
                        description: None,
                    },
                },
            ],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(1), DataSpaceId::new(1)),
            (LaneId::new(2), DataSpaceId::new(2)),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(1),
                alias: "alice".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(2),
                alias: "bob".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let router = ConfigLaneRouter::new(policy, dataspace_catalog, lane_catalog);
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("vault", "bob").unwrap(),
            "voucher".parse().unwrap(),
        );
        let opaque_asset_definition =
            AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
                .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition {
                    asset_definition: opaque_asset_definition,
                },
                alice_id.clone(),
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("voucher".to_owned())
                    .build(&bob_id),
            ],
            router.dataspace_catalog.as_ref().clone(),
            router.lane_catalog.as_ref().clone(),
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque named-dataspace permission should defer to state"),
            None
        );

        let decision = router
            .try_route_with_view(&tx, &state.view())
            .expect("named-dataspace asset permission should route to that dataspace");

        assert_eq!(
            decision,
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2))
        );
    }

    #[test]
    fn dataspace_scoped_permission_grant_routes_mixed_dataspaces_to_universal() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(7);
        let second_dataspace = DataSpaceId::new(8);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(3), first_dataspace),
            (LaneId::new(4), second_dataspace),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: first_dataspace,
                alias: "first".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: second_dataspace,
                alias: "second".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let router = ConfigLaneRouter::new(policy, dataspace_catalog, lane_catalog);
        let first_permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: first_dataspace,
        }
        .into();
        let second_permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: second_dataspace,
        }
        .into();
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![
                InstructionBox::from(Grant::account_permission(
                    first_permission.clone(),
                    alice_id.clone(),
                )),
                InstructionBox::from(Revoke::account_permission(
                    second_permission,
                    alice_id.clone(),
                )),
            ],
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("mixed dataspace-scoped permissions should route to AMX coordinator"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn account_alias_dataspace_permission_grant_routes_by_scope() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (holder_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(3);
        let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog,
            lane_catalog,
        );
        let permission = Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(dataspace_id),
        });
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                permission, holder_id,
            ))],
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("dataspace alias permission should route without world state"),
            Some(RoutingDecision::new(lane_id, dataspace_id))
        );
    }

    #[test]
    fn account_alias_domain_permission_grant_routes_by_scope() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (holder_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(3);
        let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog,
            lane_catalog,
        );
        let permission = Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Domain(
                DomainId::try_new("mibank", "paynet").expect("domain id"),
            ),
        });
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                permission, holder_id,
            ))],
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("domain alias permission should route without world state"),
            Some(RoutingDecision::new(lane_id, dataspace_id))
        );
    }

    #[test]
    fn account_scope_directory_scope_matches_destination_account_permission_route() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (holder_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(dataspace_id),
                matcher: LaneRoutingMatcher {
                    account: Some("*@hbl.paynet".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(1), dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog);

        let permission = Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(
                DomainId::try_new("hbl", "paynet").expect("domain id"),
            ),
        });
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                permission,
                holder_id.clone(),
            ))],
        );

        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        scope_entry.bind_domain(
            dataspace_id,
            AccountAliasDomain::from("hbl".parse::<Name>().expect("domain label")),
        );
        let state = state_with_account_scope_entries(&[(holder_id.clone(), scope_entry)], catalog);
        state.nexus.write().lane_catalog = router.lane_catalog.as_ref().clone();
        let state_view = state.view();
        assert_eq!(
            state_view
                .world()
                .account_scope_hierarchy(&holder_id)
                .expect("scope hierarchy"),
            BTreeMap::from([(
                dataspace_id,
                BTreeSet::from([DomainId::try_new("hbl", "paynet").expect("domain id")]),
            )])
        );
        assert!(account_matches_alias_scope(
            "hbl.paynet",
            &holder_id,
            &state_view
        ));

        assert_eq!(
            router.route_with_view(&tx, &state_view),
            RoutingDecision::new(LaneId::new(1), dataspace_id)
        );
    }

    #[test]
    fn world_validation_routes_account_permission_holder_by_scope() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (holder_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(3);
        let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: lane_id,
                dataspace: Some(dataspace_id),
                matcher: LaneRoutingMatcher {
                    account: Some("*@paynet".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(policy.clone(), catalog.clone(), lane_catalog.clone());
        let permission = Permission::from(CanRegisterTrigger {
            authority: holder_id.clone(),
        });
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                permission,
                holder_id.clone(),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(&[(holder_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog.clone();
        let state_view = state.view();
        let expected = RoutingDecision::new(lane_id, dataspace_id);

        assert_eq!(
            router
                .try_route_with_view(&tx, &state_view)
                .expect("state-view routing should use account scope"),
            expected
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                &policy,
                &lane_catalog,
                &state_view.nexus().dataspace_catalog,
                &tx,
                state_view.world(),
            )
            .expect("validation routing should use account scope"),
            expected
        );
    }

    #[test]
    fn state_view_routing_uses_committed_nexus_policy_not_cached_router_policy() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (holder_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(3);
        let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let committed_policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: lane_id,
                dataspace: Some(dataspace_id),
                matcher: LaneRoutingMatcher {
                    account: Some("*@paynet".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy::default(),
            DataSpaceCatalog::default(),
            LaneCatalog::default(),
        );
        let permission = Permission::from(CanRegisterTrigger {
            authority: holder_id.clone(),
        });
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                permission,
                holder_id.clone(),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(&[(holder_id, scope_entry)], catalog);
        {
            let mut nexus = state.nexus.write();
            nexus.routing_policy = committed_policy;
            nexus.lane_catalog = lane_catalog;
        }
        let state_view = state.view();

        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state_view)
                .expect("state-view routing should use committed nexus policy")
                .coordinator_route(),
            RoutingDecision::new(lane_id, dataspace_id)
        );
        assert_eq!(
            router
                .try_route_plan_without_state(&tx)
                .expect("permission grant without a cached rule should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_plan_with_state(&tx, &state)
                .expect("state routing should use committed nexus policy")
                .coordinator_route(),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn fee_sponsor_account_permission_grant_routes_to_holder_single_scope() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (holder_id, _) = gen_account_in("wonderland");
        let (sponsor_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(3);
        let catalog = dataspace_catalog(&[(dataspace_id, "bpng")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let router = ConfigLaneRouter::new(policy.clone(), catalog.clone(), lane_catalog.clone());
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                CanUseFeeSponsor {
                    sponsor: sponsor_id,
                },
                holder_id.clone(),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(&[(holder_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog.clone();
        let state_view = state.view();
        let expected = RoutingDecision::new(lane_id, dataspace_id);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("fee sponsor grants should defer without account scope state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state_view)
                .expect("state-view routing should use holder account scope"),
            expected
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                &policy,
                &lane_catalog,
                &state_view.nexus().dataspace_catalog,
                &tx,
                state_view.world(),
            )
            .expect("validation routing should use holder account scope"),
            expected
        );
    }

    #[test]
    fn account_permission_query_ignores_instruction_rule_but_uses_state_scope_fallback() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (holder_id, _) = gen_account_in("wonderland");
        let (sponsor_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(3);
        let catalog = dataspace_catalog(&[(dataspace_id, "bpng")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(1), DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("grant::permission".to_string()),
                    description: None,
                },
            }],
        };
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog.clone());
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                CanUseFeeSponsor {
                    sponsor: sponsor_id,
                },
                holder_id.clone(),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(&[(holder_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog;
        let state_view = state.view();

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("instruction-only query route should defer without account scope state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state_view)
                .expect("instruction matcher should be ignored for account-permission query route"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn account_metadata_write_routes_to_single_scope_dataspace_with_state() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog.clone(),
            lane_catalog,
        );
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(RemoveKeyValue::account(
                target_id.clone(),
                "routing".parse().expect("metadata key"),
            ))],
        );

        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(&[(target_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = router.lane_catalog.as_ref().clone();

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("account metadata writes should defer until account scope is loaded"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("single-scope account metadata writes should route to that dataspace"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn register_account_with_dataspace_label_routes_without_state() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog.clone(),
            lane_catalog,
        );
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Register::account(
                Account::new(target_id)
                    .with_label(Some(account_alias("merchant@restricted", &catalog))),
            ))],
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("account registration with a dataspace label should route without state"),
            Some(RoutingDecision::new(lane_id, dataspace_id))
        );
    }

    #[test]
    fn account_metadata_write_with_multiple_scopes_falls_back_to_default_route() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let first_dataspace = DataSpaceId::new(1);
        let second_dataspace = DataSpaceId::new(10);
        let catalog = dataspace_catalog(&[
            (first_dataspace, "governance"),
            (second_dataspace, "restricted"),
        ]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(1), first_dataspace),
            (LaneId::new(2), second_dataspace),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog.clone(),
            lane_catalog,
        );
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(RemoveKeyValue::account(
                target_id.clone(),
                "routing".parse().expect("metadata key"),
            ))],
        );

        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(first_dataspace);
        scope_entry.ensure_dataspace(second_dataspace);
        let state = state_with_account_scope_entries(&[(target_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = router.lane_catalog.as_ref().clone();

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("multi-scope account metadata writes should defer until scope is loaded"),
            None
        );
        assert_eq!(
            router.try_route_with_view(&tx, &state.view()).expect(
                "multi-scope account metadata writes should fall back to the default route"
            ),
            RoutingDecision::default()
        );
    }

    #[test]
    fn opaque_asset_definition_unregister_routes_to_resolved_target_dataspace() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog.clone(),
            lane_catalog,
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("vault", "restricted").expect("domain id"),
            "voucher".parse().expect("asset definition name"),
        );
        let opaque_asset_definition =
            AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
                .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Unregister::asset_definition(
                opaque_asset_definition,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("voucher".to_owned())
                    .build(&submitter_id),
            ],
            catalog,
            router.lane_catalog.as_ref().clone(),
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque asset-definition unregisters should defer to state"),
            None
        );
        assert_eq!(
            router.try_route_with_view(&tx, &state.view()).expect(
                "opaque asset-definition unregister should route to the resolved dataspace"
            ),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn opaque_asset_definition_metadata_set_routes_to_resolved_target_dataspace() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog.clone(),
            lane_catalog,
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("vault", "restricted").expect("domain id"),
            "voucher".parse().expect("asset definition name"),
        );
        let opaque_asset_definition =
            AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
                .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(SetKeyValue::asset_definition(
                opaque_asset_definition,
                "routing".parse().expect("metadata key"),
                Json::from("ok"),
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("voucher".to_owned())
                    .build(&submitter_id),
            ],
            catalog,
            router.lane_catalog.as_ref().clone(),
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque asset-definition metadata sets should defer to state"),
            None
        );
        assert_eq!(
            router.try_route_with_view(&tx, &state.view()).expect(
                "opaque asset-definition metadata set should route to the resolved dataspace"
            ),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn opaque_global_asset_definition_metadata_set_uses_stored_alias_dataspace() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog.clone(),
            lane_catalog,
        );
        let transparent_asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("domain id"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(SetKeyValue::asset_definition(
                opaque_asset_definition.clone(),
                "routing".parse().expect("metadata key"),
                Json::from("paynet"),
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&submitter_id),
            ],
            catalog,
            router.lane_catalog.as_ref().clone(),
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), opaque_asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            opaque_asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque global metadata set should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("opaque global metadata set should route through the stored alias home"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn opaque_asset_definition_metadata_remove_routes_to_resolved_target_dataspace() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog.clone(),
            lane_catalog,
        );
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("vault", "restricted").expect("domain id"),
            "voucher".parse().expect("asset definition name"),
        );
        let opaque_asset_definition =
            AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
                .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(RemoveKeyValue::asset_definition(
                opaque_asset_definition,
                "routing".parse().expect("metadata key"),
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(asset_definition)
                    .with_name("voucher".to_owned())
                    .build(&submitter_id),
            ],
            catalog,
            router.lane_catalog.as_ref().clone(),
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque asset-definition metadata removes should defer to state"),
            None
        );
        assert_eq!(
            router.try_route_with_view(&tx, &state.view()).expect(
                "opaque asset-definition metadata remove should route to the resolved dataspace"
            ),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn opaque_global_asset_definition_unregister_uses_stored_alias_dataspace() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane_id, dataspace_id),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            catalog.clone(),
            lane_catalog,
        );
        let transparent_asset_definition = AssetDefinitionId::new(
            DomainId::try_new("cash", "universal").expect("domain id"),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(Unregister::asset_definition(
                opaque_asset_definition.clone(),
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(opaque_asset_definition.clone())
                    .with_name("pkr".to_owned())
                    .with_balance_scope_policy(AssetBalancePolicy::Global)
                    .build(&submitter_id),
            ],
            catalog,
            router.lane_catalog.as_ref().clone(),
        );
        state
            .world
            .asset_definition_aliases
            .insert(alias.clone(), opaque_asset_definition.clone());
        state.world.asset_definition_alias_bindings.insert(
            opaque_asset_definition,
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("opaque global unregister should defer to state"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("opaque global unregister should route through the stored alias home"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }
}
