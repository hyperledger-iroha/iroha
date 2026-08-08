//! Lane and dataspace routing utilities for the transaction queue.
//!
//! These helpers translate pending transactions into the lane/dataspace
//! identifiers that the Nexus scheduler expects, based on the runtime
//! configuration. The router abstraction keeps the queue decoupled from the
//! exact routing policy while allowing metrics to reflect the real
//! assignments instead of collapsing metrics to the primary lane.

use std::{collections::BTreeSet, str::FromStr, sync::Arc};

use iroha_config::parameters::actual::{
    LaneRoutingMatcher, LaneRoutingPolicy, LaneRoutingRule, Nexus,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::{AccountAlias, AccountId},
    asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId},
    domain::DomainId,
    isi::{
        BurnBox, CustomInstruction, GrantBox, Instruction, InstructionBox, MintBox, RegisterBox,
        RemoveKeyValueBox, RevokeBox, SetKeyValueBox, TransferBox, UnregisterBox,
        contract_alias::SetContractAlias,
        musubi::{
            AcceptMusubiPackageMaintainerV1, AddMusubiArchiveLocationV1,
            AssertMusubiReleaseDigestV1, InviteMusubiPackageMaintainerV1, PublishMusubiReleaseV1,
            RecoverMusubiPackageV1, RegisterMusubiAliasV1, RegisterMusubiArchiveV1,
            RegisterMusubiNamespaceBindingV1, RegisterMusubiProviderBundleAttestationV1,
            RemoveMusubiPackageMaintainerV1, RetargetMusubiAliasV1, RetireMusubiArchiveLocationV1,
            RevokeMusubiPackageMaintainerInvitationV1, SetMusubiArtifactTakedownV1,
            SetMusubiPackageMaintainerRoleV1, SetMusubiPackageMetadataV1,
            SetMusubiRegistryPolicyV1, SetMusubiReleaseYankV1,
        },
        offline::{RedeemKagemushaRecursiveV4, TopUpKagemushaRecursiveV4},
        settlement::{
            DvpIsi, FxCorridorPolicy, FxCorridorPolicyRegistry, PvpIsi, SetFxCorridorPolicy,
            SettleFxCorridor, SettlementInstructionBox,
        },
        smart_contract_code::{
            ActivateContractInstance, CommitContractDeployment, DeactivateContractInstance,
            FinalizeSmartContractCodeUpload, RegisterSmartContractBytes, RegisterSmartContractCode,
            UploadSmartContractCodeChunk,
        },
        space_directory::{
            ExpireSpaceDirectoryManifest, PublishSpaceDirectoryManifest,
            RevokeSpaceDirectoryManifest,
        },
        zk::{
            CancelConfidentialPolicyTransition, RegisterZkAsset,
            ScheduleConfidentialPolicyTransition,
        },
    },
    metadata::Metadata,
    musubi::MusubiPackageIdV1,
    name::Name,
    nexus::{
        AUTOSCALE_META_COMMITTEE, AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_DRAIN_STATE,
        AUTOSCALE_META_MANAGED, DataSpaceCatalog, DataSpaceId, LaneCatalog, LaneId,
    },
    permission::Permission,
    smart_contract::ContractAddress,
    state_path::StatePath,
    transaction::{Executable, ExecutableBatchItem, signed::TransactionPayload},
};
use iroha_executor_data_model::isi::multisig::{
    MultisigApprove, MultisigInstructionBox, MultisigProposalState, MultisigPropose,
};
use iroha_executor_data_model::permission::{
    account::{
        AccountAliasPermissionScope, CanDelegateAccountAliasResolution, CanManageAccountAlias,
        CanResolveAccountAlias,
    },
    asset::{
        CanBurnAssetWithDefinition, CanMintAssetToAccount, CanMintAssetWithDefinition,
        CanModifyAssetMetadataWithDefinition, CanTransferAssetWithDefinition,
    },
    asset_definition::{
        AssetDefinitionAliasPermissionScope, CanManageAssetDefinitionAlias,
        CanModifyAssetDefinitionMetadata, CanUnregisterAssetDefinition,
    },
    nexus::{
        CanEnrollFeeSponsorProgram, CanManageFeeSponsorProgram, CanPublishSpaceDirectoryManifest,
        CanPublishSpaceDirectoryManifestForAccountDomain, CanPublishSpaceDirectoryManifestForUaid,
        CanWithdrawFeeSponsorProgram,
    },
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

/// Read-only transaction fields consumed by deterministic lane and dataspace routing.
///
/// Both accepted transactions and canonical unsigned payloads implement this view so
/// quote discovery and queue admission execute the same routing code. Signatures and
/// envelope attachments deliberately do not participate in routing.
pub trait TransactionRoutingView {
    /// Return the authority when this entrypoint has one.
    fn authority_opt(&self) -> Option<&AccountId>;

    /// Return routing metadata when this entrypoint has it.
    fn metadata(&self) -> Option<&Metadata>;

    /// Return the executable used for native routing inspection.
    fn executable(&self) -> Option<&Executable>;

    /// Evaluate a predicate over the instruction batch used by matcher rules.
    fn any_matching_instruction(&self, predicate: &mut dyn FnMut(&dyn Instruction) -> bool)
    -> bool;

    /// Return the signature-independent hash used for elastic default-lane sharding.
    fn routing_hash(&self) -> Hash;
}

fn executable_any_matching_instruction(
    executable: &Executable,
    predicate: &mut dyn FnMut(&dyn Instruction) -> bool,
) -> bool {
    executable
        .explicit_instructions()
        .any(|instruction| predicate(&**instruction))
}

impl TransactionRoutingView for AcceptedTransaction<'_> {
    fn authority_opt(&self) -> Option<&AccountId> {
        AcceptedTransaction::authority_opt(self)
    }

    fn metadata(&self) -> Option<&Metadata> {
        AcceptedTransaction::metadata(self)
    }

    fn executable(&self) -> Option<&Executable> {
        match self.entrypoint() {
            iroha_data_model::transaction::TransactionEntrypoint::External(signed) => {
                Some(signed.instructions())
            }
            iroha_data_model::transaction::TransactionEntrypoint::SealedReveal(reveal) => {
                Some(reveal.signed_transaction().instructions())
            }
            iroha_data_model::transaction::TransactionEntrypoint::SealedCommitment(_)
            | iroha_data_model::transaction::TransactionEntrypoint::Time(_) => None,
        }
    }

    fn any_matching_instruction(
        &self,
        predicate: &mut dyn FnMut(&dyn Instruction) -> bool,
    ) -> bool {
        match self.entrypoint() {
            iroha_data_model::transaction::TransactionEntrypoint::External(signed) => {
                executable_any_matching_instruction(signed.instructions(), predicate)
            }
            iroha_data_model::transaction::TransactionEntrypoint::SealedReveal(reveal) => {
                executable_any_matching_instruction(
                    reveal.signed_transaction().instructions(),
                    predicate,
                )
            }
            iroha_data_model::transaction::TransactionEntrypoint::SealedCommitment(_)
            | iroha_data_model::transaction::TransactionEntrypoint::Time(_) => false,
        }
    }

    fn routing_hash(&self) -> Hash {
        self.external().map_or_else(
            || self.hash_as_entrypoint().into(),
            |signed| HashOf::new(signed.payload()).into(),
        )
    }
}

impl TransactionRoutingView for TransactionPayload {
    fn authority_opt(&self) -> Option<&AccountId> {
        Some(&self.authority)
    }

    fn metadata(&self) -> Option<&Metadata> {
        Some(&self.metadata)
    }

    fn executable(&self) -> Option<&Executable> {
        Some(&self.instructions)
    }

    fn any_matching_instruction(
        &self,
        predicate: &mut dyn FnMut(&dyn Instruction) -> bool,
    ) -> bool {
        executable_any_matching_instruction(&self.instructions, predicate)
    }

    fn routing_hash(&self) -> Hash {
        HashOf::new(self).into()
    }
}
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
#[derive(Clone, Debug, PartialEq, Eq, Error)]
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
    /// lane {lane_id} is not active for dataspace {dataspace_id} at the current block height
    #[error(
        "lane {lane_id} is not active for dataspace {dataspace_id} at the current block height"
    )]
    InactiveLane {
        /// Lane selected by the routing policy.
        lane_id: LaneId,
        /// Dataspace selected by the routing policy.
        dataspace_id: DataSpaceId,
    },
    /// no lane is bound to dataspace {dataspace_id}
    #[error("no lane is bound to dataspace {dataspace_id}")]
    NoLaneForDataspace {
        /// Dataspace selected by the routing policy.
        dataspace_id: DataSpaceId,
    },
    /// routing rule lane {lane_id} uses reserved autoscale metadata and cannot be an explicit rule target
    #[error(
        "routing rule lane {lane_id} uses reserved autoscale metadata and cannot be an explicit rule target"
    )]
    AutoscaleOwnedRuleLane {
        /// Lane selected by the matched routing rule.
        lane_id: LaneId,
    },
    /// default lane {lane_id} uses reserved autoscale metadata and cannot be the default route anchor
    #[error(
        "default lane {lane_id} uses reserved autoscale metadata and cannot be the default route anchor"
    )]
    AutoscaleOwnedDefaultLane {
        /// Lane selected by the default routing policy.
        lane_id: LaneId,
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
    /// FX corridor routing requires a world snapshot containing governed policy state.
    #[error("FX corridor policy `{policy_id}` cannot be routed without governed policy state")]
    FxCorridorPolicyStateUnavailable {
        /// Policy selected by the settlement instruction.
        policy_id: Name,
    },
    /// The protected FX corridor policy registry has not been initialized.
    #[error("the protected FX corridor policy registry is missing")]
    FxCorridorPolicyRegistryMissing,
    /// The protected FX corridor policy registry payload is malformed.
    #[error("the protected FX corridor policy registry is malformed")]
    FxCorridorPolicyRegistryMalformed,
    /// The selected FX corridor policy does not exist.
    #[error("FX corridor policy `{policy_id}` was not found")]
    FxCorridorPolicyNotFound {
        /// Policy selected by the settlement instruction.
        policy_id: Name,
    },
    /// provided routing plan does not match the current Nexus routing policy
    #[error("provided routing plan does not match the current Nexus routing policy")]
    StaleRoutingPlan,
}

impl RoutingResolveError {
    /// Stable telemetry label for deterministic routing failures.
    #[must_use]
    pub const fn as_label(&self) -> &'static str {
        match self {
            Self::UnknownLane { .. } => "unknown_lane",
            Self::UnknownDataspace { .. } => "unknown_dataspace",
            Self::LaneDataspaceMismatch { .. } => "lane_dataspace_mismatch",
            Self::InactiveLane { .. } => "inactive_lane",
            Self::NoLaneForDataspace { .. } => "no_lane_for_dataspace",
            Self::AutoscaleOwnedRuleLane { .. } => "autoscale_owned_rule_lane",
            Self::AutoscaleOwnedDefaultLane { .. } => "autoscale_owned_default_lane",
            Self::ConflictingDataspaceScopedPermissions { .. } => {
                "conflicting_dataspace_scoped_permissions"
            }
            Self::ConflictingTransactionDataspaceTargets { .. } => {
                "conflicting_transaction_dataspace_targets"
            }
            Self::FxCorridorPolicyStateUnavailable { .. } => "fx_corridor_policy_state_unavailable",
            Self::FxCorridorPolicyRegistryMissing => "fx_corridor_policy_registry_missing",
            Self::FxCorridorPolicyRegistryMalformed => "fx_corridor_policy_registry_malformed",
            Self::FxCorridorPolicyNotFound { .. } => "fx_corridor_policy_not_found",
            Self::StaleRoutingPlan => "stale_routing_plan",
        }
    }
}

/// Evaluate the configured routing policy for a transaction, returning the lane and dataspace.
///
/// This does not validate the decision against the lane or dataspace catalogs. Use
/// [`evaluate_policy_with_catalog`] when catalog alignment is required.
pub fn evaluate_policy(
    policy: &LaneRoutingPolicy,
    tx: &dyn TransactionRoutingView,
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

fn fail_closed_policy_route_with_view(
    policy: &LaneRoutingPolicy,
    tx: &dyn TransactionRoutingView,
    state_view: &StateView<'_>,
) -> RoutingDecision {
    let nexus = state_view.nexus();
    let target_dataspace = if let Some(account_id) = account_permission_holder_routing_target(tx) {
        account_dataspace_target(
            Some(state_view.world()),
            account_id,
            Some(state_view_ledger_time_ms(state_view)),
        )
    } else {
        let mut target = transaction_dataspace_routing_target_info(
            tx,
            Some(&nexus.dataspace_catalog),
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
        target.dataspace_id
    }
    .unwrap_or(policy.default_dataspace);

    canonical_dataspace_route(
        target_dataspace,
        &nexus.lane_catalog,
        &nexus.dataspace_catalog,
    )
    .or_else(|_| {
        canonical_dataspace_route(
            policy.default_dataspace,
            &nexus.lane_catalog,
            &nexus.dataspace_catalog,
        )
    })
    .unwrap_or_else(|_| RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
}

/// Evaluate the routing policy and resolve it against the configured catalogs.
pub fn evaluate_policy_with_catalog(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &dyn TransactionRoutingView,
) -> Result<RoutingDecision, RoutingResolveError> {
    if transaction_contains_fx_corridor_settlement(tx)
        && let Some(decision) =
            settlement_routing_decision(tx, lane_catalog, dataspace_catalog, None)?
    {
        return Ok(decision);
    }
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
        Some(tx),
        None,
    )
}

/// Evaluate the routing policy and resolve the full routing plan against the configured catalogs.
pub fn evaluate_policy_plan_with_catalog(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &dyn TransactionRoutingView,
) -> Result<RoutingPlan, RoutingResolveError> {
    if transaction_contains_fx_corridor_settlement(tx)
        && let Some(decision) =
            settlement_routing_decision(tx, lane_catalog, dataspace_catalog, None)?
    {
        return Ok(RoutingPlan::single(decision));
    }
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
        Some(tx),
        None,
    )
}

/// Evaluate the routing policy against catalogs, resolving opaque dataspace-scoped
/// permissions from the current world snapshot when possible.
pub fn evaluate_policy_with_catalog_and_world<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &dyn TransactionRoutingView,
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
    tx: &dyn TransactionRoutingView,
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
    tx: &dyn TransactionRoutingView,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<RoutingDecision, RoutingResolveError> {
    if transaction_contains_fx_corridor_settlement(tx)
        && let Some(decision) = settlement_routing_decision_with_world(
            tx,
            lane_catalog,
            dataspace_catalog,
            world,
            ledger_time_ms,
        )?
    {
        return Ok(decision);
    }
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
            None,
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
        authority_dataspace_target_with_world(Some(world), tx, ledger_time_ms),
        matched_rule.is_some_and(|rule| rule.matcher.account.is_some()),
    );
    resolve_policy_routing_decision(
        policy,
        matched_rule,
        target.dataspace_id,
        target.coordinator_route,
        lane_catalog,
        dataspace_catalog,
        Some(tx),
        None,
    )
}

/// Evaluate the routing policy and resolve the full routing plan against catalogs/world state.
pub fn evaluate_policy_plan_with_catalog_and_world<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &dyn TransactionRoutingView,
    world: &W,
) -> Result<RoutingPlan, RoutingResolveError> {
    evaluate_policy_plan_with_catalog_and_world_at_opt(
        policy,
        lane_catalog,
        dataspace_catalog,
        tx,
        world,
        None,
        None,
    )
}

/// Evaluate the routing policy and resolve the full plan at a deterministic ledger time.
pub fn evaluate_policy_plan_with_catalog_and_world_at<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &dyn TransactionRoutingView,
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
        None,
    )
}

/// Evaluate the active Nexus routing policy and resolve the full plan at a deterministic ledger
/// time.
///
/// Autoscale elastic default-route sharding requires a candidate block height; this heightless
/// helper therefore fails closed to non-elastic/default routing for no-target default traffic.
pub fn evaluate_policy_plan_with_nexus_and_world_at<W: WorldReadOnly>(
    nexus: &Nexus,
    tx: &dyn TransactionRoutingView,
    world: &W,
    ledger_time_ms: u64,
) -> Result<RoutingPlan, RoutingResolveError> {
    evaluate_policy_plan_with_catalog_and_world_at_opt(
        &nexus.routing_policy,
        &nexus.lane_catalog,
        &nexus.dataspace_catalog,
        tx,
        world,
        Some(ledger_time_ms),
        None,
    )
}

/// Evaluate the active Nexus routing policy and resolve the full plan at a deterministic ledger
/// time and block height.
pub fn evaluate_policy_plan_with_nexus_and_world_at_block_height<W: WorldReadOnly>(
    nexus: &Nexus,
    tx: &dyn TransactionRoutingView,
    world: &W,
    ledger_time_ms: u64,
    block_height: u64,
) -> Result<RoutingPlan, RoutingResolveError> {
    evaluate_policy_plan_with_catalog_and_world_at_opt(
        &nexus.routing_policy,
        &nexus.lane_catalog,
        &nexus.dataspace_catalog,
        tx,
        world,
        Some(ledger_time_ms),
        Some(AutoscaleElasticRange::from_nexus_at_height(
            nexus,
            block_height,
        )),
    )
}

fn evaluate_policy_plan_with_catalog_and_world_at_opt<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &dyn TransactionRoutingView,
    world: &W,
    ledger_time_ms: Option<u64>,
    autoscale_range: Option<AutoscaleElasticRange>,
) -> Result<RoutingPlan, RoutingResolveError> {
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches_with_world(rule, tx, dataspace_catalog, world, ledger_time_ms));
    if let Some(plan) = native_amx_fx_routing_plan_with_world(
        tx,
        matched_rule,
        lane_catalog,
        dataspace_catalog,
        world,
        ledger_time_ms,
    )? {
        return Ok(plan);
    }
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
            autoscale_range,
        )
        .map(RoutingPlan::single);
    }
    let mut target = transaction_dataspace_routing_target_info_with_world(
        tx,
        Some(dataspace_catalog),
        world,
        ledger_time_ms,
    )?;
    apply_authority_dataspace_target(
        &mut target,
        authority_dataspace_target_with_world(Some(world), tx, ledger_time_ms),
        matched_rule.is_some_and(|rule| rule.matcher.account.is_some()),
    );
    resolve_policy_routing_plan(
        policy,
        matched_rule,
        target,
        lane_catalog,
        dataspace_catalog,
        Some(tx),
        autoscale_range,
    )
}

fn dataspace_scoped_permission_routing_decision(
    tx: &dyn TransactionRoutingView,
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
        Executable::Batch(items) => {
            // A mixed native/contract batch must be routed from the complete target set below.
            // Returning the contract target from this permission shortcut would discard ordinary
            // native targets and collapse a cross-dataspace batch onto the contract lane.
            if items
                .iter()
                .any(|item| matches!(item, ExecutableBatchItem::ContractCall(_)))
            {
                return Ok(None);
            }
            for item in items {
                let ExecutableBatchItem::Instruction(instruction) = item else {
                    unreachable!("contract-call batches return before the permission shortcut");
                };
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
    tx: &dyn TransactionRoutingView,
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
        Executable::Batch(items) => {
            // A mixed native/contract batch must be routed from the complete target set below.
            // Returning the contract target from this permission shortcut would discard ordinary
            // native targets and collapse a cross-dataspace batch onto the contract lane.
            if items
                .iter()
                .any(|item| matches!(item, ExecutableBatchItem::ContractCall(_)))
            {
                return Ok(None);
            }
            for item in items {
                let ExecutableBatchItem::Instruction(instruction) = item else {
                    unreachable!("contract-call batches return before the permission shortcut");
                };
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
    tx: &dyn TransactionRoutingView,
) -> Option<RoutingDecision> {
    if transaction_contains_fx_corridor_settlement(tx) {
        return Some(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL));
    }
    let dataspace_id = settlement_transaction_dataspace_target(tx, None, None).ok()??;
    (dataspace_id == DataSpaceId::UNIVERSAL)
        .then(|| RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
}

fn settlement_routing_decision(
    tx: &dyn TransactionRoutingView,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    state_view: Option<&StateView<'_>>,
) -> Result<Option<RoutingDecision>, RoutingResolveError> {
    let Some(dataspace_id) =
        settlement_transaction_dataspace_target(tx, Some(dataspace_catalog), state_view)?
    else {
        return Ok(None);
    };
    canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog).map(Some)
}

fn settlement_routing_decision_with_world<W: WorldReadOnly>(
    tx: &dyn TransactionRoutingView,
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
    )?
    else {
        return Ok(None);
    };
    canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog).map(Some)
}

fn native_amx_fx_routing_plan_with_world<W: WorldReadOnly>(
    tx: &dyn TransactionRoutingView,
    matched_rule: Option<&LaneRoutingRule>,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<Option<RoutingPlan>, RoutingResolveError> {
    if !transaction_contains_fx_corridor_settlement(tx) {
        return Ok(None);
    }

    let coordinator_route =
        canonical_dataspace_route(DataSpaceId::UNIVERSAL, lane_catalog, dataspace_catalog)?;
    let mut participant_dataspaces = native_amx_participant_dataspaces_with_world_at(
        tx,
        dataspace_catalog,
        world,
        ledger_time_ms,
    )?;
    if amx_policy_rejects_cross_dataspace(tx) && participant_dataspaces.len() > 1 {
        return Err(native_dataspace_conflict_error(
            NativeDataspaceConflict::Transaction,
            participant_dataspaces[0],
            participant_dataspaces[1],
        ));
    }
    if let Some(policy_dataspace) = smart_contract_deploy_policy_dataspace(matched_rule) {
        participant_dataspaces.push(policy_dataspace);
        participant_dataspaces.sort_unstable();
        participant_dataspaces.dedup();
    }
    let participants = participant_dataspaces
        .into_iter()
        .map(|dataspace_id| {
            canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog)
                .map(|route| RouteLeg::new(route, RouteLegRole::Participant))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(RoutingPlan::native_amx(
        coordinator_route,
        participants,
    )))
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

fn fx_corridor_policy_with_world<W: WorldReadOnly>(
    world: &W,
    policy_id: &Name,
) -> Result<FxCorridorPolicy, RoutingResolveError> {
    let custom = world
        .parameters()
        .custom()
        .get(&FxCorridorPolicyRegistry::parameter_id())
        .ok_or(RoutingResolveError::FxCorridorPolicyRegistryMissing)?;
    FxCorridorPolicyRegistry::from_custom_parameter(custom)
        .map_err(|_| RoutingResolveError::FxCorridorPolicyRegistryMalformed)?
        .ok_or(RoutingResolveError::FxCorridorPolicyRegistryMalformed)?
        .get(policy_id)
        .cloned()
        .ok_or_else(|| RoutingResolveError::FxCorridorPolicyNotFound {
            policy_id: policy_id.clone(),
        })
}

fn fx_corridor_policy_with_state(
    state_view: Option<&StateView<'_>>,
    policy_id: &Name,
) -> Result<FxCorridorPolicy, RoutingResolveError> {
    let state_view =
        state_view.ok_or_else(|| RoutingResolveError::FxCorridorPolicyStateUnavailable {
            policy_id: policy_id.clone(),
        })?;
    fx_corridor_policy_with_world(state_view.world(), policy_id)
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

fn trigger_executable_transaction_dataspace_target(
    executable: &Executable,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    match executable {
        Executable::ContractCall(call) => contract_address_dataspace_target(&call.contract_address),
        Executable::Instructions(instructions) => {
            merge_instruction_dataspace_targets(instructions.iter().map(|instruction| {
                instruction_transaction_dataspace_target(
                    &**instruction,
                    dataspace_catalog,
                    state_view,
                )
            }))
        }
        Executable::Batch(items) => {
            merge_instruction_dataspace_targets(items.iter().map(|item| match item {
                ExecutableBatchItem::Instruction(instruction) => {
                    instruction_transaction_dataspace_target(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    )
                }
                ExecutableBatchItem::ContractCall(call) => {
                    contract_address_dataspace_target(&call.contract_address)
                }
            }))
        }
        Executable::Ivm(_) => None,
        Executable::IvmProved(proved) => {
            merge_instruction_dataspace_targets(proved.overlay.iter().map(|instruction| {
                instruction_transaction_dataspace_target(
                    &**instruction,
                    dataspace_catalog,
                    state_view,
                )
            }))
        }
    }
}

fn trigger_executable_transaction_dataspace_target_with_world<W: WorldReadOnly>(
    executable: &Executable,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    match executable {
        Executable::ContractCall(call) => contract_address_dataspace_target(&call.contract_address),
        Executable::Instructions(instructions) => {
            merge_instruction_dataspace_targets(instructions.iter().map(|instruction| {
                instruction_transaction_dataspace_target_with_world(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                )
            }))
        }
        Executable::Batch(items) => {
            merge_instruction_dataspace_targets(items.iter().map(|item| match item {
                ExecutableBatchItem::Instruction(instruction) => {
                    instruction_transaction_dataspace_target_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }
                ExecutableBatchItem::ContractCall(call) => {
                    contract_address_dataspace_target(&call.contract_address)
                }
            }))
        }
        Executable::Ivm(_) => None,
        Executable::IvmProved(proved) => {
            merge_instruction_dataspace_targets(proved.overlay.iter().map(|instruction| {
                instruction_transaction_dataspace_target_with_world(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                )
            }))
        }
    }
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

fn asset_definition_requires_universal_coordinator(
    asset_definition_id: &AssetDefinitionId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> bool {
    asset_balance_definition_route_target(asset_definition_id, dataspace_catalog, state_view)
        .balance_scope_policy
        == Some(AssetBalancePolicy::Global)
}

fn asset_definition_requires_universal_coordinator_with_world<W: WorldReadOnly>(
    asset_definition_id: &AssetDefinitionId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> bool {
    asset_balance_definition_route_target_with_world(
        asset_definition_id,
        dataspace_catalog,
        world,
        ledger_time_ms,
    )
    .balance_scope_policy
        == Some(AssetBalancePolicy::Global)
}

fn settlement_transaction_dataspace_target(
    tx: &dyn TransactionRoutingView,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Result<Option<DataSpaceId>, RoutingResolveError> {
    let Some(executable) = transaction_executable(tx) else {
        return Ok(None);
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
                    )?,
                );
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::Batch(items) => {
            for item in items {
                if let ExecutableBatchItem::Instruction(instruction) = item {
                    merge_settlement_target_dataspace(
                        &mut target_dataspace,
                        instruction_settlement_dataspace_target(
                            &**instruction,
                            dataspace_catalog,
                            state_view,
                        )?,
                    );
                }
            }
        }
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                merge_settlement_target_dataspace(
                    &mut target_dataspace,
                    instruction_settlement_dataspace_target(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    )?,
                );
            }
        }
    }

    Ok(target_dataspace)
}

fn settlement_transaction_dataspace_target_with_world<W: WorldReadOnly>(
    tx: &dyn TransactionRoutingView,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<Option<DataSpaceId>, RoutingResolveError> {
    let Some(executable) = transaction_executable(tx) else {
        return Ok(None);
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
                    )?,
                );
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::Batch(items) => {
            for item in items {
                if let ExecutableBatchItem::Instruction(instruction) = item {
                    merge_settlement_target_dataspace(
                        &mut target_dataspace,
                        instruction_settlement_dataspace_target_with_world(
                            &**instruction,
                            dataspace_catalog,
                            world,
                            ledger_time_ms,
                        )?,
                    );
                }
            }
        }
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                merge_settlement_target_dataspace(
                    &mut target_dataspace,
                    instruction_settlement_dataspace_target_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )?,
                );
            }
        }
    }

    Ok(target_dataspace)
}

