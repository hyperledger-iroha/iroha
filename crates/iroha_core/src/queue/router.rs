//! Lane and dataspace routing utilities for the transaction queue.
//!
//! These helpers translate pending transactions into the lane/dataspace
//! identifiers that the Nexus scheduler expects, based on the runtime
//! configuration. The router abstraction keeps the queue decoupled from the
//! exact routing policy while allowing metrics to reflect the real
//! assignments instead of single-lane placeholders.

use std::sync::Arc;

use iroha_config::parameters::actual::{LaneRoutingMatcher, LaneRoutingPolicy, LaneRoutingRule};
use iroha_data_model::{
    account::{AccountAlias, AccountId},
    asset::AssetDefinitionId,
    domain::DomainId,
    isi::{
        BurnBox, GrantBox, Instruction, MintBox, RegisterBox, RemoveKeyValueBox, RevokeBox,
        SetKeyValueBox, TransferBox, UnregisterBox,
        smart_contract_code::{RegisterSmartContractBytes, RegisterSmartContractCode},
    },
    nexus::{DataSpaceCatalog, DataSpaceId, LaneCatalog, LaneId},
    permission::Permission,
    transaction::Executable,
};
use iroha_executor_data_model::permission::{
    asset::{
        CanBurnAssetWithDefinition, CanMintAssetWithDefinition,
        CanModifyAssetMetadataWithDefinition, CanTransferAssetWithDefinition,
    },
    asset_definition::{CanModifyAssetDefinitionMetadata, CanUnregisterAssetDefinition},
    nexus::CanPublishSpaceDirectoryManifest,
};

use crate::{
    state::{State, StateReadOnly, StateView, WorldReadOnly},
    tx::AcceptedTransaction,
};
use thiserror::Error;