fn instruction_settlement_dataspace_target(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Result<Option<DataSpaceId>, RoutingResolveError> {
    let any = instruction.as_any();

    if let Some(dvp) = any.downcast_ref::<DvpIsi>() {
        return Ok(settlement_pair_dataspace_target(
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
        ));
    }

    if let Some(pvp) = any.downcast_ref::<PvpIsi>() {
        return Ok(settlement_pair_dataspace_target(
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
        ));
    }

    if any.downcast_ref::<SetFxCorridorPolicy>().is_some() {
        return Ok(Some(DataSpaceId::UNIVERSAL));
    }

    if let Some(fx) = any.downcast_ref::<SettleFxCorridor>() {
        let policy = fx_corridor_policy_with_state(state_view, &fx.policy_id)?;
        return Ok(settlement_pair_dataspace_target(
            Some(policy.source_dataspace),
            Some(policy.destination_dataspace),
        ));
    }

    if let Some(settlement) = any.downcast_ref::<SettlementInstructionBox>() {
        return Ok(match settlement {
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
            SettlementInstructionBox::SetFxCorridorPolicy(_) => Some(DataSpaceId::UNIVERSAL),
            SettlementInstructionBox::SettleFxCorridor(fx) => {
                let policy = fx_corridor_policy_with_state(state_view, &fx.policy_id)?;
                settlement_pair_dataspace_target(
                    Some(policy.source_dataspace),
                    Some(policy.destination_dataspace),
                )
            }
        });
    }

    Ok(None)
}

fn instruction_settlement_dataspace_target_with_world<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<Option<DataSpaceId>, RoutingResolveError> {
    let any = instruction.as_any();

    if let Some(dvp) = any.downcast_ref::<DvpIsi>() {
        return Ok(settlement_pair_dataspace_target(
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
        ));
    }

    if let Some(pvp) = any.downcast_ref::<PvpIsi>() {
        return Ok(settlement_pair_dataspace_target(
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
        ));
    }

    if any.downcast_ref::<SetFxCorridorPolicy>().is_some() {
        return Ok(Some(DataSpaceId::UNIVERSAL));
    }

    if let Some(fx) = any.downcast_ref::<SettleFxCorridor>() {
        let policy = fx_corridor_policy_with_world(world, &fx.policy_id)?;
        return Ok(settlement_pair_dataspace_target(
            Some(policy.source_dataspace),
            Some(policy.destination_dataspace),
        ));
    }

    if let Some(settlement) = any.downcast_ref::<SettlementInstructionBox>() {
        return Ok(match settlement {
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
            SettlementInstructionBox::SetFxCorridorPolicy(_) => Some(DataSpaceId::UNIVERSAL),
            SettlementInstructionBox::SettleFxCorridor(fx) => {
                let policy = fx_corridor_policy_with_world(world, &fx.policy_id)?;
                settlement_pair_dataspace_target(
                    Some(policy.source_dataspace),
                    Some(policy.destination_dataspace),
                )
            }
        });
    }

    Ok(None)
}

fn transaction_contains_fx_corridor_settlement(tx: &dyn TransactionRoutingView) -> bool {
    let Some(executable) = transaction_executable(tx) else {
        return false;
    };
    let contains = |instruction: &InstructionBox| {
        let any = instruction.as_any();
        any.downcast_ref::<SettleFxCorridor>().is_some()
            || matches!(
                any.downcast_ref::<SettlementInstructionBox>(),
                Some(SettlementInstructionBox::SettleFxCorridor(_))
            )
    };
    match executable {
        Executable::Instructions(instructions) => instructions.iter().any(contains),
        Executable::IvmProved(proved) => proved.overlay.iter().any(contains),
        Executable::Batch(items) => items.iter().any(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => contains(instruction),
            ExecutableBatchItem::ContractCall(_) => false,
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) => false,
    }
}

fn transaction_executable(tx: &dyn TransactionRoutingView) -> Option<&Executable> {
    tx.executable()
}

fn amx_policy_rejects_cross_dataspace(tx: &dyn TransactionRoutingView) -> bool {
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
    // Participant descriptors are collected on `TransactionDataspaceTarget::participants` and
    // later committed through `RoutingPlan::NativeAmx` plus block execution-context receipts.
    // This merge only decides whether the coordinator must fall back to the universal route.
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

struct SameTransactionMultisigProposalTarget {
    account: AccountId,
    instructions_hash: HashOf<Vec<InstructionBox>>,
    dataspace_id: Option<DataSpaceId>,
    concrete_dataspaces: BTreeSet<DataSpaceId>,
    requires_universal_coordinator: bool,
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

fn merge_transaction_concrete_or_collapsed_dataspaces(
    target: &mut TransactionDataspaceTarget,
    concrete_dataspaces: Option<BTreeSet<DataSpaceId>>,
    collapsed_dataspace: Option<DataSpaceId>,
    reject_cross_dataspace: bool,
) -> Result<(), RoutingResolveError> {
    if let Some(concrete_dataspaces) =
        concrete_dataspaces.filter(|dataspaces| !dataspaces.is_empty())
    {
        for dataspace in concrete_dataspaces {
            merge_transaction_target_dataspace(target, Some(dataspace), reject_cross_dataspace)?;
        }
        return Ok(());
    }

    merge_transaction_target_dataspace(target, collapsed_dataspace, reject_cross_dataspace)
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
    tx: &dyn TransactionRoutingView,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Result<Option<DataSpaceId>, RoutingResolveError> {
    transaction_dataspace_routing_target_info(tx, dataspace_catalog, state_view)
        .map(|target| target.dataspace_id)
}

fn transaction_dataspace_routing_target_info(
    tx: &dyn TransactionRoutingView,
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
            let instruction_refs = instructions.iter().collect::<Vec<_>>();
            let same_transaction_multisig_proposals = same_transaction_multisig_proposal_targets(
                &instruction_refs,
                dataspace_catalog,
                state_view,
            );
            for instruction in instructions {
                let same_transaction_approve_target =
                    same_transaction_multisig_approve_route_target(
                        &same_transaction_multisig_proposals,
                        &**instruction,
                    );
                let instruction_target = same_transaction_approve_target.map_or_else(
                    || {
                        instruction_transaction_dataspace_target(
                            &**instruction,
                            dataspace_catalog,
                            state_view,
                        )
                    },
                    |target| target.dataspace_id,
                );
                merge_transaction_concrete_or_collapsed_dataspaces(
                    &mut target,
                    same_transaction_approve_target
                        .map(|proposal| proposal.concrete_dataspaces.clone())
                        .or_else(|| {
                            deferred_instruction_concrete_dataspace_targets(
                                &**instruction,
                                dataspace_catalog,
                                state_view,
                            )
                        }),
                    instruction_target,
                    reject_cross_dataspace,
                )?;
                let requires_universal_coordinator = same_transaction_approve_target.map_or_else(
                    || {
                        instruction_transaction_target_requires_universal_coordinator(
                            &**instruction,
                            dataspace_catalog,
                            state_view,
                        )
                    },
                    |target| target.requires_universal_coordinator,
                );
                if instruction_target == Some(DataSpaceId::UNIVERSAL)
                    && requires_universal_coordinator
                {
                    target.coordinator_route = true;
                }
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::Batch(items) => {
            for item in items {
                let item_target = match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        let instruction_target = instruction_transaction_dataspace_target(
                            &**instruction,
                            dataspace_catalog,
                            state_view,
                        );
                        merge_transaction_concrete_or_collapsed_dataspaces(
                            &mut target,
                            deferred_instruction_concrete_dataspace_targets(
                                &**instruction,
                                dataspace_catalog,
                                state_view,
                            ),
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
                        continue;
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        contract_address_dataspace_target(&call.contract_address)
                    }
                };
                merge_transaction_concrete_or_collapsed_dataspaces(
                    &mut target,
                    item_target.map(|dataspace| BTreeSet::from([dataspace])),
                    item_target,
                    reject_cross_dataspace,
                )?;
                if item_target == Some(DataSpaceId::UNIVERSAL) {
                    target.coordinator_route = true;
                }
            }
        }
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                let instruction_target = instruction_transaction_dataspace_target(
                    &**instruction,
                    dataspace_catalog,
                    state_view,
                );
                merge_transaction_concrete_or_collapsed_dataspaces(
                    &mut target,
                    deferred_instruction_concrete_dataspace_targets(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    ),
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
    tx: &dyn TransactionRoutingView,
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
            let instruction_refs = instructions.iter().collect::<Vec<_>>();
            let same_transaction_multisig_proposals =
                same_transaction_multisig_proposal_targets_with_world(
                    &instruction_refs,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                );
            for instruction in instructions {
                let same_transaction_approve_target =
                    same_transaction_multisig_approve_route_target(
                        &same_transaction_multisig_proposals,
                        &**instruction,
                    );
                let instruction_target = same_transaction_approve_target.map_or_else(
                    || {
                        instruction_transaction_dataspace_target_with_world(
                            &**instruction,
                            dataspace_catalog,
                            world,
                            ledger_time_ms,
                        )
                    },
                    |target| target.dataspace_id,
                );
                merge_transaction_concrete_or_collapsed_dataspaces(
                    &mut target,
                    same_transaction_approve_target
                        .map(|proposal| proposal.concrete_dataspaces.clone())
                        .or_else(|| {
                            deferred_instruction_concrete_dataspace_targets_with_world(
                                &**instruction,
                                dataspace_catalog,
                                world,
                                ledger_time_ms,
                            )
                        }),
                    instruction_target,
                    reject_cross_dataspace,
                )?;
                let requires_universal_coordinator = same_transaction_approve_target.map_or_else(
                    || {
                        instruction_transaction_target_requires_universal_coordinator_with_world(
                            &**instruction,
                            dataspace_catalog,
                            world,
                            ledger_time_ms,
                        )
                    },
                    |target| target.requires_universal_coordinator,
                );
                if instruction_target == Some(DataSpaceId::UNIVERSAL)
                    && requires_universal_coordinator
                {
                    target.coordinator_route = true;
                }
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::Batch(items) => {
            for item in items {
                let item_target = match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        let instruction_target =
                            instruction_transaction_dataspace_target_with_world(
                                &**instruction,
                                dataspace_catalog,
                                world,
                                ledger_time_ms,
                            );
                        merge_transaction_concrete_or_collapsed_dataspaces(
                            &mut target,
                            deferred_instruction_concrete_dataspace_targets_with_world(
                                &**instruction,
                                dataspace_catalog,
                                world,
                                ledger_time_ms,
                            ),
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
                        continue;
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        contract_address_dataspace_target(&call.contract_address)
                    }
                };
                merge_transaction_concrete_or_collapsed_dataspaces(
                    &mut target,
                    item_target.map(|dataspace| BTreeSet::from([dataspace])),
                    item_target,
                    reject_cross_dataspace,
                )?;
                if item_target == Some(DataSpaceId::UNIVERSAL) {
                    target.coordinator_route = true;
                }
            }
        }
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                let instruction_target = instruction_transaction_dataspace_target_with_world(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                );
                merge_transaction_concrete_or_collapsed_dataspaces(
                    &mut target,
                    deferred_instruction_concrete_dataspace_targets_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    ),
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
    tx: &dyn TransactionRoutingView,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
) -> Vec<DataSpaceId> {
    native_amx_participant_dataspaces_with_world_at(tx, dataspace_catalog, world, None)
        .unwrap_or_default()
}

fn native_amx_participant_dataspaces_with_world_at<W: WorldReadOnly>(
    tx: &dyn TransactionRoutingView,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Result<Vec<DataSpaceId>, RoutingResolveError> {
    let mut dataspaces = std::collections::BTreeSet::new();
    let Some(executable) = transaction_executable(tx) else {
        return Ok(Vec::new());
    };

    match executable {
        Executable::Instructions(instructions) => {
            let instruction_refs = instructions.iter().collect::<Vec<_>>();
            let same_transaction_multisig_proposals =
                same_transaction_multisig_proposal_targets_with_world(
                    &instruction_refs,
                    Some(dataspace_catalog),
                    world,
                    ledger_time_ms,
                );
            for instruction in instructions {
                if let Some(proposal) = same_transaction_multisig_approve_route_target(
                    &same_transaction_multisig_proposals,
                    &**instruction,
                ) && !proposal.concrete_dataspaces.is_empty()
                {
                    for dataspace in &proposal.concrete_dataspaces {
                        insert_native_amx_participant(&mut dataspaces, Some(*dataspace));
                    }
                    continue;
                }
                collect_instruction_native_amx_participants(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                    &mut dataspaces,
                )?;
            }
        }
        Executable::ContractCall(call) => {
            insert_native_amx_participant(
                &mut dataspaces,
                contract_address_dataspace_target(&call.contract_address),
            );
        }
        Executable::Batch(items) => {
            for item in items {
                match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        collect_instruction_native_amx_participants(
                            &**instruction,
                            dataspace_catalog,
                            world,
                            ledger_time_ms,
                            &mut dataspaces,
                        )?;
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        insert_native_amx_participant(
                            &mut dataspaces,
                            contract_address_dataspace_target(&call.contract_address),
                        );
                    }
                }
            }
        }
        Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                collect_instruction_native_amx_participants(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                    &mut dataspaces,
                )?;
            }
        }
    }

    Ok(dataspaces.into_iter().collect())
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

fn collect_settlement_pair_native_amx_participants<W: WorldReadOnly>(
    dataspaces: &mut std::collections::BTreeSet<DataSpaceId>,
    first_asset_definition: &AssetDefinitionId,
    second_asset_definition: &AssetDefinitionId,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
) {
    for asset_definition in [first_asset_definition, second_asset_definition] {
        insert_native_amx_participant(
            dataspaces,
            asset_balance_definition_dataspace_target_with_world(
                asset_definition,
                Some(dataspace_catalog),
                world,
                ledger_time_ms,
            ),
        );
    }
}

fn collect_trigger_executable_native_amx_participants<W: WorldReadOnly>(
    executable: &Executable,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
    dataspaces: &mut BTreeSet<DataSpaceId>,
) -> Result<(), RoutingResolveError> {
    match executable {
        Executable::ContractCall(call) => {
            insert_native_amx_participant(
                dataspaces,
                contract_address_dataspace_target(&call.contract_address),
            );
        }
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                collect_instruction_native_amx_participants(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                    dataspaces,
                )?;
            }
        }
        Executable::Batch(items) => {
            for item in items {
                match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        collect_instruction_native_amx_participants(
                            &**instruction,
                            dataspace_catalog,
                            world,
                            ledger_time_ms,
                            dataspaces,
                        )?;
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        insert_native_amx_participant(
                            dataspaces,
                            contract_address_dataspace_target(&call.contract_address),
                        );
                    }
                }
            }
        }
        Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                collect_instruction_native_amx_participants(
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                    dataspaces,
                )?;
            }
        }
    }
    Ok(())
}

fn collect_instruction_native_amx_participants<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
    dataspaces: &mut std::collections::BTreeSet<DataSpaceId>,
) -> Result<(), RoutingResolveError> {
    insert_native_amx_participant(
        dataspaces,
        instruction_dataspace_scoped_permission_target_with_world(
            instruction,
            Some(dataspace_catalog),
            world,
            ledger_time_ms,
        ),
    );

    let any = instruction.as_any();
    if let Some(primary) =
        any.downcast_ref::<iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias>()
    {
        let alias_dataspaces = compare_and_set_primary_account_alias_dataspace_targets(primary);
        if alias_dataspaces.is_empty() {
            insert_native_amx_participant(
                dataspaces,
                account_dataspace_target(Some(world), &primary.account, ledger_time_ms),
            );
        } else {
            for dataspace in alias_dataspaces {
                insert_native_amx_participant(dataspaces, Some(dataspace));
            }
        }
        return Ok(());
    }

    if let Some(multisig) = multisig_instruction(instruction) {
        let (account, instructions) = match multisig {
            MultisigInstructionBox::Propose(propose) => {
                (Some(propose.account), Some(propose.instructions))
            }
            MultisigInstructionBox::Approve(approve) => (
                Some(approve.account.clone()),
                multisig_proposal_state(world, &approve.account, &approve.instructions_hash)
                    .map(|proposal| proposal.instructions),
            ),
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => (None, None),
        };
        let mut nested_dataspaces = BTreeSet::new();
        if let Some(instructions) = instructions {
            for nested in &instructions {
                collect_instruction_native_amx_participants(
                    &**nested,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                    &mut nested_dataspaces,
                )?;
            }
        }
        if nested_dataspaces.is_empty() {
            insert_native_amx_participant(
                &mut nested_dataspaces,
                account.as_ref().and_then(|account| {
                    account_dataspace_target(Some(world), account, ledger_time_ms)
                }),
            );
        }
        dataspaces.extend(nested_dataspaces);
        return Ok(());
    }

    if let Some(RegisterBox::Trigger(register)) = any.downcast_ref::<RegisterBox>() {
        return collect_trigger_executable_native_amx_participants(
            register.object.action().executable(),
            dataspace_catalog,
            world,
            ledger_time_ms,
            dataspaces,
        );
    }

    let fx_instruction = any.downcast_ref::<SettleFxCorridor>().or_else(|| {
        any.downcast_ref::<SettlementInstructionBox>()
            .and_then(|settlement| match settlement {
                SettlementInstructionBox::SettleFxCorridor(fx) => Some(fx),
                SettlementInstructionBox::Dvp(_)
                | SettlementInstructionBox::Pvp(_)
                | SettlementInstructionBox::SetFxCorridorPolicy(_) => None,
            })
    });
    if let Some(fx) = fx_instruction {
        let policy = fx_corridor_policy_with_world(world, &fx.policy_id)?;
        insert_native_amx_participant(dataspaces, Some(policy.source_dataspace));
        insert_native_amx_participant(dataspaces, Some(policy.destination_dataspace));
        return Ok(());
    }

    let settlement_pair = if let Some(dvp) = any.downcast_ref::<DvpIsi>() {
        Some((
            dvp.delivery_leg().asset_definition_id(),
            dvp.payment_leg().asset_definition_id(),
        ))
    } else if let Some(pvp) = any.downcast_ref::<PvpIsi>() {
        Some((
            pvp.primary_leg().asset_definition_id(),
            pvp.counter_leg().asset_definition_id(),
        ))
    } else {
        any.downcast_ref::<SettlementInstructionBox>()
            .and_then(|settlement| match settlement {
                SettlementInstructionBox::Dvp(dvp) => Some((
                    dvp.delivery_leg().asset_definition_id(),
                    dvp.payment_leg().asset_definition_id(),
                )),
                SettlementInstructionBox::Pvp(pvp) => Some((
                    pvp.primary_leg().asset_definition_id(),
                    pvp.counter_leg().asset_definition_id(),
                )),
                SettlementInstructionBox::SetFxCorridorPolicy(_)
                | SettlementInstructionBox::SettleFxCorridor(_) => None,
            })
    };
    if let Some((first_asset_definition, second_asset_definition)) = settlement_pair {
        collect_settlement_pair_native_amx_participants(
            dataspaces,
            first_asset_definition,
            second_asset_definition,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
        return Ok(());
    }

    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        if let TransferBox::Asset(transfer) = transfer {
            collect_asset_balance_native_amx_participants(
                dataspaces,
                asset_balance_definition_route_target_with_world(
                    &transfer.source.definition,
                    Some(dataspace_catalog),
                    world,
                    ledger_time_ms,
                ),
                asset_id_explicit_dataspace_target(&transfer.source),
                [
                    account_dataspace_target(Some(world), &transfer.source.account, ledger_time_ms),
                    account_dataspace_target(Some(world), &transfer.destination, ledger_time_ms),
                ],
            );
            return Ok(());
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
                    ledger_time_ms,
                ),
                asset_id_explicit_dataspace_target(&mint.destination),
                [account_dataspace_target(
                    Some(world),
                    &mint.destination.account,
                    ledger_time_ms,
                )],
            );
            return Ok(());
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
                    ledger_time_ms,
                ),
                asset_id_explicit_dataspace_target(&burn.destination),
                [account_dataspace_target(
                    Some(world),
                    &burn.destination.account,
                    ledger_time_ms,
                )],
            );
            return Ok(());
        }
    }

    insert_native_amx_participant(
        dataspaces,
        instruction_transaction_dataspace_target_with_world(
            instruction,
            Some(dataspace_catalog),
            world,
            ledger_time_ms,
        ),
    );
    Ok(())
}

enum AccountPermissionHolderTarget<'account> {
    Holder(&'account AccountId),
    Skip,
    Abort,
}