/// Routing decision returned by a [`LaneRouter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    if let Some(account_id) = account_permission_holder_routing_target(tx) {
        return evaluate_query_policy_with_view(policy, account_id, Some(state_view));
    }
    let target_dataspace = transaction_dataspace_routing_target(
        tx,
        Some(&state_view.nexus().dataspace_catalog),
        Some(state_view),
    )
    .unwrap_or(None);
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches(rule, tx, Some(state_view)));
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
    if let Some(account_id) = account_permission_holder_routing_target(tx) {
        return resolve_query_routing_decision(
            policy,
            lane_catalog,
            dataspace_catalog,
            account_id,
            None,
        );
    }
    let target_dataspace = transaction_dataspace_routing_target(tx, Some(dataspace_catalog), None)?;
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches(rule, tx, None));
    resolve_policy_routing_decision(
        policy,
        matched_rule,
        target_dataspace,
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
    if let Some(decision) = dataspace_scoped_permission_routing_decision_with_world(
        tx,
        Some(lane_catalog),
        Some(dataspace_catalog),
        world,
    )? {
        return Ok(decision);
    }
    let target_dataspace =
        transaction_dataspace_routing_target_with_world(tx, Some(dataspace_catalog), world)?;
    let matched_rule = policy
        .rules
        .iter()
        .find(|rule| rule_matches(rule, tx, None));
    resolve_policy_routing_decision(
        policy,
        matched_rule,
        target_dataspace,
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
    let Some(executable) = transaction_executable(tx) else {
        return Ok(None);
    };

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                let Some(dataspace_id) = instruction_dataspace_scoped_permission_target(
                    &**instruction,
                    dataspace_catalog,
                    state_view,
                ) else {
                    continue;
                };
                if let Some(existing) = target_dataspace {
                    if existing != dataspace_id {
                        return Err(RoutingResolveError::ConflictingDataspaceScopedPermissions {
                            first_dataspace_id: existing,
                            second_dataspace_id: dataspace_id,
                        });
                    }
                } else {
                    target_dataspace = Some(dataspace_id);
                }
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                let Some(dataspace_id) = instruction_dataspace_scoped_permission_target(
                    &**instruction,
                    dataspace_catalog,
                    state_view,
                ) else {
                    continue;
                };
                if let Some(existing) = target_dataspace {
                    if existing != dataspace_id {
                        return Err(RoutingResolveError::ConflictingDataspaceScopedPermissions {
                            first_dataspace_id: existing,
                            second_dataspace_id: dataspace_id,
                        });
                    }
                } else {
                    target_dataspace = Some(dataspace_id);
                }
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
) -> Result<Option<RoutingDecision>, RoutingResolveError> {
    let mut target_dataspace: Option<DataSpaceId> = None;
    let Some(executable) = transaction_executable(tx) else {
        return Ok(None);
    };

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                let Some(dataspace_id) = instruction_dataspace_scoped_permission_target_with_world(
                    &**instruction,
                    dataspace_catalog,
                    world,
                ) else {
                    continue;
                };
                if let Some(existing) = target_dataspace {
                    if existing != dataspace_id {
                        return Err(RoutingResolveError::ConflictingDataspaceScopedPermissions {
                            first_dataspace_id: existing,
                            second_dataspace_id: dataspace_id,
                        });
                    }
                } else {
                    target_dataspace = Some(dataspace_id);
                }
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                let Some(dataspace_id) = instruction_dataspace_scoped_permission_target_with_world(
                    &**instruction,
                    dataspace_catalog,
                    world,
                ) else {
                    continue;
                };
                if let Some(existing) = target_dataspace {
                    if existing != dataspace_id {
                        return Err(RoutingResolveError::ConflictingDataspaceScopedPermissions {
                            first_dataspace_id: existing,
                            second_dataspace_id: dataspace_id,
                        });
                    }
                } else {
                    target_dataspace = Some(dataspace_id);
                }
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

fn transaction_executable<'tx>(tx: &'tx AcceptedTransaction<'tx>) -> Option<&'tx Executable> {
    match tx.entrypoint() {
        iroha_data_model::transaction::TransactionEntrypoint::External(signed) => {
            Some(signed.instructions())
        }
        iroha_data_model::transaction::TransactionEntrypoint::PrivateKaigi(_) => None,
        iroha_data_model::transaction::TransactionEntrypoint::Time(_) => None,
    }
}

fn merge_transaction_target_dataspace(
    target_dataspace: &mut Option<DataSpaceId>,
    candidate: Option<DataSpaceId>,
) -> Result<(), RoutingResolveError> {
    let Some(candidate) = candidate else {
        return Ok(());
    };

    if let Some(existing) = target_dataspace {
        if *existing != candidate {
            return Err(
                RoutingResolveError::ConflictingTransactionDataspaceTargets {
                    first_dataspace_id: *existing,
                    second_dataspace_id: candidate,
                },
            );
        }
    } else {
        *target_dataspace = Some(candidate);
    }

    Ok(())
}

fn transaction_dataspace_routing_target(
    tx: &AcceptedTransaction<'_>,
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
                merge_transaction_target_dataspace(
                    &mut target_dataspace,
                    instruction_transaction_dataspace_target(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    ),
                )?;
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                merge_transaction_target_dataspace(
                    &mut target_dataspace,
                    instruction_transaction_dataspace_target(
                        &**instruction,
                        dataspace_catalog,
                        state_view,
                    ),
                )?;
            }
        }
    }

    Ok(target_dataspace)
}