fn account_permission_holder_routing_target<'tx>(
    tx: &'tx dyn TransactionRoutingView,
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
        Executable::Batch(items) => account_permission_holder_from_instructions(
            items.iter().filter_map(|item| match item {
                ExecutableBatchItem::Instruction(instruction) => Some(&**instruction),
                ExecutableBatchItem::ContractCall(_) => None,
            }),
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

fn compare_and_set_primary_account_alias_dataspace_targets(
    primary: &iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias,
) -> BTreeSet<DataSpaceId> {
    primary
        .expected_alias
        .iter()
        .chain(primary.new_alias.iter())
        .map(|alias| alias.dataspace_id)
        .collect()
}

fn compare_and_set_primary_account_alias_dataspace_target(
    primary: &iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias,
) -> Option<DataSpaceId> {
    merge_instruction_dataspace_targets(
        compare_and_set_primary_account_alias_dataspace_targets(primary)
            .into_iter()
            .map(Some),
    )
}

fn instruction_transaction_dataspace_target(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let any = instruction.as_any();

    if let Some(ensure) = any.downcast_ref::<iroha_data_model::isi::alias_setup::EnsureAlias>() {
        return Some(ensure.intent.target().dataspace_id());
    }
    if let Some(renew) = any.downcast_ref::<iroha_data_model::isi::alias_setup::RenewAliasLease>() {
        return Some(renew.target.dataspace_id());
    }
    if let Some(configure) =
        any.downcast_ref::<iroha_data_model::isi::alias_setup::ConfigureAliasAutoRenew>()
    {
        return Some(configure.target.dataspace_id());
    }
    if let Some(rebind) =
        any.downcast_ref::<iroha_data_model::isi::alias_setup::RebindAccountAlias>()
    {
        return Some(rebind.alias.dataspace_id);
    }
    if let Some(primary) =
        any.downcast_ref::<iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias>()
    {
        return compare_and_set_primary_account_alias_dataspace_target(primary).or_else(|| {
            account_dataspace_target(
                state_view.map(StateView::world),
                &primary.account,
                state_view.map(state_view_ledger_time_ms),
            )
        });
    }

    if let Some(multisig) = multisig_instruction(instruction) {
        return match &multisig {
            MultisigInstructionBox::Propose(propose) => {
                multisig_propose_transaction_dataspace_target(
                    propose,
                    dataspace_catalog,
                    state_view,
                )
            }
            MultisigInstructionBox::Approve(approve) => {
                multisig_approve_transaction_dataspace_target(
                    approve,
                    dataspace_catalog,
                    state_view,
                )
            }
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => None,
        };
    }

    if let Some(propose) = any.downcast_ref::<MultisigPropose>() {
        return multisig_propose_transaction_dataspace_target(
            propose,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(approve) = any.downcast_ref::<MultisigApprove>() {
        return multisig_approve_transaction_dataspace_target(
            approve,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        return match grant {
            GrantBox::Permission(grant) => {
                dataspace_scoped_permission_target(&grant.object, dataspace_catalog, state_view)
                    .or_else(|| {
                        account_dataspace_target(
                            state_view.map(StateView::world),
                            &grant.destination,
                            state_view.map(state_view_ledger_time_ms),
                        )
                    })
            }
            GrantBox::RolePermission(grant) => {
                dataspace_scoped_permission_target(&grant.object, dataspace_catalog, state_view)
            }
            GrantBox::Role(_) => None,
        };
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return match revoke {
            RevokeBox::Permission(revoke) => {
                dataspace_scoped_permission_target(&revoke.object, dataspace_catalog, state_view)
                    .or_else(|| {
                        account_dataspace_target(
                            state_view.map(StateView::world),
                            &revoke.destination,
                            state_view.map(state_view_ledger_time_ms),
                        )
                    })
            }
            RevokeBox::RolePermission(revoke) => {
                dataspace_scoped_permission_target(&revoke.object, dataspace_catalog, state_view)
            }
            RevokeBox::Role(_) => None,
        };
    }

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
                register.object.owning_domain.as_ref(),
                Some(register.object.balance_scope_policy),
                dataspace_catalog,
                state_view,
            ),
            RegisterBox::Trigger(register) => trigger_executable_transaction_dataspace_target(
                register.object.action().executable(),
                dataspace_catalog,
                state_view,
            ),
            RegisterBox::Peer(_) | RegisterBox::Nft(_) | RegisterBox::Role(_) => None,
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
            SetKeyValueBox::Account(set) => account_dataspace_target(
                state_view.map(StateView::world),
                &set.object,
                state_view.map(state_view_ledger_time_ms),
            ),
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
            RemoveKeyValueBox::Account(remove) => account_dataspace_target(
                state_view.map(StateView::world),
                &remove.object,
                state_view.map(state_view_ledger_time_ms),
            ),
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
                        state_view.map(state_view_ledger_time_ms),
                    ),
                    account_dataspace_target(
                        state_view.map(StateView::world),
                        &transfer.destination,
                        state_view.map(state_view_ledger_time_ms),
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
                    state_view.map(state_view_ledger_time_ms),
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
                    state_view.map(state_view_ledger_time_ms),
                )],
            ),
            BurnBox::TriggerRepetitions(_) => None,
        };
    }

    if let Some(register_zk_asset) = any.downcast_ref::<RegisterZkAsset>() {
        return asset_balance_definition_dataspace_target(
            &register_zk_asset.asset,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(schedule_transition) = any.downcast_ref::<ScheduleConfidentialPolicyTransition>() {
        return asset_balance_definition_dataspace_target(
            &schedule_transition.asset,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(cancel_transition) = any.downcast_ref::<CancelConfidentialPolicyTransition>() {
        return asset_balance_definition_dataspace_target(
            &cancel_transition.asset,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(target) = musubi_instruction_dataspace_target(any) {
        return Some(target);
    }

    if let Some(publish) = any.downcast_ref::<PublishSpaceDirectoryManifest>() {
        return Some(publish.manifest.dataspace);
    }

    if let Some(revoke) = any.downcast_ref::<RevokeSpaceDirectoryManifest>() {
        return Some(revoke.dataspace);
    }

    if let Some(expire) = any.downcast_ref::<ExpireSpaceDirectoryManifest>() {
        return Some(expire.dataspace);
    }

    if let Some(activate) = any.downcast_ref::<ActivateContractInstance>() {
        return contract_address_dataspace_target(&activate.contract_address);
    }

    if let Some(commit) = any.downcast_ref::<CommitContractDeployment>() {
        return contract_address_dataspace_target(&commit.contract_address);
    }

    if let Some(deactivate) = any.downcast_ref::<DeactivateContractInstance>() {
        return contract_address_dataspace_target(&deactivate.contract_address);
    }

    if let Some(set_alias) = any.downcast_ref::<SetContractAlias>() {
        return contract_address_dataspace_target(&set_alias.contract_address);
    }

    if let Some(asset_definition_id) = confidential_asset_definition_target(any) {
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

    if let Some(ensure) = any.downcast_ref::<iroha_data_model::isi::alias_setup::EnsureAlias>() {
        return Some(ensure.intent.target().dataspace_id());
    }
    if let Some(renew) = any.downcast_ref::<iroha_data_model::isi::alias_setup::RenewAliasLease>() {
        return Some(renew.target.dataspace_id());
    }
    if let Some(configure) =
        any.downcast_ref::<iroha_data_model::isi::alias_setup::ConfigureAliasAutoRenew>()
    {
        return Some(configure.target.dataspace_id());
    }
    if let Some(rebind) =
        any.downcast_ref::<iroha_data_model::isi::alias_setup::RebindAccountAlias>()
    {
        return Some(rebind.alias.dataspace_id);
    }
    if let Some(primary) =
        any.downcast_ref::<iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias>()
    {
        return compare_and_set_primary_account_alias_dataspace_target(primary)
            .or_else(|| account_dataspace_target(Some(world), &primary.account, ledger_time_ms));
    }

    if let Some(multisig) = multisig_instruction(instruction) {
        return match &multisig {
            MultisigInstructionBox::Propose(propose) => {
                multisig_propose_transaction_dataspace_target_with_world(
                    propose,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                )
            }
            MultisigInstructionBox::Approve(approve) => {
                multisig_approve_transaction_dataspace_target_with_world(
                    approve,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                )
            }
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => None,
        };
    }

    if let Some(propose) = any.downcast_ref::<MultisigPropose>() {
        return multisig_propose_transaction_dataspace_target_with_world(
            propose,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(approve) = any.downcast_ref::<MultisigApprove>() {
        return multisig_approve_transaction_dataspace_target_with_world(
            approve,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        return match grant {
            GrantBox::Permission(grant) => dataspace_scoped_permission_target_with_world(
                &grant.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            )
            .or_else(|| account_dataspace_target(Some(world), &grant.destination, ledger_time_ms)),
            GrantBox::RolePermission(grant) => dataspace_scoped_permission_target_with_world(
                &grant.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            GrantBox::Role(_) => None,
        };
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return match revoke {
            RevokeBox::Permission(revoke) => dataspace_scoped_permission_target_with_world(
                &revoke.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            )
            .or_else(|| account_dataspace_target(Some(world), &revoke.destination, ledger_time_ms)),
            RevokeBox::RolePermission(revoke) => dataspace_scoped_permission_target_with_world(
                &revoke.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            RevokeBox::Role(_) => None,
        };
    }

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
                register.object.owning_domain.as_ref(),
                Some(register.object.balance_scope_policy),
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            RegisterBox::Trigger(register) => {
                trigger_executable_transaction_dataspace_target_with_world(
                    register.object.action().executable(),
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                )
            }
            RegisterBox::Peer(_) | RegisterBox::Nft(_) | RegisterBox::Role(_) => None,
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
            SetKeyValueBox::Account(set) => {
                account_dataspace_target(Some(world), &set.object, ledger_time_ms)
            }
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
                account_dataspace_target(Some(world), &remove.object, ledger_time_ms)
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
                    account_dataspace_target(Some(world), &transfer.source.account, ledger_time_ms),
                    account_dataspace_target(Some(world), &transfer.destination, ledger_time_ms),
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
                    ledger_time_ms,
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
                    ledger_time_ms,
                )],
            ),
            BurnBox::TriggerRepetitions(_) => None,
        };
    }

    if let Some(register_zk_asset) = any.downcast_ref::<RegisterZkAsset>() {
        return asset_balance_definition_dataspace_target_with_world(
            &register_zk_asset.asset,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(schedule_transition) = any.downcast_ref::<ScheduleConfidentialPolicyTransition>() {
        return asset_balance_definition_dataspace_target_with_world(
            &schedule_transition.asset,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(cancel_transition) = any.downcast_ref::<CancelConfidentialPolicyTransition>() {
        return asset_balance_definition_dataspace_target_with_world(
            &cancel_transition.asset,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(target) = musubi_instruction_dataspace_target(any) {
        return Some(target);
    }

    if let Some(publish) = any.downcast_ref::<PublishSpaceDirectoryManifest>() {
        return Some(publish.manifest.dataspace);
    }

    if let Some(revoke) = any.downcast_ref::<RevokeSpaceDirectoryManifest>() {
        return Some(revoke.dataspace);
    }

    if let Some(expire) = any.downcast_ref::<ExpireSpaceDirectoryManifest>() {
        return Some(expire.dataspace);
    }

    if let Some(activate) = any.downcast_ref::<ActivateContractInstance>() {
        return contract_address_dataspace_target(&activate.contract_address);
    }

    if let Some(commit) = any.downcast_ref::<CommitContractDeployment>() {
        return contract_address_dataspace_target(&commit.contract_address);
    }

    if let Some(deactivate) = any.downcast_ref::<DeactivateContractInstance>() {
        return contract_address_dataspace_target(&deactivate.contract_address);
    }

    if let Some(set_alias) = any.downcast_ref::<SetContractAlias>() {
        return contract_address_dataspace_target(&set_alias.contract_address);
    }

    if let Some(asset_definition_id) = confidential_asset_definition_target(any) {
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

fn multisig_instruction(instruction: &dyn Instruction) -> Option<MultisigInstructionBox> {
    let any = instruction.as_any();
    if let Some(multisig) = instruction
        .as_any()
        .downcast_ref::<MultisigInstructionBox>()
    {
        return Some(multisig.clone());
    }
    if let Some(propose) = any.downcast_ref::<MultisigPropose>() {
        return Some(MultisigInstructionBox::Propose(propose.clone()));
    }
    if let Some(approve) = any.downcast_ref::<MultisigApprove>() {
        return Some(MultisigInstructionBox::Approve(approve.clone()));
    }
    any.downcast_ref::<CustomInstruction>().and_then(|custom| {
        MultisigInstructionBox::try_from(custom.payload())
            .ok()
            .or_else(|| {
                custom
                    .payload()
                    .try_into_any_norito::<MultisigPropose>()
                    .ok()
                    .map(MultisigInstructionBox::Propose)
            })
            .or_else(|| {
                custom
                    .payload()
                    .try_into_any_norito::<MultisigApprove>()
                    .ok()
                    .map(MultisigInstructionBox::Approve)
            })
    })
}

fn multisig_proposal_state_key(
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> StatePath {
    const DELIMITER: char = '/';
    const MULTISIG: &str = "multisig";
    const MULTISIG_PROPOSAL_STATE: &str = "proposal";

    StatePath::from_str(&format!(
        "{MULTISIG}{DELIMITER}{MULTISIG_PROPOSAL_STATE}{DELIMITER}{}{DELIMITER}{}",
        HashOf::new(multisig_account),
        instructions_hash
    ))
    .expect("multisig proposal state path must be valid")
}

fn multisig_proposal_state<W: WorldReadOnly>(
    world: &W,
    multisig_account: &AccountId,
    instructions_hash: &HashOf<Vec<InstructionBox>>,
) -> Option<MultisigProposalState> {
    let key = multisig_proposal_state_key(multisig_account, instructions_hash);
    let bytes = world.smart_contract_state().get(&key)?;
    norito::decode_from_bytes::<MultisigProposalState>(bytes).ok()
}

fn extend_instruction_concrete_dataspace_targets(
    targets: &mut BTreeSet<DataSpaceId>,
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) {
    if let Some(nested_targets) =
        deferred_instruction_concrete_dataspace_targets(instruction, dataspace_catalog, state_view)
        && !nested_targets.is_empty()
    {
        targets.extend(nested_targets);
        return;
    }

    if let Some(target) =
        instruction_transaction_dataspace_target(instruction, dataspace_catalog, state_view)
    {
        targets.insert(target);
    }
}

fn trigger_executable_concrete_dataspace_targets(
    executable: &Executable,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> BTreeSet<DataSpaceId> {
    let mut targets = BTreeSet::new();
    match executable {
        Executable::ContractCall(call) => {
            if let Some(target) = contract_address_dataspace_target(&call.contract_address) {
                targets.insert(target);
            }
        }
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                extend_instruction_concrete_dataspace_targets(
                    &mut targets,
                    &**instruction,
                    dataspace_catalog,
                    state_view,
                );
            }
        }
        Executable::Batch(items) => {
            for item in items {
                match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        extend_instruction_concrete_dataspace_targets(
                            &mut targets,
                            &**instruction,
                            dataspace_catalog,
                            state_view,
                        );
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        if let Some(target) =
                            contract_address_dataspace_target(&call.contract_address)
                        {
                            targets.insert(target);
                        }
                    }
                }
            }
        }
        Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                extend_instruction_concrete_dataspace_targets(
                    &mut targets,
                    &**instruction,
                    dataspace_catalog,
                    state_view,
                );
            }
        }
    }
    targets
}

fn deferred_instruction_concrete_dataspace_targets(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<BTreeSet<DataSpaceId>> {
    if let Some(primary) = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias>(
    ) {
        return Some(compare_and_set_primary_account_alias_dataspace_targets(
            primary,
        ));
    }

    if let Some(multisig) = multisig_instruction(instruction) {
        return match &multisig {
            MultisigInstructionBox::Propose(propose) => {
                let mut targets = BTreeSet::new();
                for nested in &propose.instructions {
                    extend_instruction_concrete_dataspace_targets(
                        &mut targets,
                        &**nested,
                        dataspace_catalog,
                        state_view,
                    );
                }
                Some(targets)
            }
            MultisigInstructionBox::Approve(approve) => {
                let mut targets = BTreeSet::new();
                if let Some(proposal) = state_view.and_then(|view| {
                    multisig_proposal_state(
                        view.world(),
                        &approve.account,
                        &approve.instructions_hash,
                    )
                }) {
                    for nested in &proposal.instructions {
                        extend_instruction_concrete_dataspace_targets(
                            &mut targets,
                            &**nested,
                            dataspace_catalog,
                            state_view,
                        );
                    }
                }
                Some(targets)
            }
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => None,
        };
    }

    let register = instruction.as_any().downcast_ref::<RegisterBox>()?;
    let RegisterBox::Trigger(register) = register else {
        return None;
    };
    Some(trigger_executable_concrete_dataspace_targets(
        register.object.action().executable(),
        dataspace_catalog,
        state_view,
    ))
}

fn extend_instruction_concrete_dataspace_targets_with_world<W: WorldReadOnly>(
    targets: &mut BTreeSet<DataSpaceId>,
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) {
    if let Some(nested_targets) = deferred_instruction_concrete_dataspace_targets_with_world(
        instruction,
        dataspace_catalog,
        world,
        ledger_time_ms,
    ) && !nested_targets.is_empty()
    {
        targets.extend(nested_targets);
        return;
    }

    if let Some(target) = instruction_transaction_dataspace_target_with_world(
        instruction,
        dataspace_catalog,
        world,
        ledger_time_ms,
    ) {
        targets.insert(target);
    }
}

fn trigger_executable_concrete_dataspace_targets_with_world<W: WorldReadOnly>(
    executable: &Executable,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> BTreeSet<DataSpaceId> {
    let mut targets = BTreeSet::new();
    match executable {
        Executable::ContractCall(call) => {
            if let Some(target) = contract_address_dataspace_target(&call.contract_address) {
                targets.insert(target);
            }
        }
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                extend_instruction_concrete_dataspace_targets_with_world(
                    &mut targets,
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                );
            }
        }
        Executable::Batch(items) => {
            for item in items {
                match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        extend_instruction_concrete_dataspace_targets_with_world(
                            &mut targets,
                            &**instruction,
                            dataspace_catalog,
                            world,
                            ledger_time_ms,
                        );
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        if let Some(target) =
                            contract_address_dataspace_target(&call.contract_address)
                        {
                            targets.insert(target);
                        }
                    }
                }
            }
        }
        Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                extend_instruction_concrete_dataspace_targets_with_world(
                    &mut targets,
                    &**instruction,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                );
            }
        }
    }
    targets
}

fn deferred_instruction_concrete_dataspace_targets_with_world<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<BTreeSet<DataSpaceId>> {
    if let Some(primary) = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias>(
    ) {
        return Some(compare_and_set_primary_account_alias_dataspace_targets(
            primary,
        ));
    }

    if let Some(multisig) = multisig_instruction(instruction) {
        let instructions = match &multisig {
            MultisigInstructionBox::Propose(propose) => Some(propose.instructions.clone()),
            MultisigInstructionBox::Approve(approve) => {
                multisig_proposal_state(world, &approve.account, &approve.instructions_hash)
                    .map(|proposal| proposal.instructions)
            }
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => None,
        };

        return match instructions {
            Some(instructions) => {
                let mut targets = BTreeSet::new();
                for nested in &instructions {
                    extend_instruction_concrete_dataspace_targets_with_world(
                        &mut targets,
                        &**nested,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    );
                }
                Some(targets)
            }
            None => match multisig {
                MultisigInstructionBox::Approve(_) => Some(BTreeSet::new()),
                MultisigInstructionBox::Propose(_)
                | MultisigInstructionBox::Register(_)
                | MultisigInstructionBox::Cancel(_)
                | MultisigInstructionBox::InvalidateOutstanding(_) => None,
            },
        };
    }

    let register = instruction.as_any().downcast_ref::<RegisterBox>()?;
    let RegisterBox::Trigger(register) = register else {
        return None;
    };
    Some(trigger_executable_concrete_dataspace_targets_with_world(
        register.object.action().executable(),
        dataspace_catalog,
        world,
        ledger_time_ms,
    ))
}

fn same_transaction_multisig_proposal_targets(
    instructions: &[&InstructionBox],
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Vec<SameTransactionMultisigProposalTarget> {
    instructions
        .iter()
        .copied()
        .filter_map(|instruction| match multisig_instruction(&**instruction)? {
            MultisigInstructionBox::Propose(propose) => {
                let dataspace_id = multisig_propose_transaction_dataspace_target(
                    &propose,
                    dataspace_catalog,
                    state_view,
                );
                let mut concrete_dataspaces = BTreeSet::new();
                for nested in &propose.instructions {
                    extend_instruction_concrete_dataspace_targets(
                        &mut concrete_dataspaces,
                        &**nested,
                        dataspace_catalog,
                        state_view,
                    );
                }
                let requires_universal_coordinator = propose.instructions.iter().any(|nested| {
                    instruction_transaction_target_requires_universal_coordinator(
                        &**nested,
                        dataspace_catalog,
                        state_view,
                    )
                });
                Some(SameTransactionMultisigProposalTarget {
                    account: propose.account,
                    instructions_hash: HashOf::new(&propose.instructions),
                    dataspace_id,
                    concrete_dataspaces,
                    requires_universal_coordinator,
                })
            }
            MultisigInstructionBox::Approve(_)
            | MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => None,
        })
        .collect()
}

fn same_transaction_multisig_proposal_targets_with_world<W: WorldReadOnly>(
    instructions: &[&InstructionBox],
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Vec<SameTransactionMultisigProposalTarget> {
    instructions
        .iter()
        .copied()
        .filter_map(|instruction| match multisig_instruction(&**instruction)? {
            MultisigInstructionBox::Propose(propose) => {
                let dataspace_id = multisig_propose_transaction_dataspace_target_with_world(
                    &propose,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                );
                let mut concrete_dataspaces = BTreeSet::new();
                for nested in &propose.instructions {
                    extend_instruction_concrete_dataspace_targets_with_world(
                        &mut concrete_dataspaces,
                        &**nested,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    );
                }
                let requires_universal_coordinator = propose.instructions.iter().any(|nested| {
                    instruction_transaction_target_requires_universal_coordinator_with_world(
                        &**nested,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                });
                Some(SameTransactionMultisigProposalTarget {
                    account: propose.account,
                    instructions_hash: HashOf::new(&propose.instructions),
                    dataspace_id,
                    concrete_dataspaces,
                    requires_universal_coordinator,
                })
            }
            MultisigInstructionBox::Approve(_)
            | MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => None,
        })
        .collect()
}

fn same_transaction_multisig_approve_route_target<'a>(
    proposals: &'a [SameTransactionMultisigProposalTarget],
    instruction: &dyn Instruction,
) -> Option<&'a SameTransactionMultisigProposalTarget> {
    let approve = match multisig_instruction(instruction)? {
        MultisigInstructionBox::Approve(approve) => approve,
        MultisigInstructionBox::Propose(_)
        | MultisigInstructionBox::Register(_)
        | MultisigInstructionBox::Cancel(_)
        | MultisigInstructionBox::InvalidateOutstanding(_) => return None,
    };
    proposals.iter().find(|proposal| {
        proposal.account == approve.account
            && proposal.instructions_hash == approve.instructions_hash
    })
}

fn multisig_propose_transaction_dataspace_target(
    propose: &MultisigPropose,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    merge_instruction_dataspace_targets(propose.instructions.iter().map(|instruction| {
        instruction_transaction_dataspace_target(&**instruction, dataspace_catalog, state_view)
    }))
    .or_else(|| {
        account_dataspace_target(
            state_view.map(StateView::world),
            &propose.account,
            state_view.map(state_view_ledger_time_ms),
        )
    })
}

fn multisig_approve_transaction_dataspace_target(
    approve: &MultisigApprove,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let world = state_view.map(StateView::world)?;
    multisig_proposal_state(world, &approve.account, &approve.instructions_hash)
        .and_then(|proposal_state| {
            merge_instruction_dataspace_targets(proposal_state.instructions.iter().map(
                |instruction| {
                    instruction_transaction_dataspace_target(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    )
                },
            ))
        })
        .or_else(|| {
            account_dataspace_target(
                Some(world),
                &approve.account,
                state_view.map(state_view_ledger_time_ms),
            )
        })
}

fn multisig_approve_transaction_dataspace_target_with_world<W: WorldReadOnly>(
    approve: &MultisigApprove,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    multisig_proposal_state(world, &approve.account, &approve.instructions_hash)
        .and_then(|proposal_state| {
            merge_instruction_dataspace_targets(proposal_state.instructions.iter().map(
                |instruction| {
                    instruction_transaction_dataspace_target_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                },
            ))
        })
        .or_else(|| account_dataspace_target(Some(world), &approve.account, ledger_time_ms))
}

fn multisig_propose_transaction_dataspace_target_with_world<W: WorldReadOnly>(
    propose: &MultisigPropose,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    merge_instruction_dataspace_targets(propose.instructions.iter().map(|instruction| {
        instruction_transaction_dataspace_target_with_world(
            &**instruction,
            dataspace_catalog,
            world,
            ledger_time_ms,
        )
    }))
    .or_else(|| account_dataspace_target(Some(world), &propose.account, ledger_time_ms))
}

fn confidential_asset_definition_target(any: &dyn std::any::Any) -> Option<&AssetDefinitionId> {
    if let Some(topup) = any.downcast_ref::<TopUpKagemushaRecursiveV4>() {
        return Some(topup.request.asset.definition());
    }
    if let Some(redeem) = any.downcast_ref::<RedeemKagemushaRecursiveV4>() {
        return Some(&redeem.request.bundle.statement.asset);
    }
    None
}

fn trigger_executable_requires_universal_coordinator(
    executable: &Executable,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> bool {
    match executable {
        Executable::ContractCall(call) => {
            contract_address_dataspace_target(&call.contract_address)
                == Some(DataSpaceId::UNIVERSAL)
        }
        Executable::Instructions(instructions) => {
            trigger_executable_concrete_dataspace_targets(executable, dataspace_catalog, state_view)
                .len()
                > 1
                || instructions.iter().any(|instruction| {
                    instruction_transaction_target_requires_universal_coordinator(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    )
                })
        }
        Executable::Batch(items) => {
            trigger_executable_concrete_dataspace_targets(executable, dataspace_catalog, state_view)
                .len()
                > 1
                || items.iter().any(|item| match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        instruction_transaction_target_requires_universal_coordinator(
                            &**instruction,
                            dataspace_catalog,
                            state_view,
                        )
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        contract_address_dataspace_target(&call.contract_address)
                            == Some(DataSpaceId::UNIVERSAL)
                    }
                })
        }
        Executable::Ivm(_) => false,
        Executable::IvmProved(proved) => {
            trigger_executable_concrete_dataspace_targets(executable, dataspace_catalog, state_view)
                .len()
                > 1
                || proved.overlay.iter().any(|instruction| {
                    instruction_transaction_target_requires_universal_coordinator(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    )
                })
        }
    }
}

fn trigger_executable_requires_universal_coordinator_with_world<W: WorldReadOnly>(
    executable: &Executable,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> bool {
    match executable {
        Executable::ContractCall(call) => {
            contract_address_dataspace_target(&call.contract_address)
                == Some(DataSpaceId::UNIVERSAL)
        }
        Executable::Instructions(instructions) => {
            trigger_executable_concrete_dataspace_targets_with_world(
                executable,
                dataspace_catalog,
                world,
                ledger_time_ms,
            )
            .len()
                > 1
                || instructions.iter().any(|instruction| {
                    instruction_transaction_target_requires_universal_coordinator_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                })
        }
        Executable::Batch(items) => {
            trigger_executable_concrete_dataspace_targets_with_world(
                executable,
                dataspace_catalog,
                world,
                ledger_time_ms,
            )
            .len()
                > 1
                || items.iter().any(|item| match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        instruction_transaction_target_requires_universal_coordinator_with_world(
                            &**instruction,
                            dataspace_catalog,
                            world,
                            ledger_time_ms,
                        )
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        contract_address_dataspace_target(&call.contract_address)
                            == Some(DataSpaceId::UNIVERSAL)
                    }
                })
        }
        Executable::Ivm(_) => false,
        Executable::IvmProved(proved) => {
            trigger_executable_concrete_dataspace_targets_with_world(
                executable,
                dataspace_catalog,
                world,
                ledger_time_ms,
            )
            .len()
                > 1
                || proved.overlay.iter().any(|instruction| {
                    instruction_transaction_target_requires_universal_coordinator_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                })
        }
    }
}

fn instruction_transaction_target_requires_universal_coordinator(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> bool {
    let any = instruction.as_any();

    if musubi_instruction_requires_universal_coordinator(any) {
        return true;
    }

    if let Some(multisig) = multisig_instruction(instruction) {
        let instructions = match multisig {
            MultisigInstructionBox::Propose(propose) => Some(propose.instructions),
            MultisigInstructionBox::Approve(approve) => state_view
                .and_then(|view| {
                    multisig_proposal_state(
                        view.world(),
                        &approve.account,
                        &approve.instructions_hash,
                    )
                })
                .map(|proposal| proposal.instructions),
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => None,
        };
        let Some(instructions) = instructions else {
            return false;
        };
        let mut concrete_dataspaces = BTreeSet::new();
        for nested in &instructions {
            extend_instruction_concrete_dataspace_targets(
                &mut concrete_dataspaces,
                &**nested,
                dataspace_catalog,
                state_view,
            );
        }
        return concrete_dataspaces.len() > 1
            || instructions.iter().any(|nested| {
                instruction_transaction_target_requires_universal_coordinator(
                    &**nested,
                    dataspace_catalog,
                    state_view,
                )
            });
    }

    if let Some(RegisterBox::Trigger(register)) = any.downcast_ref::<RegisterBox>() {
        return trigger_executable_requires_universal_coordinator(
            register.object.action().executable(),
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(fx) = any.downcast_ref::<SettleFxCorridor>() {
        return fx_corridor_policy_with_state(state_view, &fx.policy_id)
            .is_ok_and(|policy| policy.source_dataspace != policy.destination_dataspace);
    }
    if let Some(SettlementInstructionBox::SettleFxCorridor(fx)) =
        any.downcast_ref::<SettlementInstructionBox>()
    {
        return fx_corridor_policy_with_state(state_view, &fx.policy_id)
            .is_ok_and(|policy| policy.source_dataspace != policy.destination_dataspace);
    }

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

    if let Some(register_zk_asset) = any.downcast_ref::<RegisterZkAsset>() {
        return asset_definition_requires_universal_coordinator(
            &register_zk_asset.asset,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(schedule_transition) = any.downcast_ref::<ScheduleConfidentialPolicyTransition>() {
        return asset_definition_requires_universal_coordinator(
            &schedule_transition.asset,
            dataspace_catalog,
            state_view,
        );
    }

    if let Some(cancel_transition) = any.downcast_ref::<CancelConfidentialPolicyTransition>() {
        return asset_definition_requires_universal_coordinator(
            &cancel_transition.asset,
            dataspace_catalog,
            state_view,
        );
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

    if musubi_instruction_requires_universal_coordinator(any) {
        return true;
    }

    if let Some(multisig) = multisig_instruction(instruction) {
        let instructions = match multisig {
            MultisigInstructionBox::Propose(propose) => Some(propose.instructions),
            MultisigInstructionBox::Approve(approve) => {
                multisig_proposal_state(world, &approve.account, &approve.instructions_hash)
                    .map(|proposal| proposal.instructions)
            }
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => None,
        };
        let Some(instructions) = instructions else {
            return false;
        };
        let mut concrete_dataspaces = BTreeSet::new();
        for nested in &instructions {
            extend_instruction_concrete_dataspace_targets_with_world(
                &mut concrete_dataspaces,
                &**nested,
                dataspace_catalog,
                world,
                ledger_time_ms,
            );
        }
        return concrete_dataspaces.len() > 1
            || instructions.iter().any(|nested| {
                instruction_transaction_target_requires_universal_coordinator_with_world(
                    &**nested,
                    dataspace_catalog,
                    world,
                    ledger_time_ms,
                )
            });
    }

    if let Some(RegisterBox::Trigger(register)) = any.downcast_ref::<RegisterBox>() {
        return trigger_executable_requires_universal_coordinator_with_world(
            register.object.action().executable(),
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(fx) = any.downcast_ref::<SettleFxCorridor>() {
        return fx_corridor_policy_with_world(world, &fx.policy_id)
            .is_ok_and(|policy| policy.source_dataspace != policy.destination_dataspace);
    }
    if let Some(SettlementInstructionBox::SettleFxCorridor(fx)) =
        any.downcast_ref::<SettlementInstructionBox>()
    {
        return fx_corridor_policy_with_world(world, &fx.policy_id)
            .is_ok_and(|policy| policy.source_dataspace != policy.destination_dataspace);
    }

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

    if let Some(register_zk_asset) = any.downcast_ref::<RegisterZkAsset>() {
        return asset_definition_requires_universal_coordinator_with_world(
            &register_zk_asset.asset,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(schedule_transition) = any.downcast_ref::<ScheduleConfidentialPolicyTransition>() {
        return asset_definition_requires_universal_coordinator_with_world(
            &schedule_transition.asset,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    if let Some(cancel_transition) = any.downcast_ref::<CancelConfidentialPolicyTransition>() {
        return asset_definition_requires_universal_coordinator_with_world(
            &cancel_transition.asset,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    false
}

fn account_dataspace_target<W: WorldReadOnly>(
    world: Option<&W>,
    account_id: &AccountId,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let world = world?;
    // A persisted scope-directory entry is the committed routing view. Do not
    // let a partial alias index narrow or override it.
    if world.account_scope_directory().get(account_id).is_some() {
        let hierarchy = world.account_scope_hierarchy(account_id).ok()?;
        let mut dataspaces = hierarchy.keys();
        let dataspace_id = *dataspaces.next()?;
        if dataspaces.next().is_some() {
            return Some(DataSpaceId::UNIVERSAL);
        }
        return (dataspace_id != DataSpaceId::UNIVERSAL).then_some(dataspace_id);
    }

    let account = world.accounts().get(account_id)?;
    let mut dataspaces = BTreeSet::new();
    let mut primary_dataspace = None;

    if let Some(now_ms) = ledger_time_ms {
        if let Some(label) = account.as_ref().label()
            && crate::sns::resolve_active_account_alias(
                world,
                world.dataspace_catalog(),
                label,
                now_ms,
            )
            .as_ref()
                == Some(account_id)
        {
            primary_dataspace = Some(label.dataspace);
            dataspaces.insert(label.dataspace);
        }
        for alias in world.bound_account_aliases(account_id) {
            if crate::sns::resolve_active_account_alias(
                world,
                world.dataspace_catalog(),
                &alias,
                now_ms,
            )
            .as_ref()
                == Some(account_id)
            {
                dataspaces.insert(alias.dataspace);
            }
        }
    }

    if let Some(uaid) = account.as_ref().uaid().copied()
        && let Some(bindings) = world.uaid_dataspaces().get(&uaid)
    {
        dataspaces.extend(bindings.iter().filter_map(|(dataspace, accounts)| {
            accounts.contains(account_id).then_some(*dataspace)
        }));
    }

    if primary_dataspace.is_none_or(|dataspace| dataspace == DataSpaceId::UNIVERSAL) {
        dataspaces.insert(DataSpaceId::UNIVERSAL);
    }
    if dataspaces.len() > 1 {
        return Some(DataSpaceId::UNIVERSAL);
    }
    let dataspace_id = *dataspaces.iter().next()?;
    (dataspace_id != DataSpaceId::UNIVERSAL).then_some(dataspace_id)
}

fn authority_dataspace_target(
    state_view: Option<&StateView<'_>>,
    tx: &dyn TransactionRoutingView,
) -> Option<DataSpaceId> {
    tx.authority_opt().and_then(|authority| {
        account_dataspace_target(
            state_view.map(StateView::world),
            authority,
            state_view.map(state_view_ledger_time_ms),
        )
    })
}

fn authority_dataspace_target_with_world<W: WorldReadOnly>(
    world: Option<&W>,
    tx: &dyn TransactionRoutingView,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    tx.authority_opt()
        .and_then(|authority| account_dataspace_target(world, authority, ledger_time_ms))
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

fn musubi_package_dataspace_target(package: &MusubiPackageIdV1) -> DataSpaceId {
    package.home_dataspace
}

fn musubi_instruction_dataspace_target(any: &dyn core::any::Any) -> Option<DataSpaceId> {
    if let Some(register) = any.downcast_ref::<RegisterMusubiNamespaceBindingV1>() {
        return Some(register.binding.home_dataspace);
    }
    if any.downcast_ref::<RegisterMusubiArchiveV1>().is_some()
        || any
            .downcast_ref::<RegisterMusubiProviderBundleAttestationV1>()
            .is_some()
        || any.downcast_ref::<AddMusubiArchiveLocationV1>().is_some()
        || any
            .downcast_ref::<RetireMusubiArchiveLocationV1>()
            .is_some()
        || any.downcast_ref::<SetMusubiRegistryPolicyV1>().is_some()
    {
        return Some(DataSpaceId::UNIVERSAL);
    }
    if let Some(publish) = any.downcast_ref::<PublishMusubiReleaseV1>() {
        return Some(musubi_package_dataspace_target(
            &publish.publication.manifest.release.package,
        ));
    }
    if let Some(yank) = any.downcast_ref::<SetMusubiReleaseYankV1>() {
        return Some(musubi_package_dataspace_target(&yank.release.package));
    }
    if let Some(metadata) = any.downcast_ref::<SetMusubiPackageMetadataV1>() {
        return Some(musubi_package_dataspace_target(&metadata.package));
    }
    if let Some(invite) = any.downcast_ref::<InviteMusubiPackageMaintainerV1>() {
        return Some(musubi_package_dataspace_target(&invite.package));
    }
    if let Some(accept) = any.downcast_ref::<AcceptMusubiPackageMaintainerV1>() {
        return Some(musubi_package_dataspace_target(&accept.package));
    }
    if let Some(revoke) = any.downcast_ref::<RevokeMusubiPackageMaintainerInvitationV1>() {
        return Some(musubi_package_dataspace_target(&revoke.package));
    }
    if let Some(set_role) = any.downcast_ref::<SetMusubiPackageMaintainerRoleV1>() {
        return Some(musubi_package_dataspace_target(&set_role.package));
    }
    if let Some(remove) = any.downcast_ref::<RemoveMusubiPackageMaintainerV1>() {
        return Some(musubi_package_dataspace_target(&remove.package));
    }
    if let Some(register) = any.downcast_ref::<RegisterMusubiAliasV1>() {
        return Some(musubi_package_dataspace_target(&register.target));
    }
    if let Some(recover) = any.downcast_ref::<RecoverMusubiPackageV1>() {
        return Some(musubi_package_dataspace_target(&recover.package));
    }
    if let Some(retarget) = any.downcast_ref::<RetargetMusubiAliasV1>() {
        return Some(musubi_package_dataspace_target(&retarget.target));
    }
    if let Some(takedown) = any.downcast_ref::<SetMusubiArtifactTakedownV1>() {
        return Some(musubi_package_dataspace_target(&takedown.release.package));
    }
    if let Some(assert) = any.downcast_ref::<AssertMusubiReleaseDigestV1>() {
        return Some(musubi_package_dataspace_target(&assert.release.package));
    }
    None
}

fn musubi_instruction_requires_universal_coordinator(any: &dyn core::any::Any) -> bool {
    any.downcast_ref::<RegisterMusubiNamespaceBindingV1>()
        .is_some()
        || any.downcast_ref::<PublishMusubiReleaseV1>().is_some()
        || any.downcast_ref::<SetMusubiReleaseYankV1>().is_some()
        || any.downcast_ref::<SetMusubiPackageMetadataV1>().is_some()
        || any.downcast_ref::<RegisterMusubiAliasV1>().is_some()
        || any.downcast_ref::<RecoverMusubiPackageV1>().is_some()
        || any.downcast_ref::<RetargetMusubiAliasV1>().is_some()
        || any.downcast_ref::<SetMusubiArtifactTakedownV1>().is_some()
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

fn asset_definition_target_from_parts_with_state(
    asset_definition_id: &AssetDefinitionId,
    owning_domain: Option<&DomainId>,
    balance_scope_policy: Option<AssetBalancePolicy>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let dataspace_alias = state_view
        .and_then(|view| {
            view.world
                .asset_definition_domains()
                .get(asset_definition_id)
                .map(|domain| domain.dataspace().as_ref().to_owned())
        })
        .or_else(|| owning_domain.map(|domain| domain.dataspace().as_ref().to_owned()));
    let Some(dataspace_alias) = dataspace_alias else {
        return balance_scope_policy
            .is_some_and(|policy| policy == AssetBalancePolicy::Global)
            .then_some(DataSpaceId::UNIVERSAL);
    };
    dataspace_alias_target_with_state(&dataspace_alias, dataspace_catalog, state_view)
}

fn asset_definition_target_from_parts_with_world<W: WorldReadOnly>(
    asset_definition_id: &AssetDefinitionId,
    owning_domain: Option<&DomainId>,
    balance_scope_policy: Option<AssetBalancePolicy>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let dataspace_alias = world
        .asset_definition_domains()
        .get(asset_definition_id)
        .map(|domain| domain.dataspace().as_ref().to_owned())
        .or_else(|| owning_domain.map(|domain| domain.dataspace().as_ref().to_owned()));
    let Some(dataspace_alias) = dataspace_alias else {
        return balance_scope_policy
            .is_some_and(|policy| policy == AssetBalancePolicy::Global)
            .then_some(DataSpaceId::UNIVERSAL);
    };
    dataspace_alias_target_with_world(&dataspace_alias, dataspace_catalog, world, ledger_time_ms)
}

fn trigger_executable_transaction_target_needs_state(executable: &Executable) -> bool {
    match executable {
        Executable::Instructions(instructions) => instructions.iter().any(|instruction| {
            instruction_transaction_dataspace_target_needs_state(&**instruction)
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) => false,
        Executable::Batch(items) => items.iter().any(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => {
                instruction_transaction_dataspace_target_needs_state(&**instruction)
            }
            ExecutableBatchItem::ContractCall(_) => false,
        }),
        Executable::IvmProved(proved) => proved.overlay.iter().any(|instruction| {
            instruction_transaction_dataspace_target_needs_state(&**instruction)
        }),
    }
}

fn instruction_transaction_dataspace_target_needs_state(instruction: &dyn Instruction) -> bool {
    let any = instruction.as_any();

    if any.downcast_ref::<SettleFxCorridor>().is_some() {
        return true;
    }

    if let Some(settlement) = any.downcast_ref::<SettlementInstructionBox>()
        && matches!(settlement, SettlementInstructionBox::SettleFxCorridor(_))
    {
        return true;
    }

    if let Some(multisig) = multisig_instruction(instruction) {
        return match multisig {
            MultisigInstructionBox::Propose(_) | MultisigInstructionBox::Approve(_) => true,
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => false,
        };
    }

    if any.downcast_ref::<MultisigPropose>().is_some() {
        return true;
    }

    if any.downcast_ref::<MultisigApprove>().is_some() {
        return true;
    }

    if let Some(RegisterBox::Trigger(register)) = any.downcast_ref::<RegisterBox>() {
        return trigger_executable_transaction_target_needs_state(
            register.object.action().executable(),
        );
    }

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        return match grant {
            GrantBox::Permission(grant) => {
                dataspace_scoped_permission_target_needs_state(&grant.object)
            }
            GrantBox::RolePermission(grant) => {
                dataspace_scoped_permission_target_needs_state(&grant.object)
            }
            GrantBox::Role(_) => false,
        };
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return match revoke {
            RevokeBox::Permission(revoke) => {
                dataspace_scoped_permission_target_needs_state(&revoke.object)
            }
            RevokeBox::RolePermission(revoke) => {
                dataspace_scoped_permission_target_needs_state(&revoke.object)
            }
            RevokeBox::Role(_) => false,
        };
    }

    if any.downcast_ref::<DvpIsi>().is_some() {
        return true;
    }

    if any.downcast_ref::<PvpIsi>().is_some() {
        return true;
    }

    if let Some(settlement) = any.downcast_ref::<SettlementInstructionBox>() {
        return match settlement {
            SettlementInstructionBox::Dvp(_) | SettlementInstructionBox::Pvp(_) => true,
            SettlementInstructionBox::SetFxCorridorPolicy(_) => false,
            SettlementInstructionBox::SettleFxCorridor(_) => true,
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

    if any.downcast_ref::<RegisterZkAsset>().is_some()
        || any
            .downcast_ref::<ScheduleConfidentialPolicyTransition>()
            .is_some()
        || any
            .downcast_ref::<CancelConfidentialPolicyTransition>()
            .is_some()
    {
        return true;
    }

    if confidential_asset_definition_target(any).is_some() {
        return true;
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
            GrantBox::RolePermission(grant) => {
                dataspace_scoped_permission_target(&grant.object, dataspace_catalog, state_view)
            }
            GrantBox::Role(_) => None,
        };
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return match revoke {
            RevokeBox::Permission(revoke) => {
                dataspace_scoped_permission_target(&revoke.object, dataspace_catalog, state_view)
            }
            RevokeBox::RolePermission(revoke) => {
                dataspace_scoped_permission_target(&revoke.object, dataspace_catalog, state_view)
            }
            RevokeBox::Role(_) => None,
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
            GrantBox::RolePermission(grant) => dataspace_scoped_permission_target_with_world(
                &grant.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            GrantBox::Role(_) => None,
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
            RevokeBox::RolePermission(revoke) => dataspace_scoped_permission_target_with_world(
                &revoke.object,
                dataspace_catalog,
                world,
                ledger_time_ms,
            ),
            RevokeBox::Role(_) => None,
        };
    }

    None
}

fn asset_definition_dataspace_target(
    asset_definition_id: &AssetDefinitionId,
    owning_domain: Option<&DomainId>,
    balance_scope_policy: Option<AssetBalancePolicy>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let resolved = state_view
        .and_then(|view| asset_definition_for_routing(&view.world, asset_definition_id))
        .map(|definition| {
            let balance_scope_policy = definition.balance_scope_policy();
            (
                definition.id,
                balance_scope_policy,
                definition.owning_domain,
            )
        });
    let effective_id = resolved
        .as_ref()
        .map(|(resolved_id, _, _)| resolved_id)
        .unwrap_or(asset_definition_id);
    let effective_owning_domain = resolved
        .as_ref()
        .and_then(|(_, _, resolved_domain)| resolved_domain.as_ref())
        .or(owning_domain);
    let effective_policy = resolved
        .as_ref()
        .map(|(_, policy, _)| *policy)
        .or(balance_scope_policy);
    asset_definition_target_from_parts_with_state(
        effective_id,
        effective_owning_domain,
        effective_policy,
        dataspace_catalog,
        state_view,
    )
}

fn asset_definition_dataspace_target_with_world<W: WorldReadOnly>(
    asset_definition_id: &AssetDefinitionId,
    owning_domain: Option<&DomainId>,
    balance_scope_policy: Option<AssetBalancePolicy>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    let resolved = asset_definition_for_routing(world, asset_definition_id).map(|definition| {
        let balance_scope_policy = definition.balance_scope_policy();
        (
            definition.id,
            balance_scope_policy,
            definition.owning_domain,
        )
    });
    let effective_id = resolved
        .as_ref()
        .map(|(resolved_id, _, _)| resolved_id)
        .unwrap_or(asset_definition_id);
    let effective_owning_domain = resolved
        .as_ref()
        .and_then(|(_, _, resolved_domain)| resolved_domain.as_ref())
        .or(owning_domain);
    let effective_policy = resolved
        .as_ref()
        .map(|(_, policy, _)| *policy)
        .or(balance_scope_policy);
    asset_definition_target_from_parts_with_world(
        effective_id,
        effective_owning_domain,
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
            (
                definition.id,
                balance_scope_policy,
                definition.owning_domain,
            )
        });
    let effective_id = resolved
        .as_ref()
        .map(|(resolved_id, _, _)| resolved_id)
        .unwrap_or(asset_definition_id);
    let effective_owning_domain = resolved
        .as_ref()
        .and_then(|(_, _, resolved_domain)| resolved_domain.as_ref());
    let effective_policy = resolved.as_ref().map(|(_, policy, _)| *policy);
    let dataspace_id = asset_definition_target_from_parts_with_state(
        effective_id,
        effective_owning_domain,
        effective_policy,
        dataspace_catalog,
        state_view,
    );
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
            (
                definition.id,
                balance_scope_policy,
                definition.owning_domain,
            )
        });
    let effective_id = resolved
        .as_ref()
        .map(|(resolved_id, _, _)| resolved_id)
        .unwrap_or(asset_definition_id);
    let effective_owning_domain = resolved
        .as_ref()
        .and_then(|(_, _, resolved_domain)| resolved_domain.as_ref());
    let effective_policy = resolved.as_ref().map(|(_, policy, _)| *policy);
    let dataspace_id = asset_definition_target_from_parts_with_world(
        effective_id,
        effective_owning_domain,
        effective_policy,
        dataspace_catalog,
        world,
        ledger_time_ms,
    );
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
    world.asset_definition(asset_definition_id).ok()
}

fn asset_definition_for_balance_routing<W: WorldReadOnly>(
    world: &W,
    asset_definition_id: &AssetDefinitionId,
) -> Option<AssetDefinition> {
    let mut definition = world.asset_definition(asset_definition_id).ok()?;

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
        AccountAliasPermissionScope::Alias(alias) => Some(alias.dataspace_id),
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
        AccountAliasPermissionScope::Alias(alias) => Some(alias.dataspace_id),
    }
}

fn asset_definition_alias_permission_scope_dataspace_target_with_state(
    scope: &AssetDefinitionAliasPermissionScope,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    match scope {
        AssetDefinitionAliasPermissionScope::Domain(domain_id) => {
            domain_dataspace_target_with_state(domain_id, dataspace_catalog, state_view)
        }
        AssetDefinitionAliasPermissionScope::Dataspace(dataspace_id) => Some(*dataspace_id),
        AssetDefinitionAliasPermissionScope::Alias(alias) => Some(alias.dataspace_id),
    }
}

fn asset_definition_alias_permission_scope_dataspace_target_with_world<W: WorldReadOnly>(
    scope: &AssetDefinitionAliasPermissionScope,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    match scope {
        AssetDefinitionAliasPermissionScope::Domain(domain_id) => {
            domain_dataspace_target_with_world(domain_id, dataspace_catalog, world, ledger_time_ms)
        }
        AssetDefinitionAliasPermissionScope::Dataspace(dataspace_id) => Some(*dataspace_id),
        AssetDefinitionAliasPermissionScope::Alias(alias) => Some(alias.dataspace_id),
    }
}

fn dataspace_scoped_permission_target_needs_state(permission: &Permission) -> bool {
    match permission.name() {
        "CanMintAssetToAccount" => permission
            .payload()
            .try_into_any_norito::<CanMintAssetToAccount>()
            .ok()
            .is_some(),
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
        "CanManageAssetDefinitionAlias" => permission
            .payload()
            .try_into_any_norito::<CanManageAssetDefinitionAlias>()
            .ok()
            .is_some(),
        "CanManageFeeSponsorProgram" => permission
            .payload()
            .try_into_any_norito::<CanManageFeeSponsorProgram>()
            .ok()
            .is_some(),
        "CanEnrollFeeSponsorProgram" => permission
            .payload()
            .try_into_any_norito::<CanEnrollFeeSponsorProgram>()
            .ok()
            .is_some(),
        "CanWithdrawFeeSponsorProgram" => permission
            .payload()
            .try_into_any_norito::<CanWithdrawFeeSponsorProgram>()
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
    if permission.name() != "CanPublishSpaceDirectoryManifest"
        && permission.name() != "CanPublishSpaceDirectoryManifestForUaid"
        && permission.name() != "CanPublishSpaceDirectoryManifestForAccountDomain"
    {
        return match permission.name() {
            "CanMintAssetToAccount" => permission
                .payload()
                .try_into_any_norito::<CanMintAssetToAccount>()
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
            "CanManageAssetDefinitionAlias" => permission
                .payload()
                .try_into_any_norito::<CanManageAssetDefinitionAlias>()
                .ok()
                .and_then(|token| {
                    asset_definition_alias_permission_scope_dataspace_target_with_state(
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
            "CanDelegateAccountAliasResolution" => permission
                .payload()
                .try_into_any_norito::<CanDelegateAccountAliasResolution>()
                .ok()
                .and_then(|token| {
                    account_alias_permission_scope_dataspace_target_with_state(
                        &token.scope,
                        dataspace_catalog,
                        state_view,
                    )
                }),
            "CanManageFeeSponsorProgram" => permission
                .payload()
                .try_into_any_norito::<CanManageFeeSponsorProgram>()
                .ok()
                .and_then(|token| {
                    account_dataspace_target(
                        state_view.map(StateView::world),
                        &token.sponsor,
                        state_view.map(state_view_ledger_time_ms),
                    )
                }),
            "CanEnrollFeeSponsorProgram" => permission
                .payload()
                .try_into_any_norito::<CanEnrollFeeSponsorProgram>()
                .ok()
                .and_then(|token| {
                    account_dataspace_target(
                        state_view.map(StateView::world),
                        &token.program_id.sponsor,
                        state_view.map(state_view_ledger_time_ms),
                    )
                }),
            "CanWithdrawFeeSponsorProgram" => permission
                .payload()
                .try_into_any_norito::<CanWithdrawFeeSponsorProgram>()
                .ok()
                .and_then(|token| {
                    account_dataspace_target(
                        state_view.map(StateView::world),
                        &token.program_id.sponsor,
                        state_view.map(state_view_ledger_time_ms),
                    )
                }),
            _ => None,
        };
    }

    match permission.name() {
        "CanPublishSpaceDirectoryManifest" => permission
            .payload()
            .try_into_any_norito::<CanPublishSpaceDirectoryManifest>()
            .ok()
            .map(|token| token.dataspace),
        "CanPublishSpaceDirectoryManifestForUaid" => permission
            .payload()
            .try_into_any_norito::<CanPublishSpaceDirectoryManifestForUaid>()
            .ok()
            .map(|token| token.dataspace),
        "CanPublishSpaceDirectoryManifestForAccountDomain" => permission
            .payload()
            .try_into_any_norito::<CanPublishSpaceDirectoryManifestForAccountDomain>()
            .ok()
            .map(|token| token.dataspace),
        _ => None,
    }
}

fn dataspace_scoped_permission_target_with_world<W: WorldReadOnly>(
    permission: &Permission,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> Option<DataSpaceId> {
    if permission.name() != "CanPublishSpaceDirectoryManifest"
        && permission.name() != "CanPublishSpaceDirectoryManifestForUaid"
        && permission.name() != "CanPublishSpaceDirectoryManifestForAccountDomain"
    {
        return match permission.name() {
            "CanMintAssetToAccount" => permission
                .payload()
                .try_into_any_norito::<CanMintAssetToAccount>()
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
            "CanManageAssetDefinitionAlias" => permission
                .payload()
                .try_into_any_norito::<CanManageAssetDefinitionAlias>()
                .ok()
                .and_then(|token| {
                    asset_definition_alias_permission_scope_dataspace_target_with_world(
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
            "CanDelegateAccountAliasResolution" => permission
                .payload()
                .try_into_any_norito::<CanDelegateAccountAliasResolution>()
                .ok()
                .and_then(|token| {
                    account_alias_permission_scope_dataspace_target_with_world(
                        &token.scope,
                        dataspace_catalog,
                        world,
                        ledger_time_ms,
                    )
                }),
            "CanManageFeeSponsorProgram" => permission
                .payload()
                .try_into_any_norito::<CanManageFeeSponsorProgram>()
                .ok()
                .and_then(|token| {
                    account_dataspace_target(Some(world), &token.sponsor, ledger_time_ms)
                }),
            "CanEnrollFeeSponsorProgram" => permission
                .payload()
                .try_into_any_norito::<CanEnrollFeeSponsorProgram>()
                .ok()
                .and_then(|token| {
                    account_dataspace_target(Some(world), &token.program_id.sponsor, ledger_time_ms)
                }),
            "CanWithdrawFeeSponsorProgram" => permission
                .payload()
                .try_into_any_norito::<CanWithdrawFeeSponsorProgram>()
                .ok()
                .and_then(|token| {
                    account_dataspace_target(Some(world), &token.program_id.sponsor, ledger_time_ms)
                }),
            _ => None,
        };
    }

    match permission.name() {
        "CanPublishSpaceDirectoryManifest" => permission
            .payload()
            .try_into_any_norito::<CanPublishSpaceDirectoryManifest>()
            .ok()
            .map(|token| token.dataspace),
        "CanPublishSpaceDirectoryManifestForUaid" => permission
            .payload()
            .try_into_any_norito::<CanPublishSpaceDirectoryManifestForUaid>()
            .ok()
            .map(|token| token.dataspace),
        "CanPublishSpaceDirectoryManifestForAccountDomain" => permission
            .payload()
            .try_into_any_norito::<CanPublishSpaceDirectoryManifestForAccountDomain>()
            .ok()
            .map(|token| token.dataspace),
        _ => None,
    }
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
            GrantBox::RolePermission(grant) => {
                dataspace_scoped_permission_target_needs_state(&grant.object)
            }
            GrantBox::Role(_) => false,
        };
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        return match revoke {
            RevokeBox::Permission(revoke) => {
                dataspace_scoped_permission_target_needs_state(&revoke.object)
            }
            RevokeBox::RolePermission(revoke) => {
                dataspace_scoped_permission_target_needs_state(&revoke.object)
            }
            RevokeBox::Role(_) => false,
        };
    }

    false
}

fn dataspace_scoped_permission_routing_requires_state(tx: &dyn TransactionRoutingView) -> bool {
    let Some(executable) = transaction_executable(tx) else {
        return false;
    };

    match executable {
        Executable::Instructions(instructions) => instructions.iter().any(|instruction| {
            instruction_dataspace_scoped_permission_target_needs_state(&**instruction)
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) => false,
        Executable::Batch(items) => items.iter().any(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => {
                instruction_dataspace_scoped_permission_target_needs_state(&**instruction)
            }
            ExecutableBatchItem::ContractCall(_) => false,
        }),
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
        .filter(|lane| is_canonical_dataspace_lane(lane, dataspace_id))
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

fn is_canonical_dataspace_lane(
    lane: &iroha_data_model::nexus::LaneConfig,
    dataspace_id: DataSpaceId,
) -> bool {
    lane.dataspace_id == dataspace_id && !lane_uses_reserved_autoscale_metadata(lane)
}

fn lane_uses_reserved_autoscale_metadata(lane: &iroha_data_model::nexus::LaneConfig) -> bool {
    lane.metadata.contains_key(AUTOSCALE_META_MANAGED)
        || lane.metadata.contains_key(AUTOSCALE_META_CREATED_HEIGHT)
        || lane.metadata.contains_key(AUTOSCALE_META_DRAIN_STATE)
        || lane.metadata.contains_key(AUTOSCALE_META_COMMITTEE)
}

fn reject_autoscale_owned_rule_lane(
    rule: &LaneRoutingRule,
    lane_catalog: &LaneCatalog,
) -> Result<(), RoutingResolveError> {
    if lane_catalog
        .lanes()
        .iter()
        .any(|lane| lane.id == rule.lane && lane_uses_reserved_autoscale_metadata(lane))
    {
        return Err(RoutingResolveError::AutoscaleOwnedRuleLane { lane_id: rule.lane });
    }
    Ok(())
}

fn is_autoscale_managed_elastic_lane(lane: &iroha_data_model::nexus::LaneConfig) -> bool {
    lane.is_autoscale_managed_elastic()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AutoscaleElasticRange {
    min_lanes: u32,
    max_lanes: u32,
    current_height: Option<u64>,
}

impl AutoscaleElasticRange {
    fn from_nexus(nexus: &iroha_config::parameters::actual::Nexus) -> Self {
        let min_lanes = nexus.autoscale.min_lanes.get();
        let mut max_lanes = min_lanes;
        if nexus.enabled && nexus.autoscale.enabled {
            let configured_max_lanes = nexus.autoscale.max_lanes.get();
            let default_lane = nexus.routing_policy.default_lane.as_u32();
            let cap = iroha_config::parameters::defaults::nexus::autoscale::MAX_LANES;
            if min_lanes < configured_max_lanes
                && configured_max_lanes <= cap
                && default_lane < min_lanes
            {
                max_lanes = configured_max_lanes;
            }
        }
        Self {
            min_lanes,
            max_lanes,
            current_height: None,
        }
    }

    fn from_nexus_at_height(
        nexus: &iroha_config::parameters::actual::Nexus,
        current_height: u64,
    ) -> Self {
        Self {
            current_height: Some(current_height),
            ..Self::from_nexus(nexus)
        }
    }

    const fn contains_lane(self, lane_id: LaneId) -> bool {
        let lane = lane_id.as_u32();
        lane >= self.min_lanes && lane < self.max_lanes
    }

    fn lane_created_height_active(self, lane: &iroha_data_model::nexus::LaneConfig) -> bool {
        self.current_height.is_none_or(|current_height| {
            lane.autoscale_created_height()
                .is_some_and(|created_height| created_height <= current_height)
        })
    }

    fn contains_active_elastic_lane(self, lane: &iroha_data_model::nexus::LaneConfig) -> bool {
        self.contains_lane(lane.id)
            && is_autoscale_managed_elastic_lane(lane)
            && self.lane_created_height_active(lane)
            && crate::state::autoscale_lane_accepts_proposal_height(
                lane,
                self.current_height.unwrap_or(u64::MAX),
            )
    }
}

fn default_route_elastic_candidates(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    autoscale_range: Option<AutoscaleElasticRange>,
) -> Vec<LaneId> {
    let Some(default_lane) = lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == policy.default_lane)
    else {
        return Vec::new();
    };
    if default_lane.dataspace_id != policy.default_dataspace {
        return Vec::new();
    }

    let Some(autoscale_range) = autoscale_range else {
        return vec![policy.default_lane];
    };

    if lane_catalog.lanes().iter().any(|lane| {
        lane.id != policy.default_lane
            && autoscale_range.contains_lane(lane.id)
            && (lane.dataspace_id != policy.default_dataspace
                || !autoscale_range.contains_active_elastic_lane(lane))
    }) {
        return vec![policy.default_lane];
    }

    let mut candidates = vec![policy.default_lane];
    candidates.extend(
        lane_catalog
            .lanes()
            .iter()
            .filter(|lane| {
                lane.id != policy.default_lane
                    && lane.dataspace_id == policy.default_dataspace
                    && autoscale_range.contains_active_elastic_lane(lane)
            })
            .map(|lane| lane.id),
    );
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn insert_height_active_routable_lane(
    lanes: &mut BTreeSet<LaneId>,
    route: RoutingDecision,
    nexus: &Nexus,
    block_height: u64,
) {
    if crate::state::nexus_active_lane_dataspace_at_height(route.lane_id, nexus, block_height)
        == Some(route.dataspace_id)
    {
        lanes.insert(route.lane_id);
    }
}

/// Resolve the set of lanes that the configured Nexus routing policy can select at a block height.
pub(crate) fn routable_lane_ids_for_nexus_at_height(
    nexus: &Nexus,
    block_height: u64,
) -> BTreeSet<LaneId> {
    let mut lane_ids = BTreeSet::new();
    if !nexus.enabled {
        return lane_ids;
    }

    let policy = &nexus.routing_policy;
    let lane_catalog = &nexus.lane_catalog;
    let dataspace_catalog = &nexus.dataspace_catalog;

    let default_lane_can_anchor = lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == policy.default_lane)
        .is_some_and(|lane| !lane_uses_reserved_autoscale_metadata(lane));
    if default_lane_can_anchor {
        for lane_id in default_route_elastic_candidates(
            policy,
            lane_catalog,
            Some(AutoscaleElasticRange::from_nexus_at_height(
                nexus,
                block_height,
            )),
        ) {
            if let Ok(route) = resolve_routing_decision(
                RoutingDecision::new(lane_id, policy.default_dataspace),
                lane_catalog,
                dataspace_catalog,
            ) {
                insert_height_active_routable_lane(&mut lane_ids, route, nexus, block_height);
            }
        }
    }

    for dataspace in dataspace_catalog.entries() {
        if let Ok(route) = canonical_dataspace_route(dataspace.id, lane_catalog, dataspace_catalog)
        {
            insert_height_active_routable_lane(&mut lane_ids, route, nexus, block_height);
        }
    }

    for rule in &policy.rules {
        if reject_autoscale_owned_rule_lane(rule, lane_catalog).is_err() {
            continue;
        }
        let dataspace_id = rule.dataspace.unwrap_or(policy.default_dataspace);
        if let Ok(route) = resolve_routing_decision(
            RoutingDecision::new(rule.lane, dataspace_id),
            lane_catalog,
            dataspace_catalog,
        ) {
            insert_height_active_routable_lane(&mut lane_ids, route, nexus, block_height);
        }
    }

    lane_ids
}

fn default_route_shard_index(tx: &dyn TransactionRoutingView, lane_count: usize) -> usize {
    let hash = tx.routing_hash();
    let mut bytes = [0_u8; core::mem::size_of::<u64>()];
    bytes.copy_from_slice(&hash.as_ref()[..core::mem::size_of::<u64>()]);
    let shard = u64::from_le_bytes(bytes);
    (shard % lane_count as u64) as usize
}

fn resolve_default_routing_decision(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: Option<&dyn TransactionRoutingView>,
    autoscale_range: Option<AutoscaleElasticRange>,
) -> Result<RoutingDecision, RoutingResolveError> {
    if lane_catalog
        .lanes()
        .iter()
        .any(|lane| lane.id == policy.default_lane && lane_uses_reserved_autoscale_metadata(lane))
    {
        return Err(RoutingResolveError::AutoscaleOwnedDefaultLane {
            lane_id: policy.default_lane,
        });
    }

    if let Some(tx) = tx {
        let candidates = default_route_elastic_candidates(policy, lane_catalog, autoscale_range);
        if candidates.len() > 1 {
            let lane_id = candidates[default_route_shard_index(tx, candidates.len())];
            return resolve_routing_decision(
                RoutingDecision::new(lane_id, policy.default_dataspace),
                lane_catalog,
                dataspace_catalog,
            );
        }
    }

    resolve_routing_decision(
        RoutingDecision::new(policy.default_lane, policy.default_dataspace),
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
    tx: Option<&dyn TransactionRoutingView>,
    autoscale_range: Option<AutoscaleElasticRange>,
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
        tx,
        autoscale_range,
    )?;
    Ok(RoutingPlan::single(decision))
}

fn add_smart_contract_deploy_policy_participant(
    target: &mut TransactionDataspaceTarget,
    matched_rule: Option<&LaneRoutingRule>,
) {
    let Some(rule_dataspace) = smart_contract_deploy_policy_dataspace(matched_rule) else {
        return;
    };
    let target_is_universal = target.dataspace_id == Some(DataSpaceId::UNIVERSAL);
    if target.participants.is_empty() && !target_is_universal {
        return;
    }
    if !target.participants.is_empty()
        && target
            .participants
            .iter()
            .all(|participant| *participant == rule_dataspace)
    {
        return;
    }

    target.participants.insert(rule_dataspace);
    target.dataspace_id = Some(DataSpaceId::UNIVERSAL);
    if target_is_universal {
        target.coordinator_route = true;
    }
}

fn smart_contract_deploy_policy_dataspace(
    matched_rule: Option<&LaneRoutingRule>,
) -> Option<DataSpaceId> {
    let rule = matched_rule?;
    let dataspace = rule.dataspace?;
    (dataspace != DataSpaceId::UNIVERSAL && rule_matches_smart_contract_deploy(rule))
        .then_some(dataspace)
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
    tx: Option<&dyn TransactionRoutingView>,
    autoscale_range: Option<AutoscaleElasticRange>,
) -> Result<RoutingDecision, RoutingResolveError> {
    if target_is_coordinator_route && target_dataspace == Some(DataSpaceId::UNIVERSAL) {
        return canonical_dataspace_route(DataSpaceId::UNIVERSAL, lane_catalog, dataspace_catalog);
    }

    if let Some(dataspace_id) = target_dataspace {
        if let Some(rule) = matched_rule {
            reject_autoscale_owned_rule_lane(rule, lane_catalog)?;
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
        reject_autoscale_owned_rule_lane(rule, lane_catalog)?;
        let decision = RoutingDecision::new(
            rule.lane,
            rule.dataspace.unwrap_or(policy.default_dataspace),
        );
        return resolve_routing_decision(decision, lane_catalog, dataspace_catalog);
    }

    resolve_default_routing_decision(policy, lane_catalog, dataspace_catalog, tx, autoscale_range)
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
        let target_dataspace = account_dataspace_target(
            Some(state_view.world()),
            authority,
            Some(state_view_ledger_time_ms(state_view)),
        );
        return resolve_policy_routing_decision(
            policy,
            matched_rule,
            target_dataspace,
            target_dataspace == Some(DataSpaceId::UNIVERSAL),
            lane_catalog,
            dataspace_catalog,
            None,
            Some(AutoscaleElasticRange::from_nexus_at_height(
                state_view.nexus(),
                u64::try_from(state_view.height()).unwrap_or(u64::MAX),
            )),
        );
    }
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| query_rule_matches(rule, authority, None));
    resolve_policy_routing_decision(
        policy,
        matched_rule,
        None,
        false,
        lane_catalog,
        dataspace_catalog,
        None,
        None,
    )
}

fn resolve_query_routing_decision_with_world<W: WorldReadOnly>(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    authority: &AccountId,
    world: &W,
    ledger_time_ms: Option<u64>,
    autoscale_range: Option<AutoscaleElasticRange>,
) -> Result<RoutingDecision, RoutingResolveError> {
    let matched_rule = policy.rules.iter().find(|rule| {
        query_rule_matches_with_world(rule, authority, dataspace_catalog, world, ledger_time_ms)
    });
    let target_dataspace = account_dataspace_target(Some(world), authority, ledger_time_ms);
    resolve_policy_routing_decision(
        policy,
        matched_rule,
        target_dataspace,
        target_dataspace == Some(DataSpaceId::UNIVERSAL),
        lane_catalog,
        dataspace_catalog,
        None,
        autoscale_range,
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
        .find(|lane| {
            lane.id == LaneId::SINGLE
                && lane.dataspace_id == DataSpaceId::UNIVERSAL
                && !lane_uses_reserved_autoscale_metadata(lane)
        })
        .map(|lane| lane.id)
}

fn rule_matches(
    rule: &LaneRoutingRule,
    tx: &dyn TransactionRoutingView,
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
    tx: &dyn TransactionRoutingView,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> bool {
    let matcher = &rule.matcher;

    if let Some(account) = matcher.account.as_deref()
        && !tx.authority_opt().is_some_and(|authority| {
            account_matches_with_world(account, authority, dataspace_catalog, world, ledger_time_ms)
        })
    {
        return false;
    }

    if let Some(instruction) = matcher.instruction.as_deref()
        && !instructions_match_with_world(instruction, tx, dataspace_catalog, world, ledger_time_ms)
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
    ledger_time_ms: Option<u64>,
) -> bool {
    if rule.matcher.instruction.is_some() {
        return false;
    }

    rule.matcher.account.as_deref().map_or(true, |account| {
        account_matches_with_world(account, authority, dataspace_catalog, world, ledger_time_ms)
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
        Some(state_view_ledger_time_ms(state_view)),
    )
}

fn account_matches_with_world<W: WorldReadOnly>(
    pattern: &str,
    authority: &AccountId,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }

    if account_matches_literal_or_encoded(pattern, authority) {
        return true;
    }

    if let Some(scope) = pattern.strip_prefix("*@") {
        return account_matches_alias_scope_with_world(
            scope,
            authority,
            dataspace_catalog,
            world,
            ledger_time_ms,
        );
    }

    AccountAlias::from_literal(pattern, dataspace_catalog)
        .ok()
        .is_some_and(|alias| {
            ledger_time_ms.is_some_and(|now_ms| {
                crate::sns::resolve_active_account_alias(world, dataspace_catalog, &alias, now_ms)
                    .as_ref()
                    == Some(authority)
            })
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
        Some(state_view_ledger_time_ms(state_view)),
    )
}

fn account_matches_alias_scope_with_world<W: WorldReadOnly>(
    scope: &str,
    account_id: &AccountId,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> bool {
    let scope = scope.trim().to_ascii_lowercase();
    if scope.is_empty() {
        return false;
    }

    // When the committed directory has an entry, it is authoritative: malformed
    // or missing scope material must not fall through to a partial alias index.
    if world.account_scope_directory().get(account_id).is_some() {
        return world
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
            });
    }

    let Some(now_ms) = ledger_time_ms else {
        return false;
    };
    world
        .bound_account_aliases(account_id)
        .into_iter()
        .any(|alias| {
            if crate::sns::resolve_active_account_alias(world, dataspace_catalog, &alias, now_ms)
                .as_ref()
                != Some(account_id)
            {
                return false;
            }
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
    tx: &dyn TransactionRoutingView,
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

    tx.any_matching_instruction(&mut |instruction| {
        instruction_matches(matcher_label, destination_scope, instruction, state_view)
    })
}

fn instructions_match_with_world<W: WorldReadOnly>(
    matcher: &str,
    tx: &dyn TransactionRoutingView,
    dataspace_catalog: &DataSpaceCatalog,
    world: &W,
    ledger_time_ms: Option<u64>,
) -> bool {
    let matcher_norm = matcher.trim().to_ascii_lowercase();
    if matcher_norm.is_empty() {
        return false;
    }
    let (matcher_label, destination_scope) = split_instruction_matcher(&matcher_norm);
    if matcher_label.is_empty() {
        return false;
    }

    tx.any_matching_instruction(&mut |instruction| {
        instruction_matches_with_world(
            matcher_label,
            destination_scope,
            instruction,
            dataspace_catalog,
            world,
            ledger_time_ms,
        )
    })
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
    ledger_time_ms: Option<u64>,
) -> bool {
    if destination_scope.is_some_and(|scope| {
        !transfer_destination_matches_alias_scope_with_world(
            instruction,
            scope,
            dataspace_catalog,
            world,
            ledger_time_ms,
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
    ledger_time_ms: Option<u64>,
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
    account_matches_alias_scope_with_world(
        scope,
        destination,
        dataspace_catalog,
        world,
        ledger_time_ms,
    )
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
    state_view
        .and_then(|view| {
            view.world
                .asset_definition_domains()
                .get(asset_definition_id)
                .cloned()
        })
        .is_some_and(|domain_id| domain_scope_matches(scope, &domain_id))
}

fn asset_definition_scope_matches_with_world<W: WorldReadOnly>(
    scope: &str,
    asset_definition_id: &AssetDefinitionId,
    world: &W,
) -> bool {
    world
        .asset_definition_domains()
        .get(asset_definition_id)
        .cloned()
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

    if any.is::<RegisterSmartContractCode>()
        || any.is::<RegisterSmartContractBytes>()
        || any.is::<UploadSmartContractCodeChunk>()
        || any.is::<FinalizeSmartContractCodeUpload>()
        || any.is::<CommitContractDeployment>()
    {
        return matches_label(matcher, "smartcontract::deploy")
            || matches_label(matcher, "smart_contract::deploy");
    }

    false
}

fn matches_box_variant(matcher: &str, base: &str, variant: &str) -> bool {
    matches_label(matcher, base) || matches_label(matcher, variant)
}

fn matches_zk_instruction_label(matcher: &str, variant: &str) -> bool {
    matches_label(matcher, "zk")
        || matches_label(matcher, variant)
        || matches_label(matcher, &format!("zk::{variant}"))
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
    fn route(&self, tx: &dyn TransactionRoutingView) -> RoutingDecision;

    /// Route the given transaction using an already acquired state view.
    ///
    /// Routers that require dynamic world-state can override this method and
    /// [`LaneRouter::route_without_state`].
    fn route_with_view(
        &self,
        tx: &dyn TransactionRoutingView,
        _state_view: &StateView<'_>,
    ) -> RoutingDecision {
        self.route(tx)
    }

    /// Route the given transaction with narrow state access when possible.
    ///
    /// The default implementation prefers [`LaneRouter::route_without_state`]
    /// and only falls back to taking a short-lived [`StateView`] when needed.
    fn route_with_state(&self, tx: &dyn TransactionRoutingView, state: &State) -> RoutingDecision {
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
    fn route_without_state(&self, tx: &dyn TransactionRoutingView) -> Option<RoutingDecision> {
        Some(self.route(tx))
    }

    /// Route the given transaction and return deterministic route-resolution errors.
    fn try_route(
        &self,
        tx: &dyn TransactionRoutingView,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        Ok(self.route(tx))
    }

    /// Route with an existing state view and return deterministic route-resolution errors.
    fn try_route_with_view(
        &self,
        tx: &dyn TransactionRoutingView,
        state_view: &StateView<'_>,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        Ok(self.route_with_view(tx, state_view))
    }

    /// Route with narrow state access and return deterministic route-resolution errors.
    fn try_route_with_state(
        &self,
        tx: &dyn TransactionRoutingView,
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
        tx: &dyn TransactionRoutingView,
    ) -> Result<Option<RoutingDecision>, RoutingResolveError> {
        Ok(self.route_without_state(tx))
    }

    /// Build the full routing plan for a transaction and return deterministic errors.
    fn try_route_plan(
        &self,
        tx: &dyn TransactionRoutingView,
    ) -> Result<RoutingPlan, RoutingResolveError> {
        self.try_route(tx)
            .map(|route| RoutingPlan::Single(RouteLeg::new(route, RouteLegRole::Coordinator)))
    }

    /// Build the full routing plan with an existing state view.
    fn try_route_plan_with_view(
        &self,
        tx: &dyn TransactionRoutingView,
        state_view: &StateView<'_>,
    ) -> Result<RoutingPlan, RoutingResolveError> {
        self.try_route_with_view(tx, state_view)
            .map(|route| RoutingPlan::Single(RouteLeg::new(route, RouteLegRole::Coordinator)))
    }

    /// Build the full routing plan with narrow state access when possible.
    fn try_route_plan_with_state(
        &self,
        tx: &dyn TransactionRoutingView,
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
        tx: &dyn TransactionRoutingView,
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
    fn route(&self, _tx: &dyn TransactionRoutingView) -> RoutingDecision {
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
        tx: &dyn TransactionRoutingView,
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
    fn route(&self, tx: &dyn TransactionRoutingView) -> RoutingDecision {
        evaluate_policy(&self.policy, tx)
    }

    fn route_with_view(
        &self,
        tx: &dyn TransactionRoutingView,
        state_view: &StateView<'_>,
    ) -> RoutingDecision {
        self.try_route_with_view(tx, state_view)
            .unwrap_or_else(|_| {
                fail_closed_policy_route_with_view(
                    &state_view.nexus().routing_policy,
                    tx,
                    state_view,
                )
            })
    }

    fn route_without_state(&self, tx: &dyn TransactionRoutingView) -> Option<RoutingDecision> {
        self.try_route_without_state(tx).ok().flatten()
    }

    fn try_route(
        &self,
        tx: &dyn TransactionRoutingView,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        if transaction_contains_fx_corridor_settlement(tx)
            && let Some(decision) = settlement_routing_decision(
                tx,
                self.lane_catalog.as_ref(),
                self.dataspace_catalog.as_ref(),
                None,
            )?
        {
            return Ok(decision);
        }
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
            Some(tx),
            None,
        )
    }

    fn try_route_plan(
        &self,
        tx: &dyn TransactionRoutingView,
    ) -> Result<RoutingPlan, RoutingResolveError> {
        if transaction_contains_fx_corridor_settlement(tx)
            && let Some(decision) = settlement_routing_decision(
                tx,
                self.lane_catalog.as_ref(),
                self.dataspace_catalog.as_ref(),
                None,
            )?
        {
            return Ok(RoutingPlan::single(decision));
        }
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
            Some(tx),
            None,
        )
    }

    fn try_route_with_view(
        &self,
        tx: &dyn TransactionRoutingView,
        state_view: &StateView<'_>,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        let nexus = state_view.nexus();
        if transaction_contains_fx_corridor_settlement(tx)
            && let Some(decision) = settlement_routing_decision(
                tx,
                &nexus.lane_catalog,
                &nexus.dataspace_catalog,
                Some(state_view),
            )?
        {
            return Ok(decision);
        }
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
            Some(tx),
            Some(AutoscaleElasticRange::from_nexus_at_height(
                nexus,
                u64::try_from(state_view.height()).unwrap_or(u64::MAX),
            )),
        )
    }

    fn try_route_plan_with_view(
        &self,
        tx: &dyn TransactionRoutingView,
        state_view: &StateView<'_>,
    ) -> Result<RoutingPlan, RoutingResolveError> {
        let nexus = state_view.nexus();
        let matched_rule = nexus
            .routing_policy
            .rules
            .iter()
            .find(|rule| rule_matches(rule, tx, Some(state_view)));
        if let Some(plan) = native_amx_fx_routing_plan_with_world(
            tx,
            matched_rule,
            &nexus.lane_catalog,
            &nexus.dataspace_catalog,
            state_view.world(),
            Some(state_view_ledger_time_ms(state_view)),
        )? {
            return Ok(plan);
        }
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
            Some(tx),
            Some(AutoscaleElasticRange::from_nexus_at_height(
                nexus,
                u64::try_from(state_view.height()).unwrap_or(u64::MAX),
            )),
        )
    }

    fn try_route_without_state(
        &self,
        tx: &dyn TransactionRoutingView,
    ) -> Result<Option<RoutingDecision>, RoutingResolveError> {
        // The governed corridor registry is state-backed; defer instead of failing before the
        // caller can retry with a state view.
        if transaction_contains_fx_corridor_settlement(tx) {
            return Ok(None);
        }
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
        let target =
            transaction_dataspace_routing_target(tx, Some(self.dataspace_catalog.as_ref()), None)?;
        let matched_rule = self
            .policy
            .rules
            .iter()
            .any(|rule| rule_matches(rule, tx, None));
        if target.is_none() && !matched_rule {
            return Ok(None);
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
        tx: &dyn TransactionRoutingView,
    ) -> Result<Option<RoutingPlan>, RoutingResolveError> {
        // Participant dataspaces come from the governed corridor registry.
        if transaction_contains_fx_corridor_settlement(tx) {
            return Ok(None);
        }
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
        let target =
            transaction_dataspace_routing_target(tx, Some(self.dataspace_catalog.as_ref()), None)?;
        let matched_rule = self
            .policy
            .rules
            .iter()
            .any(|rule| rule_matches(rule, tx, None));
        if target.is_none() && !matched_rule {
            return Ok(None);
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
        tx: &dyn TransactionRoutingView,
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

fn transaction_target_routing_requires_state(tx: &dyn TransactionRoutingView) -> bool {
    let Some(executable) = transaction_executable(tx) else {
        return false;
    };

    match executable {
        Executable::Instructions(instructions) => instructions.iter().any(|instruction| {
            instruction_transaction_dataspace_target_needs_state(&**instruction)
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) => false,
        Executable::Batch(items) => items.iter().any(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => {
                instruction_transaction_dataspace_target_needs_state(&**instruction)
            }
            ExecutableBatchItem::ContractCall(_) => false,
        }),
        Executable::IvmProved(proved) => proved.overlay.iter().any(|instruction| {
            instruction_transaction_dataspace_target_needs_state(&**instruction)
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use iroha_config::parameters::actual::{LaneRoutingMatcher, LaneRoutingRule};
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        Encode, IntoKeyValue,
        account::{AccountAddress, AccountAliasDomain},
        alias_setup::{AccountAliasName, ResolvedAccountAliasV1},
        asset::{
            AssetDefinitionAlias, Mintable, NewAssetDefinition, definition::AssetConfidentialPolicy,
        },
        isi::{
            alias_setup::CompareAndSetPrimaryAccountAlias,
            prelude::{Mint, Register, Transfer},
            settlement::{
                DvpIsi, FxCorridorPolicy, FxCorridorPolicyRegistry, FxCorridorSource, PvpIsi,
                SettleFxCorridor, SettlementAtomicity, SettlementExecutionOrder,
                SettlementInstructionBox, SettlementLeg, SettlementPlan,
            },
            smart_contract_code::{
                FinalizeSmartContractCodeUpload, RegisterSmartContractBytes,
                UploadSmartContractCodeChunk,
            },
        },
        merge::{LaneDrainIntentV1, LaneDrainStateV1},
        metadata::Metadata,
        nexus::{
            AUTOSCALE_META_COMMITTEE, AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_DRAIN_STATE,
            AUTOSCALE_META_MANAGED, AssetPermissionManifest, LaneConfig, LaneVisibility,
            ManifestVersion, UniversalAccountId,
        },
        peer::PeerId,
        permission::Permission,
        prelude::*,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        sns::{NameControllerV1, NameRecordV1},
        transaction::TransactionBuilder,
    };
    use iroha_executor_data_model::permission::{
        account::{
            AccountAliasPermissionScope, CanDelegateAccountAliasResolution, CanManageAccountAlias,
            CanResolveAccountAlias,
        },
        asset_definition::{AssetDefinitionAliasPermissionScope, CanManageAssetDefinitionAlias},
        nexus::{
            CanPublishSpaceDirectoryManifest, CanPublishSpaceDirectoryManifestForAccountDomain,
            CanPublishSpaceDirectoryManifestForUaid,
        },
        trigger::CanRegisterTrigger,
    };
    use iroha_primitives::numeric::NumericSpec;
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

    fn resolved_account_alias(alias: &str, dataspace_id: DataSpaceId) -> ResolvedAccountAliasV1 {
        ResolvedAccountAliasV1::new(
            alias
                .parse::<AccountAliasName>()
                .expect("canonical account alias"),
            dataspace_id,
        )
    }

    fn sample_transaction_with_metadata(
        authority: &AccountId,
        signer: &iroha_crypto::PrivateKey,
        instructions: Vec<InstructionBox>,
        metadata: Metadata,
    ) -> AcceptedTransaction<'static> {
        let chain_id = ChainId::from("chain");
        let tx = TransactionBuilder::new(
            chain_id.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
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
        metadata: Metadata,
    ) -> AcceptedTransaction<'static> {
        let chain_id = ChainId::from("chain");
        let tx = TransactionBuilder::new(
            chain_id.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(
                Vec::new(),
                core::num::NonZeroU64::new(10_000),
            ),
        )
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

    fn sample_contract_invocation(
        authority: &AccountId,
        dataspace: DataSpaceId,
        nonce: u64,
    ) -> iroha_data_model::transaction::executable::ContractInvocation {
        iroha_data_model::transaction::executable::ContractInvocation {
            contract_address: ContractAddress::derive(
                &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
                authority,
                nonce,
                dataspace,
            )
            .expect("contract address"),
            expected_code_hash: Hash::new(format!("router-contract-{nonce}").as_bytes()),
            entrypoint: "transfer".to_owned(),
            arguments: None,
        }
    }

    fn sample_trigger_registration(
        authority: &AccountId,
        name: &str,
        executable: Executable,
    ) -> InstructionBox {
        let trigger_id: iroha_data_model::trigger::TriggerId = name.parse().expect("trigger id");
        let action = iroha_data_model::trigger::action::Action::new(
            executable,
            iroha_data_model::trigger::action::Repeats::Exactly(1),
            authority.clone(),
            iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants");
        InstructionBox::from(Register::trigger(Trigger::new(trigger_id, action)))
    }

    fn sample_contract_trigger_registration(
        authority: &AccountId,
        name: &str,
        dataspace: DataSpaceId,
        nonce: u64,
    ) -> InstructionBox {
        sample_trigger_registration(
            authority,
            name,
            Executable::ContractCall(sample_contract_invocation(authority, dataspace, nonce)),
        )
    }

    fn three_dataspace_contract_router() -> (
        LaneRoutingPolicy,
        DataSpaceCatalog,
        LaneCatalog,
        ConfigLaneRouter,
    ) {
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let catalog = dataspace_catalog(&[
            (DataSpaceId::new(7), "signer"),
            (DataSpaceId::new(8), "multisig"),
            (DataSpaceId::new(9), "contract"),
        ]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(2), DataSpaceId::new(7)),
            (LaneId::new(3), DataSpaceId::new(8)),
            (LaneId::new(4), DataSpaceId::new(9)),
        ]);
        let router = ConfigLaneRouter::new(policy.clone(), catalog.clone(), lane_catalog.clone());
        (policy, catalog, lane_catalog, router)
    }

    fn account_scope_entry(
        dataspace: DataSpaceId,
    ) -> crate::nexus::space_directory::AccountScopeDirectoryEntry {
        let mut entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        entry.ensure_dataspace(dataspace);
        entry
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

    fn lane_catalog_from_configs(lanes: Vec<LaneConfig>) -> LaneCatalog {
        let max_lane = lanes.iter().map(|lane| lane.id.as_u32()).max().unwrap_or(0);
        let lane_count =
            std::num::NonZeroU32::new(max_lane + 1).expect("catalog requires nonzero lanes");
        LaneCatalog::new(lane_count, lanes).expect("valid lane catalog")
    }

    fn nexus_with_routing(
        routing_policy: LaneRoutingPolicy,
        lane_catalog: LaneCatalog,
        dataspace_catalog: DataSpaceCatalog,
    ) -> Nexus {
        let lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog);
        Nexus {
            enabled: true,
            routing_policy,
            lane_catalog,
            lane_config,
            dataspace_catalog,
            ..Nexus::default()
        }
    }

    fn default_lane_config() -> LaneConfig {
        LaneConfig::default()
    }

    fn autoscale_elastic_lane_config(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        created_height: u64,
    ) -> LaneConfig {
        let mut metadata = BTreeMap::new();
        metadata.insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
        metadata.insert(
            AUTOSCALE_META_CREATED_HEIGHT.to_string(),
            created_height.to_string(),
        );
        let mut lane = LaneConfig {
            id: lane_id,
            dataspace_id,
            alias: format!("elastic-lane-{}", lane_id.as_u32()),
            metadata,
            ..LaneConfig::default()
        };
        crate::state::attach_synthetic_autoscale_committee_for_test(&mut lane);
        lane
    }

    fn role_registration_instruction(authority: &AccountId, name: &str) -> InstructionBox {
        let role_id = iroha_data_model::role::RoleId {
            name: name.parse().expect("valid role name"),
        };
        InstructionBox::from(Register::role(iroha_data_model::role::Role::new(
            role_id,
            authority.clone(),
        )))
    }

    fn catalog_with_lanes(lanes: &[LaneId]) -> LaneCatalog {
        let entries: Vec<(LaneId, DataSpaceId)> = lanes
            .iter()
            .map(|lane_id| (*lane_id, DataSpaceId::UNIVERSAL))
            .collect();
        catalog_with_lane_dataspaces(&entries)
    }

    fn state_from_world(world: crate::state::World) -> crate::state::State {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let telemetry = crate::telemetry::StateTelemetry::default();
        #[cfg(feature = "telemetry")]
        return crate::state::State::with_telemetry(world, kura, query, telemetry);
        #[cfg(not(feature = "telemetry"))]
        crate::state::State::new(world, kura, query)
    }

    fn blank_state() -> crate::state::State {
        state_from_world(crate::state::World::default())
    }

    fn seed_committed_height_for_router_test(state: &crate::state::State, height: u64) {
        let mut block_hashes = state.block_hashes.block();
        for idx in 0..height {
            block_hashes.push_for_tests(iroha_crypto::HashOf::<
                iroha_data_model::block::BlockHeader,
            >::from_untyped_unchecked(Hash::new(idx.to_le_bytes())));
        }
        block_hashes.commit_for_tests();
    }

    fn install_router_nexus(state: &crate::state::State, router: &ConfigLaneRouter) {
        let mut nexus = state.nexus.write();
        nexus.enabled = true;
        nexus.routing_policy = router.policy.as_ref().clone();
        nexus.dataspace_catalog = router.dataspace_catalog.as_ref().clone();
        nexus.lane_catalog = router.lane_catalog.as_ref().clone();
    }

    fn set_nexus_autoscale_range(
        state: &crate::state::State,
        enabled: bool,
        min_lanes: u32,
        max_lanes: u32,
    ) {
        let mut nexus = state.nexus.write();
        nexus.autoscale.enabled = enabled;
        nexus.autoscale.min_lanes =
            std::num::NonZeroU32::new(min_lanes).expect("autoscale test min_lanes must be nonzero");
        nexus.autoscale.max_lanes =
            std::num::NonZeroU32::new(max_lanes).expect("autoscale test max_lanes must be nonzero");
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
                .insert(account_id.clone(), BTreeSet::from([alias.clone()]));
            world.account_rekey_records.insert(
                alias.clone(),
                iroha_data_model::account::rekey::AccountRekeyRecord::new(
                    alias.clone(),
                    account_id.clone(),
                ),
            );

            let selector = crate::sns::selector_for_account_alias(alias, &dataspace_catalog)
                .expect("fixture account alias selector");
            let address = AccountAddress::from_account_id(&account_id)
                .expect("fixture account address must be canonical");
            let record = NameRecordV1::new(
                selector.clone(),
                account_id.clone(),
                vec![NameControllerV1::account(&address)],
                0,
                0,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                Metadata::default(),
            );
            world
                .smart_contract_state_mut_for_testing()
                .insert(crate::sns::record_storage_key(&selector), record.encode());
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

    fn dummy_zk_proof_attachment() -> ProofAttachment {
        ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
            VerifyingKeyId::new("halo2/ipa", "router-zk-route-fixture"),
        )
    }

    fn attach_valid_drain_state(lane: &mut LaneConfig, close_global_height: u64) {
        let keypair = KeyPair::try_from_seed(
            b"queue-router-drain-validator".to_vec(),
            Algorithm::BlsNormal,
        )
        .expect("derive queue-router drain validator");
        let validator_set = vec![PeerId::new(keypair.public_key().clone())];
        let state = LaneDrainStateV1 {
            version: 1,
            intent: LaneDrainIntentV1 {
                version: 1,
                chain_id_digest: Hash::new(b"queue-router-drain-chain"),
                lane_id: lane.id,
                dataspace_id: lane.dataspace_id,
                lane_incarnation: Hash::new(b"queue-router-drain-incarnation"),
                close_global_height,
                initial_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                    lane.id,
                    lane.dataspace_id,
                    Hash::new(b"queue-router-drain-incarnation"),
                    0,
                    None,
                ),
                validator_set_hash_version:
                    iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count: 1,
                min_quorum: 1,
            },
            commitment: None,
        };
        lane.metadata.insert(
            AUTOSCALE_META_DRAIN_STATE.to_owned(),
            hex::encode(norito::to_bytes(&state).expect("encode valid drain state")),
        );
    }

    include!("router_initial_routing_tests.rs");

    #[test]
    fn canonical_dataspace_route_ignores_autoscale_owned_lanes() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(9),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let mut malformed_autoscale_claim =
            autoscale_elastic_lane_config(LaneId::new(2), DataSpaceId::UNIVERSAL, 7);
        malformed_autoscale_claim.alias = "lane-2".to_string();
        let lane_catalog = lane_catalog_from_configs(vec![
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
            malformed_autoscale_claim,
            LaneConfig {
                id: LaneId::new(9),
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: "base-default".to_string(),
                ..LaneConfig::default()
            },
        ]);
        let dataspace_catalog = DataSpaceCatalog::default();
        let expected = RoutingDecision::new(LaneId::new(9), DataSpaceId::UNIVERSAL);

        assert_eq!(
            canonical_dataspace_route(DataSpaceId::UNIVERSAL, &lane_catalog, &dataspace_catalog)
                .expect("canonical universal route should use the non-autoscale base lane"),
            expected
        );

        let router = ConfigLaneRouter::new(policy.clone(), dataspace_catalog, lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("canonical", "universal").expect("domain"),
            )))],
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("catalog routing should ignore autoscale-owned canonical lanes"),
            expected
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("live routing should ignore autoscale-owned canonical lanes"),
            expected
        );
    }

    #[test]
    fn canonical_dataspace_route_fails_closed_with_only_autoscale_owned_lanes() {
        let mut malformed_autoscale_claim =
            autoscale_elastic_lane_config(LaneId::new(2), DataSpaceId::UNIVERSAL, 7);
        malformed_autoscale_claim
            .metadata
            .insert(AUTOSCALE_META_MANAGED.to_string(), "FALSE".to_string());
        malformed_autoscale_claim.alias = "lane-2".to_string();
        let lane_catalog = lane_catalog_from_configs(vec![
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
            malformed_autoscale_claim,
        ]);

        assert_eq!(
            canonical_dataspace_route(
                DataSpaceId::UNIVERSAL,
                &lane_catalog,
                &DataSpaceCatalog::default()
            ),
            Err(RoutingResolveError::NoLaneForDataspace {
                dataspace_id: DataSpaceId::UNIVERSAL,
            })
        );
    }

    #[test]
    fn canonical_dataspace_route_fails_closed_with_created_height_only_reserved_lane() {
        let mut marker_only = LaneConfig {
            id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "created-height-only".to_string(),
            ..LaneConfig::default()
        };
        marker_only
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "42".to_string());
        let lane_catalog = lane_catalog_from_configs(vec![marker_only]);

        assert_eq!(
            canonical_dataspace_route(
                DataSpaceId::UNIVERSAL,
                &lane_catalog,
                &DataSpaceCatalog::default()
            ),
            Err(RoutingResolveError::NoLaneForDataspace {
                dataspace_id: DataSpaceId::UNIVERSAL,
            })
        );
    }

    #[test]
    fn canonical_dataspace_route_fails_closed_for_every_consensus_autoscale_marker() {
        for marker in [AUTOSCALE_META_DRAIN_STATE, AUTOSCALE_META_COMMITTEE] {
            let mut marker_only = LaneConfig {
                id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: format!("reserved-{marker}"),
                ..LaneConfig::default()
            };
            marker_only
                .metadata
                .insert(marker.to_owned(), "malformed-but-reserved".to_owned());
            let lane_catalog = lane_catalog_from_configs(vec![marker_only]);

            assert_eq!(
                canonical_dataspace_route(
                    DataSpaceId::UNIVERSAL,
                    &lane_catalog,
                    &DataSpaceCatalog::default()
                ),
                Err(RoutingResolveError::NoLaneForDataspace {
                    dataspace_id: DataSpaceId::UNIVERSAL,
                }),
                "presence of reserved marker {marker} must never make a lane look operator-owned"
            );
        }
    }

    #[test]
    fn default_route_shards_no_target_traffic_across_autoscaled_lanes() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router =
            ConfigLaneRouter::new(policy.clone(), DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);
        seed_committed_height_for_router_test(&state, 7);

        let mut lanes_seen = BTreeSet::new();
        for idx in 0..256 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("elasticroute{idx}"),
                )],
            );
            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("default route should resolve with live catalog");
            assert_eq!(router.route_with_view(&tx, &state.view()), with_view);
            assert_eq!(router.route_with_state(&tx, &state), with_view);
            let with_catalog = evaluate_policy_with_catalog(
                &policy,
                router.lane_catalog.as_ref(),
                router.dataspace_catalog.as_ref(),
                &tx,
            )
            .expect("default route should resolve with configured catalog");

            assert_eq!(
                with_catalog,
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                "catalog-only default routing must not shard over autoscale lanes without live state"
            );
            assert_eq!(
                router
                    .try_route(&tx)
                    .expect("catalog-only route should resolve to base default lane"),
                with_catalog
            );
            assert_eq!(
                router
                    .try_route_plan(&tx)
                    .expect("catalog-only route plan should resolve to base default lane"),
                RoutingPlan::single(with_catalog)
            );
            assert_eq!(
                router
                    .try_route_without_state(&tx)
                    .expect("state-free default-route check should be deterministic"),
                None
            );
            assert_eq!(with_view.dataspace_id, DataSpaceId::UNIVERSAL);
            lanes_seen.insert(with_view.lane_id);
            if lanes_seen.len() == 2 {
                break;
            }
        }

        assert_eq!(lanes_seen, BTreeSet::from([LaneId::SINGLE, LaneId::new(1)]));
    }

    #[test]
    fn default_route_shards_no_target_ivm_traffic_with_state_not_state_free_hint() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);
        seed_committed_height_for_router_test(&state, 7);

        let mut lanes_seen = BTreeSet::new();
        for idx in 0..512_u64 {
            let mut metadata = Metadata::default();
            metadata.insert(
                "nonce".parse().expect("metadata key"),
                iroha_primitives::json::Json::new(idx),
            );
            let tx = sample_executable_transaction_with_metadata(
                &alice_id,
                alice_keypair.private_key(),
                sample_proved_executable(Vec::new()),
                metadata,
            );

            assert_eq!(
                router
                    .try_route_without_state(&tx)
                    .expect("state-free no-target IVM route should be deterministic"),
                None,
                "state-free no-target IVM routing must defer possible autoscale sharding"
            );
            assert_eq!(
                router.route_without_state(&tx),
                None,
                "non-fallible state-free hint must not pin no-target IVM traffic to the base lane"
            );

            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("live no-target IVM route should resolve");
            assert_eq!(router.route_with_state(&tx, &state), with_view);
            lanes_seen.insert(with_view.lane_id);
            if lanes_seen.len() == 2 {
                break;
            }
        }

        assert_eq!(lanes_seen, BTreeSet::from([LaneId::SINGLE, LaneId::new(1)]));
    }

    #[test]
    fn default_route_sharding_fails_closed_for_future_created_elastic_lane() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);
        seed_committed_height_for_router_test(&state, 6);

        for idx in 0..64 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("futureelasticroute{idx}"),
                )],
            );
            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("future-created elastic lane should fail closed to the default route");
            assert_eq!(
                with_view,
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                "future-created elastic lanes must not receive default-route traffic"
            );
            assert_eq!(router.route_with_state(&tx, &state), with_view);
            assert_eq!(
                router
                    .try_route_plan_with_view(&tx, &state.view())
                    .expect("future-created elastic lane plan should resolve to the default route"),
                RoutingPlan::single(with_view),
                "future-created elastic lanes must not appear in default-route plans"
            );
        }
    }

    #[test]
    fn nexus_world_routing_at_block_height_excludes_future_created_elastic_lane() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let mut nexus = iroha_config::parameters::actual::Nexus {
            enabled: true,
            ..Default::default()
        };
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lanes = nonzero!(1_u32);
        nexus.autoscale.max_lanes = nonzero!(8_u32);
        nexus.lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        let state = blank_state();

        let mut saw_active_elastic = false;
        for idx in 0..256 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("heightawareelasticroute{idx}"),
                )],
            );
            assert_eq!(
                evaluate_policy_plan_with_nexus_and_world_at_block_height(
                    &nexus,
                    &tx,
                    state.view().world(),
                    0,
                    6,
                )
                .expect("future-created lane should resolve to the default plan"),
                RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)),
                "future-created elastic lanes must not be selected before their creation height"
            );
            assert_eq!(
                evaluate_policy_plan_with_nexus_and_world_at(&nexus, &tx, state.view().world(), 0,)
                    .expect("heightless Nexus/world route should resolve to the default plan"),
                RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)),
                "heightless Nexus/world routing must not shard over autoscale lanes"
            );
            let active_plan = evaluate_policy_plan_with_nexus_and_world_at_block_height(
                &nexus,
                &tx,
                state.view().world(),
                0,
                7,
            )
            .expect("active elastic lane should resolve at its creation height");
            if active_plan.coordinator_route().lane_id == LaneId::new(1) {
                saw_active_elastic = true;
                break;
            }
        }

        assert!(
            saw_active_elastic,
            "fixture should find a transaction assigned to the active elastic lane"
        );
    }

    #[test]
    fn default_route_sharding_fails_closed_with_default_anchor_above_elastic_range() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(9),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
            LaneConfig {
                id: LaneId::new(9),
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: "base-default".to_string(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: LaneId::new(10),
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: "manual-sidecar".to_string(),
                ..LaneConfig::default()
            },
        ]);
        let router =
            ConfigLaneRouter::new(policy.clone(), DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 3);
        seed_committed_height_for_router_test(&state, 7);

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![role_registration_instruction(
                &alice_id,
                "highdefaultelasticroute",
            )],
        );
        let expected = RoutingDecision::new(LaneId::new(9), DataSpaceId::UNIVERSAL);
        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("state-free routing should be deterministic"),
            None,
            "state-free no-target routing must defer possible autoscale sharding to live state"
        );
        assert_eq!(
            router
                .try_route(&tx)
                .expect("catalog-only route should resolve to the high default anchor"),
            expected,
            "catalog-only default routing must stay pinned to the configured default anchor"
        );
        assert_eq!(
            router
                .try_route_with_state(&tx, &state)
                .expect("default route should resolve with live Nexus state"),
            expected,
            "runtime high-side default route must fail closed instead of sharding onto elastic lanes"
        );
        assert_eq!(
            router
                .try_route_plan_with_state(&tx, &state)
                .expect("default route plan should resolve with live Nexus state"),
            RoutingPlan::single(expected)
        );
    }

    #[test]
    fn default_route_with_unmatched_rules_still_uses_live_autoscale_range() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(8),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("register::domain".to_string()),
                    description: None,
                },
            }],
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
            LaneConfig {
                id: LaneId::new(8),
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: "manual-domain-lane".to_string(),
                ..LaneConfig::default()
            },
        ]);
        let router =
            ConfigLaneRouter::new(policy.clone(), DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);
        seed_committed_height_for_router_test(&state, 7);

        let mut lanes_seen = BTreeSet::new();
        for idx in 0..512 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("unmatchedruledefault{idx}"),
                )],
            );
            assert_eq!(
                router
                    .try_route_without_state(&tx)
                    .expect("state-free routing should be deterministic"),
                None,
                "unmatched rules must not make no-target default routing catalog-only"
            );
            assert_eq!(
                router
                    .try_route_plan_without_state(&tx)
                    .expect("state-free plan routing should be deterministic"),
                None,
                "unmatched rules must not make no-target default plans catalog-only"
            );
            assert_eq!(
                evaluate_policy_with_catalog(
                    &policy,
                    router.lane_catalog.as_ref(),
                    router.dataspace_catalog.as_ref(),
                    &tx,
                )
                .expect("catalog-only default route should resolve"),
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                "catalog-only routing remains pinned to the base default lane"
            );

            let with_state = router
                .try_route_with_state(&tx, &state)
                .expect("default route should resolve with live Nexus state");
            assert_eq!(with_state.dataspace_id, DataSpaceId::UNIVERSAL);
            assert_eq!(router.route_with_state(&tx, &state), with_state);
            assert_eq!(
                router
                    .try_route_plan_with_state(&tx, &state)
                    .expect("default route plan should resolve with live Nexus state"),
                RoutingPlan::single(with_state)
            );
            lanes_seen.insert(with_state.lane_id);
            if lanes_seen.len() == 2 {
                break;
            }
        }

        assert_eq!(
            lanes_seen,
            BTreeSet::from([LaneId::SINGLE, LaneId::new(1)]),
            "live default-route sharding must use the elastic lane even when unrelated rules exist"
        );
    }

    #[test]
    fn default_route_sharding_ignores_autoscale_lanes_outside_enabled_range() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
            autoscale_elastic_lane_config(LaneId::new(8), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);
        seed_committed_height_for_router_test(&state, 7);

        let mut lanes_seen = BTreeSet::new();
        for idx in 0..512 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("rangedefault{idx}"),
                )],
            );
            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("default route should resolve with live catalog");
            assert_eq!(router.route_with_view(&tx, &state.view()), with_view);
            assert_eq!(router.route_with_state(&tx, &state), with_view);
            assert_ne!(with_view.lane_id, LaneId::new(8));
            assert_eq!(
                router
                    .try_route_plan_with_view(&tx, &state.view())
                    .expect("default route plan should resolve with live catalog"),
                RoutingPlan::single(with_view)
            );
            lanes_seen.insert(with_view.lane_id);
            if lanes_seen.len() == 2 {
                break;
            }
        }

        assert_eq!(lanes_seen, BTreeSet::from([LaneId::SINGLE, LaneId::new(1)]));
    }

    #[test]
    fn default_route_sharding_fails_closed_when_elastic_range_contains_corruption() {
        struct CorruptionCase {
            name: &'static str,
            lane: LaneConfig,
        }

        let mut malformed_managed =
            autoscale_elastic_lane_config(LaneId::new(2), DataSpaceId::UNIVERSAL, 7);
        malformed_managed.alias = "malformed-elastic-lane".to_string();

        let cases = [
            CorruptionCase {
                name: "manual-in-range",
                lane: LaneConfig {
                    id: LaneId::new(2),
                    alias: "manual-elastic-range".to_string(),
                    ..LaneConfig::default()
                },
            },
            CorruptionCase {
                name: "malformed-managed-in-range",
                lane: malformed_managed,
            },
            CorruptionCase {
                name: "off-default-managed-in-range",
                lane: autoscale_elastic_lane_config(LaneId::new(2), DataSpaceId::new(9), 7),
            },
        ];

        for case in cases {
            let (alice_id, alice_keypair) = gen_account_in("wonderland");
            let policy = LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            };
            let lane_catalog = lane_catalog_from_configs(vec![
                default_lane_config(),
                autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
                case.lane,
            ]);
            let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
            let state = blank_state();
            install_router_nexus(&state, &router);
            set_nexus_autoscale_range(&state, true, 1, 8);
            seed_committed_height_for_router_test(&state, 7);

            for idx in 0..64 {
                let tx = sample_transaction(
                    &alice_id,
                    alice_keypair.private_key(),
                    vec![role_registration_instruction(
                        &alice_id,
                        &format!("{}{}", case.name.replace('-', ""), idx),
                    )],
                );
                let with_view = router
                    .try_route_with_view(&tx, &state.view())
                    .unwrap_or_else(|err| {
                        panic!("{}: default route should resolve: {err}", case.name)
                    });
                assert_eq!(
                    with_view,
                    RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                    "{} corruption must keep default traffic on the base lane until repaired",
                    case.name
                );
                assert_eq!(router.route_with_view(&tx, &state.view()), with_view);
                assert_eq!(router.route_with_state(&tx, &state), with_view);
                assert_eq!(
                    router
                        .try_route_plan_with_view(&tx, &state.view())
                        .unwrap_or_else(|err| panic!(
                            "{}: default route plan should resolve: {err}",
                            case.name
                        )),
                    RoutingPlan::single(with_view),
                    "{} corruption must not keep sharding route plans over elastic lanes",
                    case.name
                );
            }
        }
    }

    #[test]
    fn default_route_sharding_ignores_managed_lanes_when_autoscale_disabled() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, false, 1, 8);

        for idx in 0..64 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("disabledefault{idx}"),
                )],
            );
            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("default route should resolve with live catalog");
            assert_eq!(router.route_with_view(&tx, &state.view()), with_view);
            assert_eq!(router.route_with_state(&tx, &state), with_view);
            assert_eq!(
                with_view,
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
            );
            assert_eq!(
                router
                    .try_route_plan_with_view(&tx, &state.view())
                    .expect("default route plan should resolve with live catalog"),
                RoutingPlan::single(with_view)
            );
        }
    }

    #[test]
    fn default_route_sharding_fails_closed_when_nexus_disabled() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);
        state.nexus.write().enabled = false;

        for idx in 0..64 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("nexusdisabledautoscale{idx}"),
                )],
            );
            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("default route should resolve when nexus is disabled");
            assert_eq!(
                with_view,
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
            );
            assert_eq!(
                router
                    .try_route_plan_with_view(&tx, &state.view())
                    .expect("default route plan should resolve when nexus is disabled"),
                RoutingPlan::single(with_view)
            );
        }
    }

    #[test]
    fn default_route_sharding_fails_closed_when_default_lane_is_inside_autoscale_range() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(1),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            LaneConfig {
                id: LaneId::new(1),
                alias: "corrupt-default".to_string(),
                ..LaneConfig::default()
            },
            autoscale_elastic_lane_config(LaneId::new(2), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);

        for idx in 0..64 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("defaultinrange{idx}"),
                )],
            );
            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("corrupt default route should still resolve to the default lane");
            assert_eq!(
                with_view,
                RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL)
            );
            assert_eq!(
                router
                    .try_route_plan_with_view(&tx, &state.view())
                    .expect("corrupt default route plan should still resolve"),
                RoutingPlan::single(with_view)
            );
        }
    }

    #[test]
    fn default_route_sharding_fails_closed_when_autoscale_max_lanes_exceeds_cap() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(
            &state,
            true,
            1,
            iroha_config::parameters::defaults::nexus::autoscale::MAX_LANES + 1,
        );

        for idx in 0..64 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("autoscalecap{idx}"),
                )],
            );
            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("default route should resolve when autoscale bounds are invalid");
            assert_eq!(
                with_view,
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
            );
            assert_eq!(
                router
                    .try_route_plan_with_view(&tx, &state.view())
                    .expect("default route plan should resolve when autoscale bounds are invalid"),
                RoutingPlan::single(with_view)
            );
        }
    }

    #[test]
    fn default_route_sharding_fails_closed_when_autoscale_min_exceeds_max() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 4, 2);

        for idx in 0..64 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("autoscaleinverted{idx}"),
                )],
            );
            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("default route should resolve when autoscale bounds are inverted");
            assert_eq!(
                with_view,
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
            );
            assert_eq!(
                router
                    .try_route_plan_with_state(&tx, &state)
                    .expect("default route plan should resolve when autoscale bounds are inverted"),
                RoutingPlan::single(with_view)
            );
        }
    }

    #[test]
    fn default_route_sharding_fails_closed_when_autoscale_min_equals_max() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(4), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 4, 4);

        for idx in 0..64 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("autoscaleempty{idx}"),
                )],
            );
            let with_view = router
                .try_route_with_view(&tx, &state.view())
                .expect("default route should resolve when autoscale range is empty");
            assert_eq!(
                with_view,
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                "empty autoscale elastic ranges must fail closed to the default lane"
            );
            assert_eq!(
                router
                    .try_route_plan_with_state(&tx, &state)
                    .expect("default route plan should resolve when autoscale range is empty"),
                RoutingPlan::single(with_view)
            );
        }
    }

    #[test]
    fn default_route_rejects_autoscale_owned_default_lane() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(1),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![role_registration_instruction(&alice_id, "autoscaledefault")],
        );

        assert_eq!(
            router.try_route(&tx),
            Err(RoutingResolveError::AutoscaleOwnedDefaultLane {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            router.try_route_with_view(&tx, &state.view()),
            Err(RoutingResolveError::AutoscaleOwnedDefaultLane {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            router.try_route_plan(&tx),
            Err(RoutingResolveError::AutoscaleOwnedDefaultLane {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            router.route_with_view(&tx, &state.view()),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            "non-fallible live routing must not return the autoscale-owned default lane"
        );
        assert_eq!(
            router.route_with_state(&tx, &state),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            "state-backed non-fallible routing must fail closed to a canonical base route"
        );
        assert_eq!(router.try_route_without_state(&tx), Ok(None));
    }

    #[test]
    fn default_route_rejects_created_height_only_default_lane() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(1),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let mut marker_only = LaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "created-height-only-default".to_string(),
            ..LaneConfig::default()
        };
        marker_only
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "42".to_string());
        let lane_catalog = lane_catalog_from_configs(vec![default_lane_config(), marker_only]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![role_registration_instruction(
                &alice_id,
                "createdheightdefault",
            )],
        );

        assert_eq!(
            router.try_route(&tx),
            Err(RoutingResolveError::AutoscaleOwnedDefaultLane {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            router.try_route_plan(&tx),
            Err(RoutingResolveError::AutoscaleOwnedDefaultLane {
                lane_id: LaneId::new(1),
            })
        );
    }

    #[test]
    fn explicit_rule_overrides_autoscaled_default_sharding() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(2),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("register::role".to_string()),
                    description: None,
                },
            }],
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
            LaneConfig {
                id: LaneId::new(2),
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: "manual-role-lane".to_string(),
                ..LaneConfig::default()
            },
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);

        for idx in 0..32 {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![role_registration_instruction(
                    &alice_id,
                    &format!("manualroute{idx}"),
                )],
            );
            assert_eq!(
                router
                    .try_route_with_view(&tx, &state.view())
                    .expect("explicit rule route should resolve"),
                RoutingDecision::new(LaneId::new(2), DataSpaceId::UNIVERSAL)
            );
            assert_eq!(
                router
                    .route_without_state(&tx)
                    .expect("explicit rule route should not need state"),
                RoutingDecision::new(LaneId::new(2), DataSpaceId::UNIVERSAL)
            );
        }
    }

    #[test]
    fn explicit_rule_rejects_autoscale_owned_rule_lane() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::UNIVERSAL),
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("register::role".to_string()),
                    description: None,
                },
            }],
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let state = blank_state();
        install_router_nexus(&state, &router);
        set_nexus_autoscale_range(&state, true, 1, 8);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![role_registration_instruction(&alice_id, "autoscalerule")],
        );

        assert_eq!(
            router.try_route(&tx),
            Err(RoutingResolveError::AutoscaleOwnedRuleLane {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            router.try_route_with_view(&tx, &state.view()),
            Err(RoutingResolveError::AutoscaleOwnedRuleLane {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            router.try_route_plan(&tx),
            Err(RoutingResolveError::AutoscaleOwnedRuleLane {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            router.route_with_view(&tx, &state.view()),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            "non-fallible live routing must not return the autoscale-owned rule lane"
        );
        assert_eq!(
            router.route_with_state(&tx, &state),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            "state-backed non-fallible routing must fail closed to a canonical base route"
        );
        assert_eq!(
            router.try_route_without_state(&tx),
            Err(RoutingResolveError::AutoscaleOwnedRuleLane {
                lane_id: LaneId::new(1),
            })
        );
    }

    #[test]
    fn explicit_rule_rejects_created_height_only_rule_lane() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::UNIVERSAL),
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("register::role".to_string()),
                    description: None,
                },
            }],
        };
        let mut marker_only = LaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "created-height-only-rule".to_string(),
            ..LaneConfig::default()
        };
        marker_only
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "42".to_string());
        let lane_catalog = lane_catalog_from_configs(vec![default_lane_config(), marker_only]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![role_registration_instruction(
                &alice_id,
                "createdheightrule",
            )],
        );

        assert_eq!(
            router.try_route(&tx),
            Err(RoutingResolveError::AutoscaleOwnedRuleLane {
                lane_id: LaneId::new(1),
            })
        );
        assert_eq!(
            router.try_route_without_state(&tx),
            Err(RoutingResolveError::AutoscaleOwnedRuleLane {
                lane_id: LaneId::new(1),
            })
        );
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
                None,
                None,
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
        let delivery_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "universal").expect("domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let payment_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "universal").expect("domain id"),
            "cash".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(DvpIsi::new(
                "commonroute".parse().expect("settlement id"),
                SettlementLeg::new(delivery_definition, 1_u32, alice_id.clone(), bob_id.clone()),
                SettlementLeg::new(payment_definition, 1_u32, bob_id, alice_id.clone()),
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
        let delivery_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "delivery").expect("domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let payment_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "payment").expect("domain id"),
            "cash".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(DvpIsi::new(
                "crossroute".parse().expect("settlement id"),
                SettlementLeg::new(delivery_definition, 1_u32, alice_id.clone(), bob_id.clone()),
                SettlementLeg::new(payment_definition, 1_u32, bob_id, alice_id.clone()),
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
        let primary_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "primary").expect("domain id"),
            "usd".parse().expect("asset definition name"),
        );
        let counter_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "counter").expect("domain id"),
            "eur".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(PvpIsi::new(
                "pvpcrossroute".parse().expect("settlement id"),
                SettlementLeg::new(primary_definition, 1_u32, alice_id.clone(), bob_id.clone()),
                SettlementLeg::new(counter_definition, 1_u32, bob_id, alice_id.clone()),
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
        assert_eq!(
            decision,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );

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
        assert_eq!(
            decision,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );

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
        assert_eq!(
            decision,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );

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
        let state = blank_state();
        install_router_nexus(&state, &router);
        let code_hash = Hash::new(&code);
        let cases = [
            InstructionBox::from(RegisterSmartContractBytes {
                code_hash,
                code: code.clone(),
            }),
            InstructionBox::from(UploadSmartContractCodeChunk {
                code_hash,
                total_size: u64::try_from(code.len()).unwrap(),
                chunk_index: 0,
                chunk_count: 1,
                chunk: code,
            }),
            InstructionBox::from(FinalizeSmartContractCodeUpload {
                code_hash,
                total_size: 4,
                chunk_count: 1,
            }),
        ];
        for instruction in cases {
            let tx = sample_transaction(&alice_id, alice_keypair.private_key(), vec![instruction]);
            let decision = router.route_with_view(&tx, &state.view());
            assert_eq!(decision.lane_id, LaneId::new(1));
        }
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
            &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
            &alice_id,
            0,
            dataspace_id,
        )
        .expect("contract address");
        let invocation = iroha_data_model::transaction::executable::ContractInvocation {
            contract_address,
            expected_code_hash: iroha_crypto::Hash::new(b"router-contract-code"),
            entrypoint: "transfer".to_owned(),
            arguments: None,
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
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))]),
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![
                InstructionBox::from(Mint::asset_quantity(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let delivery_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "paynet").expect("domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let payment_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "paynet").expect("domain id"),
            "cash".parse().expect("asset definition name"),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(DvpIsi::new(
                "proved-dvp-common".parse().expect("settlement id"),
                SettlementLeg::new(delivery_definition, 1_u32, alice_id.clone(), bob_id.clone()),
                SettlementLeg::new(payment_definition, 1_u32, bob_id, alice_id.clone()),
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
        let primary_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "paynet").expect("domain id"),
            "usd".parse().expect("asset definition name"),
        );
        let counter_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "cbuae").expect("domain id"),
            "aed".parse().expect("asset definition name"),
        );
        let tx = sample_executable_transaction(
            &alice_id,
            alice_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(PvpIsi::new(
                "proved-pvp-cross".parse().expect("settlement id"),
                SettlementLeg::new(primary_definition, 1_u32, alice_id.clone(), bob_id.clone()),
                SettlementLeg::new(counter_definition, 1_u32, bob_id, alice_id.clone()),
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
        let delivery_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("settlement", "universal").expect("domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let payment_definition = AssetDefinitionId::derive_from_components(
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
                SettlementLeg::new(opaque_delivery, 1_u32, alice_id.clone(), bob_id.clone()),
                SettlementLeg::new(opaque_payment, 1_u32, bob_id, alice_id.clone()),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            ))]),
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    delivery_definition.clone(),
                    "bond".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
                .build(&alice_id),
                AssetDefinition::numeric(
                    payment_definition.clone(),
                    "cash".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
            &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
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
    fn atomic_contract_deployment_routes_to_new_address_dataspace() {
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
            &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
            &alice_id,
            0,
            dataspace_id,
        )
        .expect("contract address");
        let instruction = CommitContractDeployment {
            expected_deploy_nonce: 0,
            contract_address,
            code_hash: Hash::new(b"contract-code"),
            contract_alias: "payments::paynet".parse().expect("contract alias"),
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        };
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(instruction)],
        );

        assert_eq!(
            router
                .try_route(&tx)
                .expect("atomic contract deployment route must resolve"),
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
            &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
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
    fn smart_contract_deploy_rule_with_universal_target_builds_native_amx_plan() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let is_dataspace = DataSpaceId::new(6647857470246403404);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: LaneId::new(3),
                    dataspace: Some(is_dataspace),
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("smartcontract::deploy".to_owned()),
                        description: None,
                    },
                }],
            },
            dataspace_catalog(&[(is_dataspace, "is")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(3), is_dataspace),
            ]),
        );
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
            &alice_id,
            0,
            DataSpaceId::UNIVERSAL,
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
            .expect("universal contract deploy rule should build a native AMX plan");
        let RoutingPlan::NativeAmx(plan) = plan else {
            panic!("universal contract deploy should not collapse to a mismatched single route");
        };
        assert_eq!(
            plan.coordinator.route,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            plan.participants,
            vec![RouteLeg::new(
                RoutingDecision::new(LaneId::new(3), is_dataspace),
                RouteLegRole::Participant,
            )]
        );
    }

    #[test]
    fn musubi_alias_registration_uses_universal_amx_with_home_dataspace_participant() {
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
        let target = iroha_data_model::musubi::MusubiPackageIdV1::new(
            dataspace_id,
            iroha_data_model::musubi::MusubiPackageScopeV1::Domain(
                "mibank".parse().expect("domain scope"),
            ),
            "fx".parse().expect("package name"),
        );
        let instruction = iroha_data_model::isi::musubi::RegisterMusubiAliasV1::new(
            "fx".parse().expect("alias"),
            target,
            1,
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(instruction)],
        );

        let plan = router
            .try_route_plan(&tx)
            .expect("Musubi alias route must resolve");
        let RoutingPlan::NativeAmx(plan) = plan else {
            panic!("Musubi alias registration must use Native AMX");
        };
        assert_eq!(
            plan.coordinator.route,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            plan.participants,
            vec![RouteLeg::new(
                RoutingDecision::new(lane_id, dataspace_id),
                RouteLegRole::Participant,
            )]
        );
    }

    #[test]
    fn musubi_release_publication_uses_universal_amx_with_home_dataspace_participant() {
        use iroha_data_model::{
            isi::musubi::PublishMusubiReleaseV1,
            musubi::{
                ArchiveId, MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiContentDigestV1,
                MusubiKotodamaEditionV1, MusubiPackageIdV1, MusubiPackageScopeV1,
                MusubiPublicationV1, MusubiRegistrySnapshotV1, MusubiReleaseIdV1,
                MusubiReleaseManifestV1, MusubiReleaseMetadataV1, MusubiResolutionProofV1,
                MusubiVerificationLockV1,
            },
        };

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
        let package = MusubiPackageIdV1::new(
            dataspace_id,
            MusubiPackageScopeV1::Domain("mibank".parse().expect("domain scope")),
            "fx".parse().expect("package name"),
        );
        let release =
            MusubiReleaseIdV1::new(package, "1.0.0".parse().expect("publication version"));
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let publication = MusubiPublicationV1 {
            manifest: MusubiReleaseManifestV1 {
                release,
                edition: MusubiKotodamaEditionV1::V1,
                abi: MusubiAbiBindingV1::new([0x41; 32]).expect("ABI binding"),
                dependencies: Vec::new(),
                exports: Vec::new(),
                interface_digest: MusubiContentDigestV1::new([0x42; 32]),
                metadata: MusubiReleaseMetadataV1::default(),
                archive_id: ArchiveId::new([0x43; 32]),
                verification_lock_digest: lock.digest(),
            },
            resolution: MusubiResolutionProofV1 {
                snapshot: MusubiRegistrySnapshotV1 {
                    finalized_height: 7,
                    finalized_block_hash: [0x44; 32],
                    index_revision: 3,
                },
                lock,
            },
        };
        let instruction = PublishMusubiReleaseV1::new(
            "mibank.paynet".parse().expect("publication namespace"),
            publication,
            None,
            1,
            None,
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(instruction)],
        );

        let plan = router
            .try_route_plan(&tx)
            .expect("Musubi publication route must resolve");
        let RoutingPlan::NativeAmx(plan) = plan else {
            panic!("Musubi publication must use Native AMX");
        };
        assert_eq!(
            plan.coordinator.route,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            plan.participants,
            vec![RouteLeg::new(
                RoutingDecision::new(lane_id, dataspace_id),
                RouteLegRole::Participant,
            )]
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

        let asset_definition: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("uae", "universal").unwrap(),
                "aed".parse().unwrap(),
            );
        let asset_id = AssetId::of(asset_definition, sender_id.clone());
        let transfer = Transfer::asset_quantity(asset_id, 1_u32, receiver_id.clone());
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
        let owning_domain = DomainId::try_new("cash", "sbp").expect("asset definition domain");
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            "pkr".parse().expect("asset definition name"),
        );
        let transfer = Transfer::asset_quantity(
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
        let owning_domain = DomainId::try_new("cash", "paynet").expect("asset definition domain");
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            "pkr".parse().expect("asset definition name"),
        );
        let opaque_asset_definition =
            AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
                .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_quantity(
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
                AssetDefinition::numeric(
                    asset_definition,
                    "pkr".to_owned(),
                    AssetBalancePolicy::DataspaceRestricted,
                    Some(owning_domain),
                )
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
    fn canonical_asset_transfer_uses_stored_owning_domain_dataspace() {
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("cash", "universal").expect("asset definition domain"),
                "pkr".parse().expect("asset definition name"),
            );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_quantity(
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
        let owning_domain = DomainId::try_new("cash", "paynet").expect("owning domain");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    opaque_asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::DataspaceRestricted,
                    Some(owning_domain),
                )
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("cash", "universal").expect("asset definition domain"),
                "pkr".parse().expect("asset definition name"),
            );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_quantity(
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
                AssetDefinition::numeric(
                    opaque_asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::DataspaceRestricted,
                    Some(DomainId::try_new("cash", "sbp").expect("owning domain")),
                )
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
        let transparent_asset_definition = AssetDefinitionId::derive_from_components(
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
            owning_domain: Some(
                DomainId::try_new("cash", "universal").expect("asset definition domain"),
            ),
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
        let transparent_asset_definition = AssetDefinitionId::derive_from_components(
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
            owning_domain: None,
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
        let transparent_asset_definition = AssetDefinitionId::derive_from_components(
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
            owning_domain: None,
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
        let transparent_asset_definition = AssetDefinitionId::derive_from_components(
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
            owning_domain: Some(
                DomainId::try_new("cash", "universal").expect("asset definition domain"),
            ),
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
            id: AssetDefinitionId::derive_from_components(
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
            owning_domain: None,
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
            &AssetDefinitionId::derive_from_components(
                DomainId::try_new("cash", "universal").expect("asset definition domain"),
                "pkr".parse().expect("asset definition name"),
            )
            .canonical_address(),
        )
        .expect("opaque pkr definition id");
        let aed_id = AssetDefinitionId::parse_address_literal(
            &AssetDefinitionId::derive_from_components(
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
            owning_domain: None,
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
            owning_domain: None,
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
        let owning_domain = DomainId::try_new("cash", "sbp").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            "pkr".parse().expect("asset definition name"),
        );
        let transfer = Transfer::asset_quantity(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Burn::asset_quantity(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let owning_domain = DomainId::try_new("cash", "paynet").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition,
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    Some(owning_domain),
                )
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
        let owning_domain = DomainId::try_new("cash", "paynet").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Burn::asset_quantity(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition,
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    Some(owning_domain),
                )
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
        let owning_domain = DomainId::try_new("cash", "paynet").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Transfer::asset_quantity(
                AssetId::of(asset_definition.clone(), alice_id.clone()),
                1_u32,
                bob_id,
            ))],
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition,
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    Some(owning_domain),
                )
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
        let owning_domain = DomainId::try_new("cash", "sbp").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
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
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                scoped_asset_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition,
                    "pkr".to_owned(),
                    AssetBalancePolicy::DataspaceRestricted,
                    Some(owning_domain),
                )
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
        let owning_domain = DomainId::try_new("cash", "sbp").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
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
            vec![InstructionBox::from(Transfer::asset_quantity(
                scoped_asset_id,
                1_u32,
                bob_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition,
                    "pkr".to_owned(),
                    AssetBalancePolicy::DataspaceRestricted,
                    Some(owning_domain),
                )
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
        let owning_domain = DomainId::try_new("cash", "paynet").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
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
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                scoped_asset_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition,
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
            vec![InstructionBox::from(Burn::asset_quantity(
                1_u32,
                scoped_asset_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition,
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
            vec![InstructionBox::from(Transfer::asset_quantity(
                scoped_asset_id,
                1_u32,
                bob_id,
            ))],
        );
        let state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition,
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
    fn global_asset_zk_registration_from_private_authority_routes_to_universal() {
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
        let owning_domain = DomainId::try_new("cash", "paynet").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            "pkr".parse().expect("asset definition name"),
        );
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    Some(owning_domain),
                )
                .build(&alice_id),
            ],
            dataspace_catalog,
            lane_catalog,
        );
        scope_account_to_dataspace(&mut state, &alice_id, dataspace_id);

        let instruction = InstructionBox::from(RegisterZkAsset::new(
            asset_definition,
            iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
            true,
            true,
            None,
            None,
        ));
        let tx = sample_transaction(&alice_id, alice_keypair.private_key(), vec![instruction]);
        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("ZK asset route should defer to state"),
            None,
            "registration should not route without the asset policy"
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("global ZK asset route must resolve"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            "registration should route to universal"
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
        let owning_domain =
            DomainId::try_new("cash", "universal").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_quantity(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    Some(owning_domain),
                )
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
        let owning_domain = DomainId::try_new("cash", "paynet").expect("asset definition domain");
        let asset_definition = AssetDefinitionId::derive_from_components(
            owning_domain.clone(),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Mint::asset_quantity(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::DataspaceRestricted,
                    Some(owning_domain),
                )
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

    fn fx_corridor_fixture(
        source_dataspace: DataSpaceId,
        destination_dataspace: DataSpaceId,
        source_sink: AccountId,
        destination_reserve: AccountId,
        recipient: AccountId,
        settlement_id: &str,
    ) -> (FxCorridorPolicy, InstructionBox) {
        let source_asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("cbuae", "universal").expect("source asset domain"),
            "aed".parse().expect("source asset name"),
        );
        let destination_asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("sbp", "universal").expect("destination asset domain"),
            "pkr".parse().expect("destination asset name"),
        );
        let corridor = FxCorridorPolicy {
            policy_id: "mobile_aed_pkr".parse().expect("FX corridor policy id"),
            revision: 1,
            source_dataspace,
            source: FxCorridorSource::TransactionAuthority,
            source_asset_definition_id: source_asset_definition_id.clone(),
            source_sink,
            destination_dataspace,
            destination_reserve,
            destination_asset_definition_id: destination_asset_definition_id.clone(),
            allowed_destination_alias_domains: BTreeSet::from([
                DomainId::try_new("hbl", "sbp").expect("HBL alias domain"),
                DomainId::try_new("ubl", "sbp").expect("UBL alias domain"),
            ]),
            rate_numerator: 76,
            rate_denominator: 1,
            enabled: true,
        };
        let settlement = SettleFxCorridor {
            policy_id: corridor.policy_id.clone(),
            expected_policy_revision: corridor.revision,
            source_asset_definition_id,
            destination_asset_definition_id,
            settlement_id: settlement_id.parse().expect("FX settlement id"),
            recipient,
            source_amount: iroha_primitives::numeric::Quantity::from(10_u32),
        };
        (
            corridor,
            InstructionBox::from(SettlementInstructionBox::SettleFxCorridor(settlement)),
        )
    }

    fn install_fx_corridor_policy(state: &crate::state::State, corridor: FxCorridorPolicy) {
        let mut registry = FxCorridorPolicyRegistry::default();
        registry.upsert(corridor);
        let mut world = state.world.block();
        world
            .parameters
            .get_mut()
            .set_parameter(iroha_data_model::parameter::Parameter::Custom(
                registry.into_custom_parameter(),
            ));
        world.commit();
    }

    fn fx_route_plan_results(
        router: &ConfigLaneRouter,
        tx: &dyn TransactionRoutingView,
        corridor: FxCorridorPolicy,
        world: crate::state::World,
    ) -> (
        Result<RoutingPlan, RoutingResolveError>,
        Result<RoutingPlan, RoutingResolveError>,
    ) {
        let state = state_from_world(world);
        install_router_nexus(&state, router);
        install_fx_corridor_policy(&state, corridor);
        let view = state.view();
        let queued_plan = router.try_route_plan_with_view(tx, &view);
        let block_plan = evaluate_policy_plan_with_nexus_and_world_at(
            view.nexus(),
            tx,
            view.world(),
            state_view_ledger_time_ms(&view),
        );
        (queued_plan, block_plan)
    }

    fn expected_fx_plan(
        source_lane: LaneId,
        source_dataspace: DataSpaceId,
        destination_lane: LaneId,
        destination_dataspace: DataSpaceId,
    ) -> RoutingPlan {
        RoutingPlan::native_amx(
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            vec![
                RouteLeg::new(
                    RoutingDecision::new(source_lane, source_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(destination_lane, destination_dataspace),
                    RouteLegRole::Participant,
                ),
            ],
        )
    }

    #[test]
    fn fx_corridor_non_deploy_first_match_does_not_add_rule_dataspaces() {
        let (authority, authority_keypair) = gen_account_in("wonderland");
        let (source_sink, _) = gen_account_in("wonderland");
        let (destination_reserve, _) = gen_account_in("wonderland");
        let (recipient, _) = gen_account_in("wonderland");
        let source_dataspace = DataSpaceId::new(10);
        let destination_dataspace = DataSpaceId::new(12);
        let non_deploy_dataspace = DataSpaceId::new(14);
        let deploy_dataspace = DataSpaceId::new(16);
        let source_lane = LaneId::new(3);
        let destination_lane = LaneId::new(4);
        let non_deploy_lane = LaneId::new(5);
        let deploy_lane = LaneId::new(6);
        let dataspace_catalog = dataspace_catalog(&[
            (source_dataspace, "cbuae"),
            (destination_dataspace, "sbp"),
            (non_deploy_dataspace, "domain_policy"),
            (deploy_dataspace, "deploy_policy"),
        ]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (source_lane, source_dataspace),
            (destination_lane, destination_dataspace),
            (non_deploy_lane, non_deploy_dataspace),
            (deploy_lane, deploy_dataspace),
        ]);
        let routing_policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![
                LaneRoutingRule {
                    lane: non_deploy_lane,
                    dataspace: Some(non_deploy_dataspace),
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("register::domain".to_owned()),
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: deploy_lane,
                    dataspace: Some(deploy_dataspace),
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("smartcontract::deploy".to_owned()),
                        description: None,
                    },
                },
            ],
        };
        let router = ConfigLaneRouter::new(routing_policy, dataspace_catalog, lane_catalog);
        let (corridor, settlement_instruction) = fx_corridor_fixture(
            source_dataspace,
            destination_dataspace,
            source_sink,
            destination_reserve,
            recipient,
            "fx_non_deploy_first_match",
        );
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let tx = sample_transaction(
            &authority,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "universal").expect("universal domain"),
                ))),
                InstructionBox::from(RegisterSmartContractBytes {
                    code_hash: Hash::new(&code),
                    code,
                }),
                settlement_instruction,
            ],
        );
        let world = crate::state::World::default();
        {
            let view = world.view();
            assert!(rule_matches_with_world(
                &router.policy.rules[0],
                &tx,
                &router.dataspace_catalog,
                &view,
                Some(0),
            ));
            assert!(rule_matches_with_world(
                &router.policy.rules[1],
                &tx,
                &router.dataspace_catalog,
                &view,
                Some(0),
            ));
        }
        let expected = expected_fx_plan(
            source_lane,
            source_dataspace,
            destination_lane,
            destination_dataspace,
        );
        let (queued_plan, block_plan) = fx_route_plan_results(&router, &tx, corridor, world);

        assert_eq!(queued_plan, Ok(expected.clone()));
        assert_eq!(block_plan, Ok(expected));
    }

    #[test]
    fn fx_corridor_universal_deploy_rule_does_not_add_policy_participant() {
        let (authority, authority_keypair) = gen_account_in("wonderland");
        let (source_sink, _) = gen_account_in("wonderland");
        let (destination_reserve, _) = gen_account_in("wonderland");
        let (recipient, _) = gen_account_in("wonderland");
        let source_dataspace = DataSpaceId::new(10);
        let destination_dataspace = DataSpaceId::new(12);
        let source_lane = LaneId::new(3);
        let destination_lane = LaneId::new(4);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: LaneId::SINGLE,
                    dataspace: Some(DataSpaceId::UNIVERSAL),
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("smartcontract::deploy".to_owned()),
                        description: None,
                    },
                }],
            },
            dataspace_catalog(&[(source_dataspace, "cbuae"), (destination_dataspace, "sbp")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (source_lane, source_dataspace),
                (destination_lane, destination_dataspace),
            ]),
        );
        let (corridor, settlement_instruction) = fx_corridor_fixture(
            source_dataspace,
            destination_dataspace,
            source_sink,
            destination_reserve,
            recipient,
            "fx_universal_deploy_policy",
        );
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let tx = sample_transaction(
            &authority,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(RegisterSmartContractBytes {
                    code_hash: Hash::new(&code),
                    code,
                }),
                settlement_instruction,
            ],
        );
        let expected = expected_fx_plan(
            source_lane,
            source_dataspace,
            destination_lane,
            destination_dataspace,
        );
        let (queued_plan, block_plan) =
            fx_route_plan_results(&router, &tx, corridor, crate::state::World::default());

        assert_eq!(queued_plan, Ok(expected.clone()));
        assert_eq!(block_plan, Ok(expected));
    }

    #[test]
    fn fx_corridor_deploy_policy_participant_is_deduplicated_from_intrinsic_participants() {
        let (authority, authority_keypair) = gen_account_in("wonderland");
        let (source_sink, _) = gen_account_in("wonderland");
        let (destination_reserve, _) = gen_account_in("wonderland");
        let (recipient, _) = gen_account_in("wonderland");
        let source_dataspace = DataSpaceId::new(10);
        let destination_dataspace = DataSpaceId::new(12);
        let source_lane = LaneId::new(3);
        let destination_lane = LaneId::new(4);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: source_lane,
                    dataspace: Some(source_dataspace),
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("smartcontract::deploy".to_owned()),
                        description: None,
                    },
                }],
            },
            dataspace_catalog(&[(source_dataspace, "cbuae"), (destination_dataspace, "sbp")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (source_lane, source_dataspace),
                (destination_lane, destination_dataspace),
            ]),
        );
        let (corridor, settlement_instruction) = fx_corridor_fixture(
            source_dataspace,
            destination_dataspace,
            source_sink,
            destination_reserve,
            recipient,
            "fx_duplicate_deploy_policy",
        );
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let tx = sample_transaction(
            &authority,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(RegisterSmartContractBytes {
                    code_hash: Hash::new(&code),
                    code,
                }),
                settlement_instruction,
            ],
        );
        let expected = expected_fx_plan(
            source_lane,
            source_dataspace,
            destination_lane,
            destination_dataspace,
        );
        let (queued_plan, block_plan) =
            fx_route_plan_results(&router, &tx, corridor, crate::state::World::default());

        assert_eq!(queued_plan, Ok(expected.clone()));
        assert_eq!(block_plan, Ok(expected));
    }

    #[test]
    fn fx_corridor_expired_sns_only_alias_is_excluded_with_queue_block_parity() {
        let (authority, authority_keypair) = gen_account_in("wonderland");
        let (source_sink, _) = gen_account_in("wonderland");
        let (destination_reserve, _) = gen_account_in("wonderland");
        let (recipient, _) = gen_account_in("wonderland");
        let source_dataspace = DataSpaceId::new(10);
        let destination_dataspace = DataSpaceId::new(12);
        let expired_dataspace =
            crate::sns::dataspace_id_for_sns_alias("alpha").expect("dynamic dataspace id");
        let source_lane = LaneId::new(3);
        let destination_lane = LaneId::new(4);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: Vec::new(),
            },
            dataspace_catalog(&[(source_dataspace, "cbuae"), (destination_dataspace, "sbp")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (source_lane, source_dataspace),
                (destination_lane, destination_dataspace),
            ]),
        );
        let (corridor, settlement_instruction) = fx_corridor_fixture(
            source_dataspace,
            destination_dataspace,
            source_sink,
            destination_reserve,
            recipient,
            "fx_expired_sns",
        );
        let tx = sample_transaction(
            &authority,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "alpha").expect("SNS-only domain"),
                ))),
                settlement_instruction,
            ],
        );
        let expected = expected_fx_plan(
            source_lane,
            source_dataspace,
            destination_lane,
            destination_dataspace,
        );
        let (queued_plan, block_plan) = fx_route_plan_results(
            &router,
            &tx,
            corridor,
            world_with_dynamic_dataspace_until("alpha", &authority, 0),
        );

        assert_eq!(queued_plan, Ok(expected.clone()));
        assert_eq!(block_plan, Ok(expected));
        assert!(
            !queued_plan
                .expect("queued plan should resolve")
                .legs()
                .iter()
                .any(|leg| leg.route.dataspace_id == expired_dataspace)
        );
    }

    #[test]
    fn fx_corridor_deploy_policy_without_canonical_lane_fails_closed_with_parity() {
        let (authority, authority_keypair) = gen_account_in("wonderland");
        let (source_sink, _) = gen_account_in("wonderland");
        let (destination_reserve, _) = gen_account_in("wonderland");
        let (recipient, _) = gen_account_in("wonderland");
        let source_dataspace = DataSpaceId::new(10);
        let destination_dataspace = DataSpaceId::new(12);
        let deploy_dataspace = DataSpaceId::new(14);
        let source_lane = LaneId::new(3);
        let destination_lane = LaneId::new(4);
        let deploy_lane = LaneId::new(5);
        let lane_catalog = lane_catalog_from_configs(vec![
            LaneConfig {
                id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: "universal".to_owned(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: source_lane,
                dataspace_id: source_dataspace,
                alias: "source".to_owned(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: destination_lane,
                dataspace_id: destination_dataspace,
                alias: "destination".to_owned(),
                ..LaneConfig::default()
            },
            autoscale_elastic_lane_config(deploy_lane, deploy_dataspace, 0),
        ]);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![LaneRoutingRule {
                    lane: deploy_lane,
                    dataspace: Some(deploy_dataspace),
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("smartcontract::deploy".to_owned()),
                        description: None,
                    },
                }],
            },
            dataspace_catalog(&[
                (source_dataspace, "cbuae"),
                (destination_dataspace, "sbp"),
                (deploy_dataspace, "deploy_policy"),
            ]),
            lane_catalog,
        );
        let (corridor, settlement_instruction) = fx_corridor_fixture(
            source_dataspace,
            destination_dataspace,
            source_sink,
            destination_reserve,
            recipient,
            "fx_missing_deploy_lane",
        );
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let tx = sample_transaction(
            &authority,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(RegisterSmartContractBytes {
                    code_hash: Hash::new(&code),
                    code,
                }),
                settlement_instruction,
            ],
        );
        let (queued_plan, block_plan) =
            fx_route_plan_results(&router, &tx, corridor, crate::state::World::default());
        let expected_error = RoutingResolveError::NoLaneForDataspace {
            dataspace_id: deploy_dataspace,
        };

        assert_eq!(queued_plan, Err(expected_error.clone()));
        assert_eq!(block_plan, Err(expected_error));
    }

    #[test]
    fn fx_corridor_plan_includes_smart_contract_deploy_policy_participant() {
        let (authority, authority_keypair) = gen_account_in("wonderland");
        let (source_sink, _) = gen_account_in("wonderland");
        let (destination_reserve, _) = gen_account_in("wonderland");
        let (recipient, _) = gen_account_in("wonderland");
        let source_dataspace = DataSpaceId::new(10);
        let destination_dataspace = DataSpaceId::new(12);
        let contract_dataspace = DataSpaceId::new(14);
        let deploy_policy_dataspace = DataSpaceId::new(16);
        let source_lane = LaneId::new(3);
        let destination_lane = LaneId::new(4);
        let contract_lane = LaneId::new(5);
        let deploy_policy_lane = LaneId::new(6);
        let dataspace_catalog = dataspace_catalog(&[
            (source_dataspace, "cbuae"),
            (destination_dataspace, "sbp"),
            (contract_dataspace, "contracts"),
            (deploy_policy_dataspace, "private_deploy"),
        ]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (source_lane, source_dataspace),
            (destination_lane, destination_dataspace),
            (contract_lane, contract_dataspace),
            (deploy_policy_lane, deploy_policy_dataspace),
        ]);
        let routing_policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: deploy_policy_lane,
                dataspace: Some(deploy_policy_dataspace),
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("smartcontract::deploy".to_owned()),
                    description: None,
                },
            }],
        };
        let router = ConfigLaneRouter::new(routing_policy, dataspace_catalog, lane_catalog);
        let (corridor, settlement_instruction) = fx_corridor_fixture(
            source_dataspace,
            destination_dataspace,
            source_sink,
            destination_reserve,
            recipient,
            "fx_deploy_policy",
        );
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
            &authority,
            0,
            contract_dataspace,
        )
        .expect("contract address");
        let tx = sample_transaction(
            &authority,
            authority_keypair.private_key(),
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
                settlement_instruction,
            ],
        );
        let state = blank_state();
        install_router_nexus(&state, &router);
        install_fx_corridor_policy(&state, corridor);
        let view = state.view();
        let expected = RoutingPlan::native_amx(
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            vec![
                RouteLeg::new(
                    RoutingDecision::new(source_lane, source_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(destination_lane, destination_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(contract_lane, contract_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(deploy_policy_lane, deploy_policy_dataspace),
                    RouteLegRole::Participant,
                ),
            ],
        );

        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &view)
                .expect("state-view FX deployment plan should resolve"),
            expected
        );
        assert_eq!(
            evaluate_policy_plan_with_nexus_and_world_at(
                view.nexus(),
                &tx,
                view.world(),
                state_view_ledger_time_ms(&view),
            )
            .expect("block-time FX deployment plan should resolve"),
            expected
        );
    }

    #[test]
    fn fx_corridor_state_view_plan_includes_active_sns_only_dataspace() {
        let (authority, authority_keypair) = gen_account_in("wonderland");
        let (source_sink, _) = gen_account_in("wonderland");
        let (destination_reserve, _) = gen_account_in("wonderland");
        let (recipient, _) = gen_account_in("wonderland");
        let source_dataspace = DataSpaceId::new(10);
        let destination_dataspace = DataSpaceId::new(12);
        let dynamic_dataspace =
            crate::sns::dataspace_id_for_sns_alias("alpha").expect("dynamic dataspace id");
        let source_lane = LaneId::new(3);
        let destination_lane = LaneId::new(4);
        let dataspace_catalog =
            dataspace_catalog(&[(source_dataspace, "cbuae"), (destination_dataspace, "sbp")]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (source_lane, source_dataspace),
            (destination_lane, destination_dataspace),
        ]);
        let routing_policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let router = ConfigLaneRouter::new(routing_policy, dataspace_catalog, lane_catalog);
        let (corridor, settlement_instruction) = fx_corridor_fixture(
            source_dataspace,
            destination_dataspace,
            source_sink,
            destination_reserve,
            recipient,
            "fx_dynamic_sns",
        );
        let tx = sample_transaction(
            &authority,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("merchant", "alpha").expect("SNS-only domain"),
                ))),
                settlement_instruction,
            ],
        );
        let state = state_from_world(world_with_dynamic_dataspace("alpha", &authority));
        install_router_nexus(&state, &router);
        install_fx_corridor_policy(&state, corridor);
        let view = state.view();
        let queued_plan = router
            .try_route_plan_with_view(&tx, &view)
            .expect("state-view FX plan should resolve the active SNS alias");
        let block_plan = evaluate_policy_plan_with_nexus_and_world_at(
            view.nexus(),
            &tx,
            view.world(),
            state_view_ledger_time_ms(&view),
        )
        .expect("block-time FX plan should resolve the active SNS alias");
        let expected = RoutingPlan::native_amx(
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            vec![
                RouteLeg::new(
                    RoutingDecision::new(source_lane, source_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(destination_lane, destination_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(LaneId::SINGLE, dynamic_dataspace),
                    RouteLegRole::Participant,
                ),
            ],
        );

        assert_eq!(queued_plan, expected);
        assert_eq!(block_plan, expected);
        assert_eq!(queued_plan.digest(), block_plan.digest());
    }

    #[test]
    fn fx_corridor_full_plan_routes_native_amx_from_governed_policy() {
        let (authority, authority_keypair) = gen_account_in("wonderland");
        let (source_sink, _) = gen_account_in("wonderland");
        let (destination_reserve, _) = gen_account_in("wonderland");
        let (recipient, _) = gen_account_in("wonderland");
        let source_dataspace = DataSpaceId::new(10);
        let auxiliary_dataspace = DataSpaceId::new(11);
        let destination_dataspace = DataSpaceId::new(12);
        let source_lane = LaneId::new(3);
        let auxiliary_lane = LaneId::new(5);
        let destination_lane = LaneId::new(4);
        let dataspace_catalog = dataspace_catalog(&[
            (source_dataspace, "cbuae"),
            (auxiliary_dataspace, "sepa"),
            (destination_dataspace, "sbp"),
        ]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (source_lane, source_dataspace),
            (auxiliary_lane, auxiliary_dataspace),
            (destination_lane, destination_dataspace),
        ]);
        let routing_policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let router = ConfigLaneRouter::new(
            routing_policy.clone(),
            dataspace_catalog.clone(),
            lane_catalog.clone(),
        );
        let source_asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("cbuae", "universal").expect("source asset domain"),
            "aed".parse().expect("source asset name"),
        );
        let destination_asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("sbp", "universal").expect("destination asset domain"),
            "pkr".parse().expect("destination asset name"),
        );
        let corridor = FxCorridorPolicy {
            policy_id: "mobile_aed_pkr".parse().expect("FX corridor policy id"),
            revision: 1,
            source_dataspace,
            source: FxCorridorSource::TransactionAuthority,
            source_asset_definition_id: source_asset_definition_id.clone(),
            source_sink: source_sink.clone(),
            destination_dataspace,
            destination_reserve,
            destination_asset_definition_id: destination_asset_definition_id.clone(),
            allowed_destination_alias_domains: BTreeSet::from([
                DomainId::try_new("hbl", "sbp").expect("HBL alias domain"),
                DomainId::try_new("ubl", "sbp").expect("UBL alias domain"),
            ]),
            rate_numerator: 76,
            rate_denominator: 1,
            enabled: true,
        };
        let settlement = SettleFxCorridor {
            policy_id: corridor.policy_id.clone(),
            expected_policy_revision: corridor.revision,
            source_asset_definition_id,
            destination_asset_definition_id,
            settlement_id: "mobile_fx_1".parse().expect("FX settlement id"),
            recipient,
            source_amount: iroha_primitives::numeric::Quantity::from(10_u32),
        };
        let settlement_instruction =
            InstructionBox::from(SettlementInstructionBox::SettleFxCorridor(settlement));
        let bilateral_settlement = InstructionBox::from(DvpIsi::new(
            "mobile_dvp_1".parse().expect("DVP settlement id"),
            SettlementLeg::new(
                AssetDefinitionId::derive_from_components(
                    DomainId::try_new("cash", "cbuae").expect("source DVP asset domain"),
                    "aed".parse().expect("source DVP asset name"),
                ),
                1_u32,
                authority.clone(),
                source_sink.clone(),
            ),
            SettlementLeg::new(
                AssetDefinitionId::derive_from_components(
                    DomainId::try_new("securities", "sepa").expect("auxiliary DVP asset domain"),
                    "bond".parse().expect("auxiliary DVP asset name"),
                ),
                1_u32,
                source_sink,
                authority.clone(),
            ),
            SettlementPlan::default(),
        ));
        let scoped_permission: Permission = CanPublishSpaceDirectoryManifest {
            dataspace: source_dataspace,
        }
        .into();
        let tx = sample_transaction(
            &authority,
            authority_keypair.private_key(),
            vec![
                InstructionBox::from(Grant::account_permission(
                    scoped_permission,
                    authority.clone(),
                )),
                bilateral_settlement,
                settlement_instruction.clone(),
            ],
        );

        let state = blank_state();
        install_router_nexus(&state, &router);
        let mut registry = FxCorridorPolicyRegistry::default();
        registry.upsert(corridor);
        {
            let mut world = state.world.block();
            world.parameters.get_mut().set_parameter(
                iroha_data_model::parameter::Parameter::Custom(registry.into_custom_parameter()),
            );
            world.commit();
        }

        let expected = RoutingPlan::native_amx(
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            vec![
                RouteLeg::new(
                    RoutingDecision::new(source_lane, source_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(auxiliary_lane, auxiliary_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(destination_lane, destination_dataspace),
                    RouteLegRole::Participant,
                ),
            ],
        );

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("FX route state requirement should be deterministic"),
            None
        );
        assert_eq!(
            router
                .try_route_with_state(&tx, &state)
                .expect("legacy FX coordinator route should resolve with state"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            router
                .try_route_plan_without_state(&tx)
                .expect("FX route state requirement should be deterministic"),
            None
        );
        assert_eq!(
            router
                .try_route_plan_with_state(&tx, &state)
                .expect("state-backed FX plan should resolve"),
            expected
        );
        let view = state.view();
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &view)
                .expect("state-view FX plan should resolve"),
            expected
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &routing_policy,
                &lane_catalog,
                &dataspace_catalog,
                &tx,
                view.world(),
            )
            .expect("world-backed FX plan should resolve"),
            expected
        );

        let mut strict_metadata = Metadata::default();
        strict_metadata.insert(
            AMX_POLICY_METADATA_KEY.parse().expect("amx policy key"),
            iroha_primitives::json::Json::new(AMX_POLICY_REJECT_CROSS_DATASPACE),
        );
        let strict_tx = sample_transaction_with_metadata(
            &authority,
            authority_keypair.private_key(),
            vec![settlement_instruction],
            strict_metadata,
        );
        assert_eq!(
            router.try_route_plan_with_state(&strict_tx, &state),
            Err(
                RoutingResolveError::ConflictingTransactionDataspaceTargets {
                    first_dataspace_id: source_dataspace,
                    second_dataspace_id: destination_dataspace,
                }
            )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Burn::asset_quantity(
                1_u32,
                AssetId::of(asset_definition.clone(), alice_id.clone()),
            ))],
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
    fn asset_home_extra_coverage_mint_permissions_use_stored_alias_dataspace() {
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
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("cash", "universal").expect("asset definition domain"),
            "pkr".parse().expect("asset definition name"),
        );
        let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
            asset_definition.clone(),
            crate::state::AssetDefinitionAliasBindingRecord {
                alias,
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );

        let permissions = [
            Permission::from(CanMintAssetWithDefinition {
                asset_definition: asset_definition.clone(),
            }),
            Permission::from(CanMintAssetToAccount {
                asset_definition,
                account: bob_id.clone(),
            }),
        ];
        for permission in permissions {
            let tx = sample_transaction(
                &alice_id,
                alice_keypair.private_key(),
                vec![InstructionBox::from(Grant::account_permission(
                    permission,
                    bob_id.clone(),
                ))],
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("cash", "paynet").expect("asset definition domain"),
                "xor".parse().expect("asset definition name"),
            );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_quantity(
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
                AssetDefinition::numeric(
                    opaque_asset_definition,
                    "xor".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    opaque_asset_definition,
                    "xor".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            ))],
        );
        let alias: AssetDefinitionAlias = "xor#paynet".parse().expect("asset alias");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    opaque_asset_definition.clone(),
                    "xor".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    opaque_asset_definition,
                    "xor".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
            vec![InstructionBox::from(Transfer::asset_quantity(
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
                1_u32,
                receiver_id,
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    opaque_asset_definition,
                    "xor".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
            vec![InstructionBox::from(Burn::asset_quantity(
                1_u32,
                AssetId::of(opaque_asset_definition.clone(), sender_id.clone()),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    opaque_asset_definition,
                    "xor".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("cash", "universal").expect("asset definition domain"),
                "pkr".parse().expect("asset definition name"),
            );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_quantity(
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
        let owning_domain = DomainId::try_new("cash", "paynet").expect("owning domain");
        let mut state = state_with_asset_definitions(
            vec![
                AssetDefinition::numeric(
                    opaque_asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::DataspaceRestricted,
                    Some(owning_domain),
                )
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("cash", "paynet").expect("asset definition domain"),
                "pkr".parse().expect("asset definition name"),
            );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_quantity(
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("cash", "paynet").expect("asset definition domain"),
                "pkr".parse().expect("asset definition name"),
            );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_quantity(
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
        let transparent_asset_definition =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("cash", "paynet").expect("asset definition domain"),
                "pkr".parse().expect("asset definition name"),
            );
        let opaque_asset_definition = AssetDefinitionId::parse_address_literal(
            &transparent_asset_definition.canonical_address(),
        )
        .expect("opaque canonical asset definition id");
        let transfer = Transfer::asset_quantity(
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
    fn mixed_native_and_contract_batch_preserves_all_dataspace_targets() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let native_dataspace = DataSpaceId::new(7);
        let contract_dataspace = DataSpaceId::new(9);
        let (policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
        let executable = Executable::Batch(
            vec![
                ExecutableBatchItem::Instruction(InstructionBox::from(Register::domain(
                    Domain::new(DomainId::try_new("merchant", "signer").expect("native domain id")),
                ))),
                ExecutableBatchItem::ContractCall(sample_contract_invocation(
                    &authority_id,
                    contract_dataspace,
                    77,
                )),
            ]
            .into(),
        );
        let tx = sample_executable_transaction(
            &authority_id,
            authority_keypair.private_key(),
            executable.clone(),
        );
        let expected = RoutingPlan::native_amx(
            RoutingDecision::new(LaneId::new(2), native_dataspace),
            vec![
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(2), native_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(4), contract_dataspace),
                    RouteLegRole::Participant,
                ),
            ],
        );

        assert_eq!(
            router
                .try_route_plan(&tx)
                .expect("mixed batch must retain native and contract targets"),
            expected
        );

        let state = blank_state();
        install_router_nexus(&state, &router);
        let state_view = state.view();
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                &lane_catalog,
                &catalog,
                &tx,
                state_view.world(),
            )
            .expect("world-backed mixed-batch routing must retain every target"),
            expected
        );

        let mut metadata = Metadata::default();
        metadata.insert(
            AMX_POLICY_METADATA_KEY.parse().expect("amx policy key"),
            iroha_primitives::json::Json::new(AMX_POLICY_REJECT_CROSS_DATASPACE),
        );
        let strict_tx = sample_executable_transaction_with_metadata(
            &authority_id,
            authority_keypair.private_key(),
            executable,
            metadata,
        );
        assert_eq!(
            router.try_route_plan(&strict_tx),
            Err(
                RoutingResolveError::ConflictingTransactionDataspaceTargets {
                    first_dataspace_id: native_dataspace,
                    second_dataspace_id: contract_dataspace,
                }
            )
        );
    }

    #[test]
    fn primary_alias_compare_and_set_across_dataspaces_builds_native_amx_plan() {
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
        let expected = resolved_account_alias("merchant@acme", first_dataspace);
        let replacement = resolved_account_alias("merchant@bank", second_dataspace);
        let instruction = CompareAndSetPrimaryAccountAlias::new(
            authority_id.clone(),
            Some(expected.clone()),
            Some(replacement.clone()),
        );
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(instruction.clone())],
        );

        let expected_plan = RoutingPlan::native_amx(
            RoutingDecision::new(LaneId::new(2), first_dataspace),
            vec![
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(2), first_dataspace),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(3), second_dataspace),
                    RouteLegRole::Participant,
                ),
            ],
        );
        assert_eq!(
            router
                .try_route_plan(&tx)
                .expect("cross-dataspace primary alias change must route through native AMX"),
            expected_plan
        );

        let reversed = CompareAndSetPrimaryAccountAlias::new(
            authority_id.clone(),
            Some(replacement),
            Some(expected),
        );
        let reversed_tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(reversed)],
        );
        assert_eq!(
            router
                .try_route_plan(&reversed_tx)
                .expect("alias ordering must not change the native AMX route"),
            expected_plan
        );

        let state = blank_state();
        install_router_nexus(&state, &router);
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("world-aware routing must preserve both alias dataspaces"),
            expected_plan
        );

        let proved_tx = sample_executable_transaction(
            &authority_id,
            authority_keypair.private_key(),
            sample_proved_executable(vec![InstructionBox::from(instruction)]),
        );
        assert_eq!(
            router
                .try_route_plan(&proved_tx)
                .expect("proved overlays must preserve both alias dataspaces"),
            expected_plan
        );
    }

    #[test]
    fn primary_alias_compare_and_set_same_dataspace_stays_single_route() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let dataspace = DataSpaceId::new(7);
        let router = ConfigLaneRouter::new(
            LaneRoutingPolicy {
                default_lane: LaneId::SINGLE,
                default_dataspace: DataSpaceId::UNIVERSAL,
                rules: vec![],
            },
            dataspace_catalog(&[(dataspace, "acme")]),
            catalog_with_lane_dataspaces(&[
                (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                (LaneId::new(2), dataspace),
            ]),
        );
        let instruction = CompareAndSetPrimaryAccountAlias::new(
            authority_id.clone(),
            Some(resolved_account_alias("old@acme", dataspace)),
            Some(resolved_account_alias("new@acme", dataspace)),
        );
        let tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(instruction)],
        );

        assert_eq!(
            router
                .try_route_plan(&tx)
                .expect("same-dataspace alias change must remain local"),
            RoutingPlan::single(RoutingDecision::new(LaneId::new(2), dataspace))
        );

        let empty = CompareAndSetPrimaryAccountAlias::new(authority_id.clone(), None, None);
        let empty_tx = sample_transaction(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(empty)],
        );
        assert_eq!(
            router
                .try_route_plan(&empty_tx)
                .expect("empty compare-and-set must keep account fallback routing"),
            RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL,))
        );
    }

    #[test]
    fn strict_amx_policy_rejects_cross_dataspace_primary_alias_compare_and_set() {
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
        let instruction = CompareAndSetPrimaryAccountAlias::new(
            authority_id.clone(),
            Some(resolved_account_alias("merchant@acme", first_dataspace)),
            Some(resolved_account_alias("merchant@bank", second_dataspace)),
        );
        let tx = sample_transaction_with_metadata(
            &authority_id,
            authority_keypair.private_key(),
            vec![InstructionBox::from(instruction)],
            metadata,
        );

        assert_eq!(
            router.try_route_plan(&tx),
            Err(
                RoutingResolveError::ConflictingTransactionDataspaceTargets {
                    first_dataspace_id: first_dataspace,
                    second_dataspace_id: second_dataspace,
                }
            )
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

        let asset_definition: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("uae", "universal").unwrap(),
                "aed".parse().unwrap(),
            );
        let uae_transfer = Transfer::asset_quantity(
            AssetId::of(asset_definition.clone(), uae_sender_id.clone()),
            1_u32,
            acme_receiver_id.clone(),
        );
        let bank_transfer = Transfer::asset_quantity(
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
    fn resolve_query_routing_decision_rejects_autoscale_owned_default_lane_without_state() {
        let (alice_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(1),
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);

        assert_eq!(
            resolve_query_routing_decision(
                &policy,
                &lane_catalog,
                &DataSpaceCatalog::default(),
                &alice_id,
                None,
            ),
            Err(RoutingResolveError::AutoscaleOwnedDefaultLane {
                lane_id: LaneId::new(1),
            }),
            "state-free query routing must not accept autoscale-owned default lanes"
        );
    }

    #[test]
    fn resolve_query_routing_decision_rejects_autoscale_owned_rule_lane_without_state() {
        let (alice_id, _) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::UNIVERSAL),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };
        let lane_catalog = lane_catalog_from_configs(vec![
            default_lane_config(),
            autoscale_elastic_lane_config(LaneId::new(1), DataSpaceId::UNIVERSAL, 7),
        ]);

        assert_eq!(
            resolve_query_routing_decision(
                &policy,
                &lane_catalog,
                &DataSpaceCatalog::default(),
                &alice_id,
                None,
            ),
            Err(RoutingResolveError::AutoscaleOwnedRuleLane {
                lane_id: LaneId::new(1),
            }),
            "state-free query routing must not accept autoscale-owned explicit rule lanes"
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

        let uaid_scoped_tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                CanPublishSpaceDirectoryManifestForUaid {
                    dataspace,
                    uaid: UniversalAccountId::from_hash(Hash::new(
                        b"uaid::dataspace-scoped-permission-route",
                    )),
                },
                alice_id.clone(),
            ))],
        );
        assert_eq!(
            router
                .try_route(&uaid_scoped_tx)
                .expect("UAID-scoped permission should resolve"),
            RoutingDecision::new(lane, dataspace),
        );

        let domain_scoped_tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::account_permission(
                CanPublishSpaceDirectoryManifestForAccountDomain {
                    dataspace,
                    domain: DomainId::try_new("hbl", "manifest").expect("HBL manifest domain"),
                },
                alice_id.clone(),
            ))],
        );
        assert_eq!(
            router
                .try_route(&domain_scoped_tx)
                .expect("account-domain-scoped permission should resolve"),
            RoutingDecision::new(lane, dataspace),
        );

        let role_id: RoleId = "hbl_manifest_publishers".parse().expect("role id");
        let role_scoped_tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Grant::role_permission(
                CanPublishSpaceDirectoryManifestForAccountDomain {
                    dataspace,
                    domain: DomainId::try_new("hbl", "manifest").expect("HBL manifest domain"),
                },
                role_id,
            ))],
        );
        assert_eq!(
            router
                .try_route_without_state(&role_scoped_tx)
                .expect("role permission routing should resolve from its dataspace payload"),
            Some(RoutingDecision::new(lane, dataspace)),
        );
    }

    #[test]
    fn space_directory_manifest_writes_route_by_manifest_dataspace() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let dataspace = DataSpaceId::new(10);
        let lane = LaneId::new(3);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane, dataspace),
        ]);
        let dataspace_catalog = dataspace_catalog(&[(dataspace, "sbp")]);
        let router = ConfigLaneRouter::new(policy.clone(), dataspace_catalog, lane_catalog.clone());
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::default(),
            uaid: UniversalAccountId::from_hash(Hash::new(b"router::space-directory-publish")),
            dataspace,
            issued_ms: 0,
            activation_epoch: 0,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(PublishSpaceDirectoryManifest {
                manifest: manifest.clone(),
            })],
        );
        let expected = RoutingDecision::new(lane, dataspace);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("space-directory publish should route without WSV state"),
            Some(expected)
        );
        assert_eq!(
            router
                .try_route_plan_without_state(&tx)
                .expect("space-directory publish plan should route without WSV state")
                .map(|plan| plan.coordinator_route()),
            Some(expected)
        );
        assert_eq!(
            evaluate_policy_with_catalog(
                &policy,
                &lane_catalog,
                router.dataspace_catalog.as_ref(),
                &tx,
            )
            .expect("validation routing should match queue routing"),
            expected
        );
    }

    #[test]
    fn mixed_activation_followups_plan_routes_space_directory_publish_to_private_lane() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let dataspace = DataSpaceId::new(10);
        let lane = LaneId::new(3);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (lane, dataspace),
        ]);
        let router = ConfigLaneRouter::new(
            policy,
            dataspace_catalog(&[(dataspace, "sbp")]),
            lane_catalog,
        );
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::default(),
            uaid: UniversalAccountId::from_hash(Hash::new(b"router::activation-followup")),
            dataspace,
            issued_ms: 0,
            activation_epoch: 0,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new("activation-followup", "universal").expect("domain id"),
                ))),
                InstructionBox::from(PublishSpaceDirectoryManifest { manifest }),
            ],
        );

        let plan = router
            .try_route_plan(&tx)
            .expect("mixed activation follow-up plan should resolve");
        let RoutingPlan::NativeAmx(plan) = plan else {
            panic!("mixed universal and SBP follow-ups should build a native AMX plan");
        };

        assert_eq!(
            plan.coordinator.route,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert!(
            plan.participants
                .iter()
                .any(|leg| leg.route == RoutingDecision::new(lane, dataspace)),
            "SBP publish leg must be retained in the routing plan"
        );
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
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition,
                    "ds1".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition,
                    "ds1".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition,
                    "voucher".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
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
    fn account_alias_resolution_delegation_routes_by_exact_scope() {
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
        let permission = Permission::from(CanDelegateAccountAliasResolution {
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
                .expect("alias-resolution delegation should route by its exact scope"),
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
    fn multisig_contract_trigger_proposal_routes_by_immutable_contract_dataspace() {
        let (submitter_id, submitter_keypair) = gen_account_in("submitter");
        let (multisig_id, _) = gen_account_in("multisig");
        let (policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
        let contract_dataspace = DataSpaceId::new(9);
        let proposed = vec![
            sample_contract_trigger_registration(
                &multisig_id,
                "proposal_contract_call",
                contract_dataspace,
                1,
            ),
            InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(
                "proposal_contract_call".parse().expect("trigger id"),
            )),
        ];
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigPropose::new(
                multisig_id.clone(),
                proposed,
                None,
            ))],
        );
        let state = state_with_account_scope_entries(
            &[
                (submitter_id, account_scope_entry(DataSpaceId::new(7))),
                (multisig_id, account_scope_entry(DataSpaceId::new(8))),
            ],
            catalog,
        );
        state.nexus.write().lane_catalog = lane_catalog;
        let expected_route = RoutingDecision::new(LaneId::new(4), contract_dataspace);
        let expected_plan = RoutingPlan::single(expected_route);

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("proposal must route by the immutable contract address"),
            expected_route
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("proposal plan must route by the immutable contract address"),
            expected_plan
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("world-backed proposal routing must match queue routing"),
            expected_plan
        );
        assert_eq!(
            native_amx_participant_dataspaces_with_world(
                &tx,
                &state.view().nexus().dataspace_catalog,
                state.view().world(),
            ),
            vec![contract_dataspace]
        );
    }

    #[test]
    fn multisig_contract_trigger_same_transaction_approval_keeps_contract_route() {
        let (submitter_id, submitter_keypair) = gen_account_in("submitter");
        let (multisig_id, _) = gen_account_in("multisig");
        let (policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
        let contract_dataspace = DataSpaceId::new(9);
        let proposed = vec![
            sample_contract_trigger_registration(
                &multisig_id,
                "same_transaction_contract_call",
                contract_dataspace,
                2,
            ),
            InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(
                "same_transaction_contract_call"
                    .parse()
                    .expect("trigger id"),
            )),
        ];
        let instructions_hash = HashOf::new(&proposed);
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![
                InstructionBox::from(MultisigPropose::new(multisig_id.clone(), proposed, None)),
                InstructionBox::from(MultisigApprove::new(multisig_id.clone(), instructions_hash)),
            ],
        );
        let state = state_with_account_scope_entries(
            &[
                (submitter_id, account_scope_entry(DataSpaceId::new(7))),
                (multisig_id, account_scope_entry(DataSpaceId::new(8))),
            ],
            catalog,
        );
        state.nexus.write().lane_catalog = lane_catalog;
        let expected =
            RoutingPlan::single(RoutingDecision::new(LaneId::new(4), contract_dataspace));

        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("sibling approval must inherit the proposal contract route"),
            expected
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("world-backed sibling approval must use the contract route"),
            expected
        );
        assert_eq!(
            native_amx_participant_dataspaces_with_world(
                &tx,
                &state.view().nexus().dataspace_catalog,
                state.view().world(),
            ),
            vec![contract_dataspace],
            "the sibling approval must not add the multisig account as a participant"
        );
    }

    #[test]
    fn multisig_contract_trigger_later_approval_reads_persisted_contract_route() {
        let (submitter_id, submitter_keypair) = gen_account_in("submitter");
        let (multisig_id, _) = gen_account_in("multisig");
        let (policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
        let contract_dataspace = DataSpaceId::new(9);
        let proposed = vec![
            sample_contract_trigger_registration(
                &multisig_id,
                "persisted_contract_call",
                contract_dataspace,
                3,
            ),
            InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(
                "persisted_contract_call".parse().expect("trigger id"),
            )),
        ];
        let instructions_hash = HashOf::new(&proposed);
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigApprove::new(
                multisig_id.clone(),
                instructions_hash,
            ))],
        );
        let mut state = state_with_account_scope_entries(
            &[
                (submitter_id, account_scope_entry(DataSpaceId::new(7))),
                (
                    multisig_id.clone(),
                    account_scope_entry(DataSpaceId::new(8)),
                ),
            ],
            catalog,
        );
        state.nexus.write().lane_catalog = lane_catalog;
        let proposal_state = MultisigProposalState::new(
            multisig_id.clone(),
            instructions_hash,
            proposed,
            1,
            10_000,
            BTreeSet::new(),
            None,
        );
        state.world.smart_contract_state_mut_for_testing().insert(
            multisig_proposal_state_key(&multisig_id, &instructions_hash),
            norito::to_bytes(&proposal_state).expect("proposal state should encode"),
        );
        let expected =
            RoutingPlan::single(RoutingDecision::new(LaneId::new(4), contract_dataspace));

        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("persisted proposal must override the multisig account route"),
            expected
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("world-backed approval must read the persisted contract route"),
            expected
        );
    }

    #[test]
    fn nested_trigger_instruction_and_proved_overlay_route_to_contract_dataspace() {
        let (submitter_id, submitter_keypair) = gen_account_in("submitter");
        let (_policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
        let contract_dataspace = DataSpaceId::new(9);
        let inner = sample_contract_trigger_registration(
            &submitter_id,
            "nested_contract_call",
            contract_dataspace,
            4,
        );
        let proved = sample_proved_executable(vec![inner]);
        let outer = sample_trigger_registration(
            &submitter_id,
            "proved_contract_wrapper",
            Executable::Instructions(
                vec![sample_trigger_registration(
                    &submitter_id,
                    "instruction_contract_wrapper",
                    proved,
                )]
                .into(),
            ),
        );
        let tx = sample_transaction(&submitter_id, submitter_keypair.private_key(), vec![outer]);
        let state = state_with_account_scope_entries(
            &[(submitter_id, account_scope_entry(DataSpaceId::new(7)))],
            catalog,
        );
        state.nexus.write().lane_catalog = lane_catalog;

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("contract-only nested trigger routing must be state-free"),
            Some(RoutingDecision::new(LaneId::new(4), contract_dataspace))
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("nested trigger executable must resolve recursively"),
            RoutingDecision::new(LaneId::new(4), contract_dataspace)
        );
    }

    #[test]
    fn conflicting_nested_contract_triggers_build_amx_plan_or_fail_strictly() {
        let (submitter_id, submitter_keypair) = gen_account_in("submitter");
        let (policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
        let multisig_dataspace = DataSpaceId::new(8);
        let contract_dataspace = DataSpaceId::new(9);
        let nested = vec![
            sample_contract_trigger_registration(
                &submitter_id,
                "first_nested_contract",
                multisig_dataspace,
                5,
            ),
            sample_contract_trigger_registration(
                &submitter_id,
                "second_nested_contract",
                contract_dataspace,
                6,
            ),
        ];
        let outer = sample_trigger_registration(
            &submitter_id,
            "cross_dataspace_contract_wrapper",
            Executable::Instructions(nested.into()),
        );
        let state = state_with_account_scope_entries(
            &[(
                submitter_id.clone(),
                account_scope_entry(DataSpaceId::new(7)),
            )],
            catalog,
        );
        state.nexus.write().lane_catalog = lane_catalog;
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![outer.clone()],
        );

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("mixed nested contract targets must use the universal coordinator"),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        let plan = router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("mixed nested contract targets must build an AMX plan");
        let RoutingPlan::NativeAmx(plan) = plan else {
            panic!("mixed nested contract targets must not collapse to a single route");
        };
        assert_eq!(
            plan.coordinator.route,
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            plan.participants
                .iter()
                .map(|participant| participant.route.dataspace_id)
                .collect::<Vec<_>>(),
            vec![multisig_dataspace, contract_dataspace]
        );
        assert_eq!(
            native_amx_participant_dataspaces_with_world(
                &tx,
                &state.view().nexus().dataspace_catalog,
                state.view().world(),
            ),
            vec![multisig_dataspace, contract_dataspace]
        );
        assert!(matches!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            ),
            Ok(RoutingPlan::NativeAmx(_))
        ));

        let mut strict_metadata = Metadata::default();
        strict_metadata.insert(
            AMX_POLICY_METADATA_KEY.parse().expect("amx policy key"),
            iroha_primitives::json::Json::new(AMX_POLICY_REJECT_CROSS_DATASPACE),
        );
        let strict_tx = sample_transaction_with_metadata(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![outer],
            strict_metadata,
        );
        assert_eq!(
            router.try_route_plan_with_view(&strict_tx, &state.view()),
            Err(
                RoutingResolveError::ConflictingTransactionDataspaceTargets {
                    first_dataspace_id: multisig_dataspace,
                    second_dataspace_id: contract_dataspace,
                }
            )
        );
    }

    #[test]
    fn non_contract_trigger_keeps_multisig_account_fallback() {
        let (submitter_id, submitter_keypair) = gen_account_in("submitter");
        let (multisig_id, _) = gen_account_in("multisig");
        let (_policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
        let proposed = vec![
            sample_trigger_registration(
                &multisig_id,
                "non_contract_proved_trigger",
                sample_proved_executable(Vec::new()),
            ),
            InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(
                "non_contract_proved_trigger".parse().expect("trigger id"),
            )),
        ];
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigPropose::new(
                multisig_id.clone(),
                proposed,
                None,
            ))],
        );
        let state = state_with_account_scope_entries(
            &[
                (submitter_id, account_scope_entry(DataSpaceId::new(7))),
                (multisig_id, account_scope_entry(DataSpaceId::new(8))),
            ],
            catalog,
        );
        state.nexus.write().lane_catalog = lane_catalog;

        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("targetless trigger must retain the multisig account fallback"),
            RoutingDecision::new(LaneId::new(3), DataSpaceId::new(8))
        );
    }

    #[test]
    fn multisig_propose_routes_by_embedded_instruction_dataspace() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
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
        let proposed = vec![InstructionBox::from(Register::account(
            Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
        ))];
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigPropose::new(
                multisig_id,
                proposed,
                None,
            ))],
        );
        let state = state_with_account_scope_entries(&[], catalog);
        state.nexus.write().lane_catalog = lane_catalog;
        let expected_route = RoutingDecision::new(lane_id, dataspace_id);
        let expected_plan = RoutingPlan::single(expected_route);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("multisig proposal should defer to state-aware routing"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("embedded proposal target should route to its dataspace"),
            expected_route
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("embedded proposal plan should route to its dataspace"),
            expected_plan
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation routing should match proposal routing"),
            expected_route
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation routing plan should match proposal routing plan"),
            expected_plan
        );
    }

    #[test]
    fn multisig_propose_plan_prefers_embedded_dataspace_over_multiscope_account() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
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
        let proposed = vec![InstructionBox::from(Register::account(
            Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
        ))];
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigPropose::new(
                multisig_id.clone(),
                proposed,
                None,
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(DataSpaceId::UNIVERSAL);
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(&[(multisig_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog;
        let expected_route = RoutingDecision::new(lane_id, dataspace_id);
        let expected_plan = RoutingPlan::single(expected_route);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("multisig proposal should defer to state-aware routing"),
            None
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("proposal plan should use the embedded write dataspace"),
            expected_plan
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation plan should use the embedded write dataspace"),
            expected_plan
        );
    }

    #[test]
    fn multisig_same_transaction_approve_uses_sibling_proposal_route() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
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
        let proposed = vec![InstructionBox::from(Register::account(
            Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
        ))];
        let proposal_hash = HashOf::new(&proposed);
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![
                InstructionBox::from(MultisigPropose::new(multisig_id.clone(), proposed, None)),
                InstructionBox::from(MultisigApprove::new(multisig_id.clone(), proposal_hash)),
            ],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(DataSpaceId::UNIVERSAL);
        let state = state_with_account_scope_entries(&[(multisig_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog;
        let expected_plan = RoutingPlan::single(RoutingDecision::new(lane_id, dataspace_id));

        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("same-transaction approval should use the sibling proposal route"),
            expected_plan
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation plan should use the sibling proposal route"),
            expected_plan
        );
    }

    #[test]
    fn custom_multisig_propose_defers_and_routes_by_embedded_instruction_dataspace() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
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
        let proposed = vec![InstructionBox::from(Register::account(
            Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
        ))];
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(CustomInstruction::new(
                iroha_primitives::json::Json::new(MultisigInstructionBox::Propose(
                    MultisigPropose::new(multisig_id, proposed, None),
                )),
            ))],
        );
        let state = state_with_account_scope_entries(&[], catalog);
        state.nexus.write().lane_catalog = lane_catalog;

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("custom multisig proposal should defer to state-aware routing"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("custom embedded proposal target should route to its dataspace"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation routing should match custom proposal routing"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn multisig_approve_routes_by_multisig_account_scope() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
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
        let proposed = vec![InstructionBox::from(Register::account(
            Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
        ))];
        let instructions_hash = HashOf::new(&proposed);
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigApprove::new(
                multisig_id.clone(),
                instructions_hash,
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(&[(multisig_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog;

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("multisig approval should defer to state-aware routing"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("approval should route by multisig account scope"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation routing should match approval routing"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn custom_multisig_approve_defers_and_routes_by_multisig_account_scope() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
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
        let proposed = vec![InstructionBox::from(Register::account(
            Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
        ))];
        let instructions_hash = HashOf::new(&proposed);
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(CustomInstruction::new(
                iroha_primitives::json::Json::new(MultisigInstructionBox::Approve(
                    MultisigApprove::new(multisig_id.clone(), instructions_hash),
                )),
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let state = state_with_account_scope_entries(&[(multisig_id, scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog;

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("custom multisig approval should defer to state-aware routing"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("custom approval should route by multisig account scope"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation routing should match custom approval routing"),
            RoutingDecision::new(lane_id, dataspace_id)
        );
    }

    #[test]
    fn multisig_approve_routes_by_persisted_proposal_when_scope_is_missing() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
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
        let proposed = vec![InstructionBox::from(Register::account(
            Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
        ))];
        let instructions_hash = HashOf::new(&proposed);
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigApprove::new(
                multisig_id.clone(),
                instructions_hash,
            ))],
        );
        let mut state = state_with_account_scope_entries(&[], catalog);
        state.nexus.write().lane_catalog = lane_catalog;
        let proposal_state = MultisigProposalState::new(
            multisig_id.clone(),
            instructions_hash,
            proposed,
            1,
            10_000,
            BTreeSet::new(),
            None,
        );
        state.world.smart_contract_state_mut_for_testing().insert(
            multisig_proposal_state_key(&multisig_id, &instructions_hash),
            norito::to_bytes(&proposal_state).expect("proposal state should encode"),
        );
        let expected_route = RoutingDecision::new(lane_id, dataspace_id);
        let expected_plan = RoutingPlan::single(expected_route);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("multisig approval should defer to state-aware routing"),
            None
        );
        assert_eq!(
            router.try_route_with_view(&tx, &state.view()).expect(
                "approval should route by embedded proposal target when account scope is absent"
            ),
            expected_route
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("approval plan should route by embedded proposal target"),
            expected_plan
        );
        assert_eq!(
            evaluate_policy_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation routing should match proposal-state fallback routing"),
            expected_route
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation routing plan should match proposal-state fallback routing"),
            expected_plan
        );
    }

    #[test]
    fn multisig_approve_ignores_corrupt_proposal_state_and_uses_account_scope() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
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
        let proposed = vec![InstructionBox::from(Register::account(
            Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
        ))];
        let instructions_hash = HashOf::new(&proposed);
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigApprove::new(
                multisig_id.clone(),
                instructions_hash,
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state =
            state_with_account_scope_entries(&[(multisig_id.clone(), scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog;
        state.world.smart_contract_state_mut_for_testing().insert(
            multisig_proposal_state_key(&multisig_id, &instructions_hash),
            b"not a multisig proposal state".to_vec(),
        );
        let expected_route = RoutingDecision::new(lane_id, dataspace_id);
        let expected_plan = RoutingPlan::single(expected_route);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("multisig approval should defer to state-aware routing"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("corrupt proposal state should fall back to multisig account scope"),
            expected_route
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("corrupt proposal state plan should fall back to multisig account scope"),
            expected_plan
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation routing should ignore corrupt proposal state")
            .coordinator_route(),
            expected_route
        );
    }

    #[test]
    fn multisig_approve_ignores_unrelated_persisted_proposal_hash() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (approved_target_id, _) = gen_account_in("wonderland");
        let (stale_target_id, _) = gen_account_in("wonderland");
        let account_dataspace = DataSpaceId::new(10);
        let stale_dataspace = DataSpaceId::new(11);
        let account_lane = LaneId::new(2);
        let stale_lane = LaneId::new(3);
        let catalog = dataspace_catalog(&[
            (account_dataspace, "restricted"),
            (stale_dataspace, "stale-restricted"),
        ]);
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (account_lane, account_dataspace),
            (stale_lane, stale_dataspace),
        ]);
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: Vec::new(),
        };
        let router = ConfigLaneRouter::new(policy.clone(), catalog.clone(), lane_catalog.clone());
        let approved = vec![InstructionBox::from(Register::account(
            Account::new(approved_target_id)
                .with_label(Some(account_alias("approved@restricted", &catalog))),
        ))];
        let approved_hash = HashOf::new(&approved);
        let stale_proposed = vec![InstructionBox::from(Register::account(
            Account::new(stale_target_id)
                .with_label(Some(account_alias("stale@stale-restricted", &catalog))),
        ))];
        let stale_hash = HashOf::new(&stale_proposed);
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigApprove::new(
                multisig_id.clone(),
                approved_hash,
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(account_dataspace);
        let mut state =
            state_with_account_scope_entries(&[(multisig_id.clone(), scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog;
        let stale_state = MultisigProposalState::new(
            multisig_id.clone(),
            stale_hash,
            stale_proposed,
            1,
            10_000,
            BTreeSet::new(),
            None,
        );
        state.world.smart_contract_state_mut_for_testing().insert(
            multisig_proposal_state_key(&multisig_id, &stale_hash),
            norito::to_bytes(&stale_state).expect("stale proposal state should encode"),
        );
        let expected_route = RoutingDecision::new(account_lane, account_dataspace);
        let expected_plan = RoutingPlan::single(expected_route);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("multisig approval should defer to state-aware routing"),
            None
        );
        assert_eq!(
            router
                .try_route_with_view(&tx, &state.view())
                .expect("unrelated proposal state should not route this approval"),
            expected_route
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("unrelated proposal state plan should fall back to account scope"),
            expected_plan
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation routing should ignore unrelated proposal state")
            .coordinator_route(),
            expected_route
        );
    }

    #[test]
    fn multisig_approve_plan_prefers_visible_proposal_over_multiscope_account() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (multisig_id, _) = gen_account_in("wonderland");
        let (target_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let lane_id = LaneId::new(2);
        let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
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
        let proposed = vec![InstructionBox::from(Register::account(
            Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
        ))];
        let instructions_hash = HashOf::new(&proposed);
        let tx = sample_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            vec![InstructionBox::from(MultisigApprove::new(
                multisig_id.clone(),
                instructions_hash,
            ))],
        );
        let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
        scope_entry.ensure_dataspace(DataSpaceId::UNIVERSAL);
        scope_entry.ensure_dataspace(dataspace_id);
        let mut state =
            state_with_account_scope_entries(&[(multisig_id.clone(), scope_entry)], catalog);
        state.nexus.write().lane_catalog = lane_catalog;
        let proposal_state = MultisigProposalState::new(
            multisig_id.clone(),
            instructions_hash,
            proposed,
            1,
            10_000,
            BTreeSet::new(),
            None,
        );
        state.world.smart_contract_state_mut_for_testing().insert(
            multisig_proposal_state_key(&multisig_id, &instructions_hash),
            norito::to_bytes(&proposal_state).expect("proposal state should encode"),
        );
        let expected_route = RoutingDecision::new(lane_id, dataspace_id);
        let expected_plan = RoutingPlan::single(expected_route);

        assert_eq!(
            router
                .try_route_without_state(&tx)
                .expect("multisig approval should defer to state-aware routing"),
            None
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .expect("visible proposal should override multiscope account route"),
            expected_plan
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .expect("validation plan should prefer visible proposal target"),
            expected_plan
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition,
                    "voucher".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition,
                    "voucher".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
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
        let transparent_asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    opaque_asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
        let asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    asset_definition,
                    "voucher".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
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
        let transparent_asset_definition = AssetDefinitionId::derive_from_components(
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
                AssetDefinition::numeric(
                    opaque_asset_definition.clone(),
                    "pkr".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )
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