fn transaction_dataspace_routing_target_with_world<W: WorldReadOnly>(
    tx: &AcceptedTransaction<'_>,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
) -> Result<Option<DataSpaceId>, RoutingResolveError> {
    let Some(executable) = transaction_executable(tx) else {
        return Ok(None);
    };
    let mut target_dataspace = None;

    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                merge_transaction_target_dataspace(
                    &mut target_dataspace,
                    instruction_transaction_dataspace_target_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                    ),
                )?;
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for instruction in &proved.overlay {
                merge_transaction_target_dataspace(
                    &mut target_dataspace,
                    instruction_transaction_dataspace_target_with_world(
                        &**instruction,
                        dataspace_catalog,
                        world,
                    ),
                )?;
            }
        }
    }

    Ok(target_dataspace)
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
                if dataspace_scoped_permission_target_needs_state(&grant.object)
                    || dataspace_scoped_permission_target(&grant.object, None, None).is_some()
                {
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
                if dataspace_scoped_permission_target_needs_state(&revoke.object)
                    || dataspace_scoped_permission_target(&revoke.object, None, None).is_some()
                {
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
            RegisterBox::Domain(register) => {
                domain_dataspace_target(&register.object.id, dataspace_catalog)
            }
            RegisterBox::Account(register) => {
                register.object.label.as_ref().map(|alias| alias.dataspace)
            }
            RegisterBox::AssetDefinition(register) => asset_definition_dataspace_target(
                &register.object.id,
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
            UnregisterBox::Domain(unregister) => {
                domain_dataspace_target(&unregister.object, dataspace_catalog)
            }
            UnregisterBox::AssetDefinition(unregister) => {
                asset_definition_dataspace_target(&unregister.object, dataspace_catalog, state_view)
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
            SetKeyValueBox::Domain(set) => domain_dataspace_target(&set.object, dataspace_catalog),
            SetKeyValueBox::Account(set) => {
                account_dataspace_target(state_view.map(StateView::world), &set.object)
            }
            SetKeyValueBox::AssetDefinition(set) => {
                asset_definition_dataspace_target(&set.object, dataspace_catalog, state_view)
            }
            SetKeyValueBox::Nft(_) | SetKeyValueBox::Trigger(_) => None,
        };
    }

    if let Some(remove_key_value) = any.downcast_ref::<RemoveKeyValueBox>() {
        return match remove_key_value {
            RemoveKeyValueBox::Domain(remove) => {
                domain_dataspace_target(&remove.object, dataspace_catalog)
            }
            RemoveKeyValueBox::Account(remove) => {
                account_dataspace_target(state_view.map(StateView::world), &remove.object)
            }
            RemoveKeyValueBox::AssetDefinition(remove) => {
                asset_definition_dataspace_target(&remove.object, dataspace_catalog, state_view)
            }
            RemoveKeyValueBox::Nft(_) | RemoveKeyValueBox::Trigger(_) => None,
        };
    }

    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        return match transfer {
            TransferBox::Domain(transfer) => {
                domain_dataspace_target(&transfer.object, dataspace_catalog)
            }
            TransferBox::AssetDefinition(transfer) => {
                asset_definition_dataspace_target(&transfer.object, dataspace_catalog, state_view)
            }
            // TODO: Route asset transfers from an explicit route-intent extractor once
            // transparent cross-dataspace transfers have a dedicated multi-scope policy.
            TransferBox::Asset(_) | TransferBox::Nft(_) => None,
        };
    }

    if let Some(mint) = any.downcast_ref::<MintBox>() {
        return match mint {
            MintBox::Asset(mint) => asset_definition_dataspace_target(
                &mint.destination.definition,
                dataspace_catalog,
                state_view,
            ),
            MintBox::TriggerRepetitions(_) => None,
        };
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>() {
        return match burn {
            BurnBox::Asset(burn) => asset_definition_dataspace_target(
                &burn.destination.definition,
                dataspace_catalog,
                state_view,
            ),
            BurnBox::TriggerRepetitions(_) => None,
        };
    }

    None
}

fn instruction_transaction_dataspace_target_with_world<W: WorldReadOnly>(
    instruction: &dyn Instruction,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
) -> Option<DataSpaceId> {
    let any = instruction.as_any();

    if let Some(register) = any.downcast_ref::<RegisterBox>() {
        return match register {
            RegisterBox::Domain(register) => {
                domain_dataspace_target(&register.object.id, dataspace_catalog)
            }
            RegisterBox::Account(register) => {
                register.object.label.as_ref().map(|alias| alias.dataspace)
            }
            RegisterBox::AssetDefinition(register) => asset_definition_dataspace_target_with_world(
                &register.object.id,
                dataspace_catalog,
                world,
            ),
            RegisterBox::Peer(_)
            | RegisterBox::Nft(_)
            | RegisterBox::Role(_)
            | RegisterBox::Trigger(_) => None,
        };
    }

    if let Some(unregister) = any.downcast_ref::<UnregisterBox>() {
        return match unregister {
            UnregisterBox::Domain(unregister) => {
                domain_dataspace_target(&unregister.object, dataspace_catalog)
            }
            UnregisterBox::AssetDefinition(unregister) => {
                asset_definition_dataspace_target_with_world(
                    &unregister.object,
                    dataspace_catalog,
                    world,
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
            SetKeyValueBox::Domain(set) => domain_dataspace_target(&set.object, dataspace_catalog),
            SetKeyValueBox::Account(set) => account_dataspace_target(Some(world), &set.object),
            SetKeyValueBox::AssetDefinition(set) => {
                asset_definition_dataspace_target_with_world(&set.object, dataspace_catalog, world)
            }
            SetKeyValueBox::Nft(_) | SetKeyValueBox::Trigger(_) => None,
        };
    }

    if let Some(remove_key_value) = any.downcast_ref::<RemoveKeyValueBox>() {
        return match remove_key_value {
            RemoveKeyValueBox::Domain(remove) => {
                domain_dataspace_target(&remove.object, dataspace_catalog)
            }
            RemoveKeyValueBox::Account(remove) => {
                account_dataspace_target(Some(world), &remove.object)
            }
            RemoveKeyValueBox::AssetDefinition(remove) => {
                asset_definition_dataspace_target_with_world(
                    &remove.object,
                    dataspace_catalog,
                    world,
                )
            }
            RemoveKeyValueBox::Nft(_) | RemoveKeyValueBox::Trigger(_) => None,
        };
    }

    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        return match transfer {
            TransferBox::Domain(transfer) => {
                domain_dataspace_target(&transfer.object, dataspace_catalog)
            }
            TransferBox::AssetDefinition(transfer) => asset_definition_dataspace_target_with_world(
                &transfer.object,
                dataspace_catalog,
                world,
            ),
            // TODO: Route asset transfers from an explicit route-intent extractor once
            // transparent cross-dataspace transfers have a dedicated multi-scope policy.
            TransferBox::Asset(_) | TransferBox::Nft(_) => None,
        };
    }

    if let Some(mint) = any.downcast_ref::<MintBox>() {
        return match mint {
            MintBox::Asset(mint) => asset_definition_dataspace_target_with_world(
                &mint.destination.definition,
                dataspace_catalog,
                world,
            ),
            MintBox::TriggerRepetitions(_) => None,
        };
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>() {
        return match burn {
            BurnBox::Asset(burn) => asset_definition_dataspace_target_with_world(
                &burn.destination.definition,
                dataspace_catalog,
                world,
            ),
            BurnBox::TriggerRepetitions(_) => None,
        };
    }

    None
}

fn account_dataspace_target<W: WorldReadOnly>(
    world: Option<&W>,
    account_id: &AccountId,
) -> Option<DataSpaceId> {
    let hierarchy = world?.account_scope_hierarchy(account_id).ok()?;
    (hierarchy.len() == 1).then(|| *hierarchy.keys().next().expect("single dataspace"))
}

fn domain_dataspace_target(
    domain_id: &DomainId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
) -> Option<DataSpaceId> {
    if domain_id
        .dataspace()
        .as_ref()
        .eq_ignore_ascii_case("universal")
    {
        return Some(DataSpaceId::UNIVERSAL);
    }
    dataspace_catalog?
        .by_alias(domain_id.dataspace().as_ref())
        .map(|entry| entry.id)
}

fn instruction_transaction_dataspace_target_needs_state(instruction: &dyn Instruction) -> bool {
    let any = instruction.as_any();

    if let Some(unregister) = any.downcast_ref::<UnregisterBox>() {
        return matches!(
            unregister,
            UnregisterBox::AssetDefinition(unregister)
                if unregister.object.is_opaque_canonical()
        );
    }

    if let Some(set_key_value) = any.downcast_ref::<SetKeyValueBox>() {
        return matches!(set_key_value, SetKeyValueBox::Account(_))
            || matches!(
                set_key_value,
                SetKeyValueBox::AssetDefinition(set)
                    if set.object.is_opaque_canonical()
            );
    }

    if let Some(remove_key_value) = any.downcast_ref::<RemoveKeyValueBox>() {
        return matches!(remove_key_value, RemoveKeyValueBox::Account(_))
            || matches!(
                remove_key_value,
                RemoveKeyValueBox::AssetDefinition(remove)
                    if remove.object.is_opaque_canonical()
            );
    }

    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        return matches!(
            transfer,
            TransferBox::AssetDefinition(transfer)
                if transfer.object.is_opaque_canonical()
        );
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
) -> Option<DataSpaceId> {
    let any = instruction.as_any();

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        return match grant {
            GrantBox::Permission(grant) => dataspace_scoped_permission_target_with_world(
                &grant.object,
                dataspace_catalog,
                world,
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
            ),
            RevokeBox::Role(_) | RevokeBox::RolePermission(_) => None,
        };
    }

    None
}

fn asset_definition_dataspace_target(
    asset_definition_id: &AssetDefinitionId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    state_view: Option<&StateView<'_>>,
) -> Option<DataSpaceId> {
    let dataspace_alias = asset_definition_id
        .try_domain()
        .map(|domain| domain.dataspace().as_ref().to_owned())
        .or_else(|| {
            state_view.and_then(|view| {
                view.world
                    .asset_definition(asset_definition_id)
                    .ok()
                    .and_then(|definition| {
                        definition
                            .id
                            .try_domain()
                            .map(|domain| domain.dataspace().as_ref().to_owned())
                    })
            })
        })?;
    if dataspace_alias.eq_ignore_ascii_case("universal") {
        return Some(DataSpaceId::UNIVERSAL);
    }
    dataspace_catalog?
        .by_alias(&dataspace_alias)
        .map(|entry| entry.id)
}

fn asset_definition_dataspace_target_with_world<W: WorldReadOnly>(
    asset_definition_id: &AssetDefinitionId,
    dataspace_catalog: Option<&DataSpaceCatalog>,
    world: &W,
) -> Option<DataSpaceId> {
    let dataspace_alias = asset_definition_id
        .try_domain()
        .map(|domain| domain.dataspace().as_ref().to_owned())
        .or_else(|| {
            world
                .asset_definition(asset_definition_id)
                .ok()
                .and_then(|definition| {
                    definition
                        .id
                        .try_domain()
                        .map(|domain| domain.dataspace().as_ref().to_owned())
                })
        })?;
    if dataspace_alias.eq_ignore_ascii_case("universal") {
        return Some(DataSpaceId::UNIVERSAL);
    }
    dataspace_catalog?
        .by_alias(&dataspace_alias)
        .map(|entry| entry.id)
}

fn dataspace_scoped_permission_target_needs_state(permission: &Permission) -> bool {
    match permission.name() {
        "CanMintAssetWithDefinition" => permission
            .payload()
            .try_into_any_norito::<CanMintAssetWithDefinition>()
            .ok()
            .is_some_and(|token| token.asset_definition.is_opaque_canonical()),
        "CanBurnAssetWithDefinition" => permission
            .payload()
            .try_into_any_norito::<CanBurnAssetWithDefinition>()
            .ok()
            .is_some_and(|token| token.asset_definition.is_opaque_canonical()),
        "CanTransferAssetWithDefinition" => permission
            .payload()
            .try_into_any_norito::<CanTransferAssetWithDefinition>()
            .ok()
            .is_some_and(|token| token.asset_definition.is_opaque_canonical()),
        "CanModifyAssetMetadataWithDefinition" => permission
            .payload()
            .try_into_any_norito::<CanModifyAssetMetadataWithDefinition>()
            .ok()
            .is_some_and(|token| token.asset_definition.is_opaque_canonical()),
        "CanUnregisterAssetDefinition" => permission
            .payload()
            .try_into_any_norito::<CanUnregisterAssetDefinition>()
            .ok()
            .is_some_and(|token| token.asset_definition.is_opaque_canonical()),
        "CanModifyAssetDefinitionMetadata" => permission
            .payload()
            .try_into_any_norito::<CanModifyAssetDefinitionMetadata>()
            .ok()
            .is_some_and(|token| token.asset_definition.is_opaque_canonical()),
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
                        dataspace_catalog,
                        world,
                    )
                }),
            "CanBurnAssetWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanBurnAssetWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        dataspace_catalog,
                        world,
                    )
                }),
            "CanTransferAssetWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanTransferAssetWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        dataspace_catalog,
                        world,
                    )
                }),
            "CanModifyAssetMetadataWithDefinition" => permission
                .payload()
                .try_into_any_norito::<CanModifyAssetMetadataWithDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        dataspace_catalog,
                        world,
                    )
                }),
            "CanUnregisterAssetDefinition" => permission
                .payload()
                .try_into_any_norito::<CanUnregisterAssetDefinition>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        dataspace_catalog,
                        world,
                    )
                }),
            "CanModifyAssetDefinitionMetadata" => permission
                .payload()
                .try_into_any_norito::<CanModifyAssetDefinitionMetadata>()
                .ok()
                .and_then(|token| {
                    asset_definition_dataspace_target_with_world(
                        &token.asset_definition,
                        dataspace_catalog,
                        world,
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
        .ok_or(RoutingResolveError::NoLaneForDataspace { dataspace_id })?;

    resolve_routing_decision(
        RoutingDecision::new(lane_id, dataspace_id),
        lane_catalog,
        dataspace_catalog,
    )
}

fn resolve_policy_routing_decision(
    policy: &LaneRoutingPolicy,
    matched_rule: Option<&LaneRoutingRule>,
    target_dataspace: Option<DataSpaceId>,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
) -> Result<RoutingDecision, RoutingResolveError> {
    if let Some(rule) = matched_rule {
        let decision = RoutingDecision::new(
            rule.lane,
            rule.dataspace
                .or(target_dataspace)
                .unwrap_or(policy.default_dataspace),
        );
        return resolve_routing_decision(decision, lane_catalog, dataspace_catalog);
    }

    if let Some(dataspace_id) = target_dataspace {
        return canonical_dataspace_route(dataspace_id, lane_catalog, dataspace_catalog);
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
    let decision = evaluate_query_policy_with_view(policy, authority, state_view);
    resolve_routing_decision(decision, lane_catalog, dataspace_catalog)
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
    if !dataspace_known {
        return Err(RoutingResolveError::UnknownDataspace {
            dataspace_id: decision.dataspace_id,
        });
    }

    if lane.dataspace_id != decision.dataspace_id {
        return Err(RoutingResolveError::LaneDataspaceMismatch {
            lane_id: lane.id,
            lane_dataspace_id: lane.dataspace_id,
            dataspace_id: decision.dataspace_id,
        });
    }

    Ok(decision)
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

fn account_matches(
    pattern: &str,
    authority: &iroha_data_model::account::AccountId,
    state_view: Option<&StateView<'_>>,
) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }

    if authority.to_string() == pattern {
        return true;
    }
    if iroha_data_model::account::AccountId::parse_encoded(pattern)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .is_ok_and(|parsed| parsed == *authority)
    {
        return true;
    }

    let Some(state_view) = state_view else {
        return false;
    };

    if let Some(scope) = pattern.strip_prefix("*@") {
        return account_matches_alias_scope(scope, authority, state_view);
    }

    AccountAlias::from_literal(pattern, &state_view.nexus().dataspace_catalog)
        .ok()
        .is_some_and(|alias| {
            state_view
                .world()
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
    let scope = scope.trim().to_ascii_lowercase();
    if scope.is_empty() {
        return false;
    }

    if state_view
        .world()
        .account_scope_hierarchy(account_id)
        .ok()
        .is_some_and(|hierarchy| {
            hierarchy.into_iter().any(|(dataspace_id, domains)| {
                state_view
                    .nexus()
                    .dataspace_catalog
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

    state_view
        .world()
        .bound_account_aliases(account_id)
        .into_iter()
        .any(|alias| {
            alias
                .to_literal(&state_view.nexus().dataspace_catalog)
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
        evaluate_policy_with_view(&self.policy, tx, state_view)
    }

    fn route_without_state(&self, tx: &AcceptedTransaction<'_>) -> Option<RoutingDecision> {
        if policy_needs_state(self.policy.as_ref())
            || dataspace_scoped_permission_routing_requires_state(tx)
            || transaction_target_routing_requires_state(tx)
        {
            return None;
        }
        Some(self.route(tx))
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
        if let Some(account_id) = account_permission_holder_routing_target(tx) {
            return resolve_query_routing_decision(
                &self.policy,
                self.lane_catalog.as_ref(),
                self.dataspace_catalog.as_ref(),
                account_id,
                None,
            );
        }
        let target_dataspace =
            transaction_dataspace_routing_target(tx, Some(self.dataspace_catalog.as_ref()), None)?;
        let matched_rule = self
            .policy
            .rules
            .iter()
            .find(|rule| rule_matches(rule, tx, None));
        resolve_policy_routing_decision(
            &self.policy,
            matched_rule,
            target_dataspace,
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
        if let Some(account_id) = account_permission_holder_routing_target(tx) {
            return resolve_query_routing_decision(
                &self.policy,
                &nexus.lane_catalog,
                &nexus.dataspace_catalog,
                account_id,
                Some(state_view),
            );
        }
        let target_dataspace = transaction_dataspace_routing_target(
            tx,
            Some(&nexus.dataspace_catalog),
            Some(state_view),
        )?;
        let matched_rule = self
            .policy
            .rules
            .iter()
            .find(|rule| rule_matches(rule, tx, Some(state_view)));
        resolve_policy_routing_decision(
            &self.policy,
            matched_rule,
            target_dataspace,
            &nexus.lane_catalog,
            &nexus.dataspace_catalog,
        )
    }

    fn try_route_without_state(
        &self,
        tx: &AcceptedTransaction<'_>,
    ) -> Result<Option<RoutingDecision>, RoutingResolveError> {
        if policy_needs_state(self.policy.as_ref())
            || dataspace_scoped_permission_routing_requires_state(tx)
            || transaction_target_routing_requires_state(tx)
        {
            return Ok(None);
        }
        self.try_route(tx).map(Some)
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
    use iroha_crypto::Hash;
    use iroha_data_model::{
        IntoKeyValue,
        account::AccountAliasDomain,
        isi::{
            prelude::{Mint, Register, Transfer},
            smart_contract_code::RegisterSmartContractBytes,
        },
        metadata::Metadata,
        nexus::LaneConfig,
        permission::Permission,
        prelude::*,
        transaction::TransactionBuilder,
    };
    use iroha_executor_data_model::permission::{
        account::{AccountAliasPermissionScope, CanManageAccountAlias},
        nexus::CanPublishSpaceDirectoryManifest,
    };
    use iroha_test_samples::gen_account_in;
    use nonzero_ext::nonzero;

    use super::*;

    fn sample_transaction(
        authority: &AccountId,
        signer: &iroha_crypto::PrivateKey,
        instructions: Vec<InstructionBox>,
    ) -> AcceptedTransaction<'static> {
        let chain_id = ChainId::from("chain");
        let tx = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_instructions(instructions)
            .with_metadata(Metadata::default())
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
                            "sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53"
                                .into(),
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
        let with_view = router.route_with_view(&tx, &state.view());
        let without_view = router.route_without_state(&tx);
        assert_eq!(without_view, Some(with_view));
    }

    #[test]
    fn rule_dataspace_override_is_used() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(5),
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

        let policy_for_helper = policy.clone();
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (LaneId::new(5), DataSpaceId::new(7)),
        ]);
        let router = ConfigLaneRouter::new(policy, catalog, lane_catalog);

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("override", "universal").expect("domain"),
            )))],
        );
        let state = blank_state();
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(5));
        assert_eq!(decision.dataspace_id, DataSpaceId::new(7));

        let helper_decision = evaluate_policy(&policy_for_helper, &tx);
        assert_eq!(helper_decision, decision);
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

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("override", "universal").expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
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

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("fallback", "universal").expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
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

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("fallback", "universal").expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
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

        let decision = router.route_with_view(&tx, &blank_state().view());
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

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::new(1));
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

        let decision = router.route_with_view(&tx, &blank_state().view());
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

        let decision = router.route_with_view(&tx, &blank_state().view());
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
    fn rejects_mixed_domain_write_targets_across_dataspaces() {
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
        let uae_decision = router.route_with_view(&uae_tx, &state.view());
        let bank_decision = router.route_with_view(&bank_tx, &state.view());
        assert_eq!(uae_decision.lane_id, LaneId::new(2));
        assert_eq!(bank_decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn matches_dataspace_root_account_alias_scope_rule() {
        let (dataspace_id, dataspace_keypair) = gen_account_in("wonderland");
        let (domain_id, domain_keypair) = gen_account_in("wonderland");
        let catalog = dataspace_catalog(&[(DataSpaceId::new(10), "sbp")]);

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(DataSpaceId::new(10)),
                matcher: LaneRoutingMatcher {
                    account: Some("*@sbp".to_string()),
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
                DomainId::try_new("sbp-match", "universal").expect("domain id"),
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
                (dataspace_id.clone(), account_alias("issuer@sbp", &catalog)),
                (
                    domain_id.clone(),
                    account_alias("operator@banka.sbp", &catalog),
                ),
            ],
            catalog,
        );

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
        let catalog = dataspace_catalog(&[(dataspace_id, "sbp")]);
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
                    account: Some("*@sbp".to_string()),
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
                account_alias("operator@sbp", &catalog),
            )],
            catalog,
        );
        state.nexus.write().lane_catalog = state_lane_catalog;

        let decision = router
            .try_route_with_view(&tx, &state.view())
            .expect("state-aware routing must resolve against the same state catalogs it matched");

        assert_eq!(decision, RoutingDecision::new(lane_id, dataspace_id));
    }

    #[test]
    fn legacy_bare_domain_account_scope_does_not_match() {
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let catalog = dataspace_catalog(&[(DataSpaceId::new(10), "sbp")]);

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
                account_alias("operator@banka.sbp", &catalog),
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
    fn dataspace_scoped_permission_grant_rejects_mixed_dataspaces() {
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

        let err = router
            .try_route(&tx)
            .expect_err("mixed dataspace-scoped permissions must be rejected");

        assert!(matches!(
            err,
            RoutingResolveError::ConflictingDataspaceScopedPermissions { .. }
        ));
        assert_eq!(err.as_label(), "conflicting_dataspace_scoped_permissions");
    }

    #[test]
    fn account_scope_directory_scope_matches_destination_account_permission_route() {
        let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
        let (holder_id, _) = gen_account_in("wonderland");
        let dataspace_id = DataSpaceId::new(10);
        let catalog = dataspace_catalog(&[(dataspace_id, "sbp")]);

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::UNIVERSAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: Some(dataspace_id),
                matcher: LaneRoutingMatcher {
                    account: Some("*@hbl.sbp".to_string()),
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
                DomainId::try_new("hbl", "sbp").expect("domain id"),
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
        let state_view = state.view();
        assert_eq!(
            state_view
                .world()
                .account_scope_hierarchy(&holder_id)
                .expect("scope hierarchy"),
            BTreeMap::from([(
                dataspace_id,
                BTreeSet::from([DomainId::try_new("hbl", "sbp").expect("domain id")]),
            )])
        );
        assert!(account_matches_alias_scope(
            "hbl.sbp",
            &holder_id,
            &state_view
        ));

        assert_eq!(
            router.route_with_view(&tx, &state_view),
            RoutingDecision::new(LaneId::new(1), dataspace_id)
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
}
