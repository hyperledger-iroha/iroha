//! Owner-signed retirement of a catalogued physical dataspace from parent execution.
//!
//! Retirement removes effective execution authority and nothing else. The configured and
//! committed physical catalogs keep the dataspace descriptor and every lane descriptor,
//! incarnation and storage binding; native lane stores and block history are untouched, so
//! startup, snapshot and Kura geometry checks see the same catalog before and after. The
//! protected record [`DataSpaceRetirementsV1`] is the only new state: an ordinary World custom
//! parameter written solely by [`StateTransaction::retire_dataspace`].
//!
//! The record has three effects, all inert until it exists:
//!
//! - parent execution of the dataspace is refused: native routing gives its work no lane, Global
//!   instruction execution refuses its scoped (non-control-plane) instructions, and no native
//!   policy, catalog transition or lane lifecycle may give it, or any of its retired physical lane
//!   identities, execution again;
//! - `RegisterPrivateDataspace` may claim the retired identity, but only for the retiring SNS owner
//!   and ownership generation, in a block after the retirement;
//! - parent-side control-plane work keeps working: SNS renewals, aliases, fees, private-root
//!   registration and anchors route to lane `0` as universal control-plane instructions.
//!
//! Native Sumeragi lane closure is required first. Governance drops the dataspace's fixed lanes
//! (and any route to them) from `sumeragi_lane_policy`; each lane closes at `c` and its record is
//! removed at `c + A + 1`. Retirement refuses while any native record, fixed lane, route, elastic
//! range or autoscale binding for the dataspace or its lanes remains, so no lane work is pending.
//!
//! TODO: parent alias-lifecycle work scoped to a retired dataspace (`ConfigureAliasAutoRenew`,
//! `RebindAccountAlias`, `CompareAndSetPrimaryAccountAlias`) has no route once its native lanes
//! close; only `EnsureAlias` and `RenewAliasLease` stay universal. Route those as control-plane
//! work if owners need them after retirement.

use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    block::consensus::SumeragiRootScope,
    isi::{
        InstructionBox,
        error::{InstructionExecutionError, InvalidParameterError},
    },
    nexus::{
        DataSpaceRetirementRequestV1, DataSpaceRetirementsV1, LaneLifecyclePlan, LaneVisibility,
        RetiredDataSpaceV1,
    },
    parameter::{CustomParameter, Parameter, Parameters},
    sumeragi_lanes::SumeragiLanePolicy,
};
use iroha_model_base::topology::{DataSpaceId, LaneId};
use mv::storage::StorageReadOnly;

use super::{StateTransaction, WorldReadOnly};
use crate::execution_attempt::{ExecutionAttemptError, ExecutionDeferred};

/// Borrow the committed record by its static name, without constructing a typed identifier.
///
/// Like the other consensus-owned identity checks, this never allocates, so the inert path adds
/// no native allocation request to routing or instruction execution.
fn record_parameter(parameters: &Parameters) -> Option<&CustomParameter> {
    parameters.custom().values().find(|parameter| {
        parameter.id().name().as_ref() == DataSpaceRetirementsV1::PARAMETER_ID_STR
    })
}

/// Decode the committed retirement record; `Ok(None)` before the first retirement.
///
/// # Errors
/// A malformed record is a completed rejection; an unfinished local decode is deferred.
pub(crate) fn retirements(
    parameters: &Parameters,
) -> Result<Option<DataSpaceRetirementsV1>, ExecutionAttemptError<String>> {
    let Some(custom) = record_parameter(parameters) else {
        return Ok(None);
    };
    match super::nexus_catalog_validation_attempt(|| {
        DataSpaceRetirementsV1::from_custom_parameter(custom)
    }) {
        Ok(Some(record)) => Ok(Some(record)),
        Ok(None) => Err(ExecutionAttemptError::Rejected(
            "dataspace retirement record has another parameter identity".into(),
        )),
        Err(error) => {
            Err(error
                .map_rejection(|error| format!("malformed dataspace retirement record: {error}")))
        }
    }
}

/// Whether any retirement was committed. This is the allocation-free inert fast path.
pub(crate) fn has_retirements(world: &impl WorldReadOnly) -> bool {
    record_parameter(world.parameters()).is_some()
}

/// The retirement of `dataspace`, if any.
///
/// # Errors
/// See [`retirements`].
pub(crate) fn retired_dataspace(
    world: &impl WorldReadOnly,
    dataspace: DataSpaceId,
) -> Result<Option<RetiredDataSpaceV1>, ExecutionAttemptError<String>> {
    if !has_retirements(world) {
        return Ok(None);
    }
    Ok(retirements(world.parameters())?.and_then(|record| record.get(dataspace).cloned()))
}

/// Flatten a retirement read for refusal-only guards whose callers carry a `String` error.
fn completed(error: ExecutionAttemptError<String>) -> String {
    match error {
        ExecutionAttemptError::Rejected(reason) => reason,
        ExecutionAttemptError::Deferred(_) => {
            "local dataspace retirement record read did not complete".into()
        }
    }
}

/// Whether native routing must refuse work targeting `dataspace`.
///
/// A malformed record fails closed for dataspace-scoped work.
///
/// # Errors
/// Returns the original local deferral of the record read.
pub(crate) fn routing_retired(
    world: &impl WorldReadOnly,
    dataspace: DataSpaceId,
) -> Result<bool, ExecutionDeferred> {
    if !has_retirements(world) {
        return Ok(false);
    }
    match retirements(world.parameters()) {
        Ok(record) => Ok(record.is_some_and(|record| record.is_retired(dataspace))),
        Err(ExecutionAttemptError::Rejected(_)) => Ok(true),
        Err(ExecutionAttemptError::Deferred(reason)) => Err(reason),
    }
}

/// Refuse parent execution claims on retired dataspaces.
///
/// # Errors
/// Returns the refusal for the first retired dataspace in `dataspaces`.
pub(crate) fn ensure_not_retired(
    world: &impl WorldReadOnly,
    dataspaces: impl IntoIterator<Item = DataSpaceId>,
) -> Result<(), String> {
    if !has_retirements(world) {
        return Ok(());
    }
    let record = retirements(world.parameters()).map_err(completed)?;
    if let Some(dataspace) = dataspaces.into_iter().find(|dataspace| {
        record
            .as_ref()
            .is_some_and(|record| record.is_retired(*dataspace))
    }) {
        return Err(format!(
            "physical parent execution cannot claim retired dataspace {dataspace}"
        ));
    }
    Ok(())
}

/// Refuse a native lane policy that would give a retired dataspace, or any of its retired
/// physical lane identities, native fixed, routed or elastic execution again.
///
/// # Errors
/// Returns the first reuse found.
pub(crate) fn ensure_native_policy_preserves_retirements(
    world: &impl WorldReadOnly,
    policy: &SumeragiLanePolicy,
) -> Result<(), String> {
    if !has_retirements(world) {
        return Ok(());
    }
    let Some(record) = retirements(world.parameters()).map_err(completed)? else {
        return Ok(());
    };
    for retired in &record.retired {
        if policy
            .fixed
            .iter()
            .any(|fixed| fixed.dataspace == retired.dataspace_id)
            || policy
                .autoscale
                .as_ref()
                .is_some_and(|autoscale| autoscale.dataspace == retired.dataspace_id)
        {
            return Err(format!(
                "native lane policy cannot execute retired dataspace {}",
                retired.dataspace_id
            ));
        }
        if let Some(lane) = retired.lanes.iter().find(|lane| {
            policy.fixed_lane(**lane).is_some()
                || policy.is_elastic(**lane)
                || policy.routes.iter().any(|route| route.lane == **lane)
        }) {
            return Err(format!(
                "native lane policy cannot reuse lane {lane} of retired dataspace {}",
                retired.dataspace_id
            ));
        }
    }
    Ok(())
}

/// Refuse a physical lane lifecycle plan that adds lanes to, or touches lanes of, a retired
/// dataspace. Retired lane identities stay in the catalog and are never reused.
///
/// # Errors
/// Returns the first conflicting addition or retirement.
pub(crate) fn ensure_lane_lifecycle_preserves_retirements(
    world: &impl WorldReadOnly,
    plan: &LaneLifecyclePlan,
) -> Result<(), String> {
    if !has_retirements(world) {
        return Ok(());
    }
    let Some(record) = retirements(world.parameters()).map_err(completed)? else {
        return Ok(());
    };
    if let Some(addition) = plan.additions.iter().find(|addition| {
        record.is_retired(addition.dataspace_id) || record.lane_owner(addition.id).is_some()
    }) {
        return Err(format!(
            "lane lifecycle cannot add lane {} to a retired dataspace or retired lane identity",
            addition.id
        ));
    }
    if let Some(lane) = plan
        .retire
        .iter()
        .find(|lane| record.lane_owner(**lane).is_some())
    {
        return Err(format!(
            "lane lifecycle cannot replace retired physical lane {lane}"
        ));
    }
    Ok(())
}

/// Refuse a Global-root instruction scoped to a retired dataspace.
///
/// Universal control-plane instructions (SNS, aliases, parameters, private-root registration and
/// anchors) remain admitted. An instruction whose native target cannot be resolved keeps its
/// existing Global behavior; only an exact retired target is refused.
///
/// # Errors
/// Refuses retired scoped work and retains an unfinished local read as a deferral.
pub(crate) fn ensure_instruction_outside_retired_dataspaces(
    instruction: &InstructionBox,
    state: &mut StateTransaction<'_, '_>,
) -> Result<(), ValidationFail> {
    if !has_retirements(&state.world) {
        return Ok(());
    }
    let record = match retirements(state.world.parameters()) {
        Ok(Some(record)) => record,
        Ok(None) => return Ok(()),
        Err(ExecutionAttemptError::Deferred(reason)) => return Err(state.defer_execution(reason)),
        Err(ExecutionAttemptError::Rejected(reason)) => {
            return Err(ValidationFail::NotPermitted(reason));
        }
    };
    let target = match crate::queue::native_instruction_execution_target(
        &**instruction,
        &state.nexus.dataspace_catalog,
        &state.world,
        state.block_unix_timestamp_ms(),
    ) {
        Ok(target) => target,
        Err(crate::queue::RoutingResolveError::Deferred(reason)) => {
            return Err(state.defer_execution(reason));
        }
        Err(_) => return Ok(()),
    };
    match target.dataspace {
        Some(dataspace) if !target.global && record.is_retired(dataspace) => {
            Err(ValidationFail::NotPermitted(format!(
                "dataspace {dataspace} is retired from parent execution"
            )))
        }
        _ => Ok(()),
    }
}

/// Execute the reserved retirement parameters; `None` for every other custom parameter.
///
/// The request is consumed and never stored. The protected record is never caller-writable.
pub(crate) fn execute_reserved_parameter(
    custom: &CustomParameter,
    authority: &AccountId,
    state: &mut StateTransaction<'_, '_>,
) -> Option<Result<(), InstructionExecutionError>> {
    let invalid = |message: String| {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message))
    };
    let name = custom.id().name().as_ref();
    if name == DataSpaceRetirementsV1::PARAMETER_ID_STR {
        return Some(Err(invalid(
            "the committed dataspace retirement record can only be changed by its typed retirement request"
                .into(),
        )));
    }
    if name != DataSpaceRetirementRequestV1::PARAMETER_ID_STR {
        return None;
    }
    let request = match super::nexus_catalog_validation_attempt(|| {
        DataSpaceRetirementRequestV1::from_custom_parameter(custom)
    }) {
        Ok(Some(request)) => request,
        Ok(None) => return None,
        Err(error) => {
            return Some(Err(state.attempt_error_to_instruction_error(
                error.map_rejection(|error| {
                    invalid(format!("invalid dataspace retirement payload: {error}"))
                }),
            )));
        }
    };
    Some(state.retire_dataspace(authority, &request))
}

impl StateTransaction<'_, '_> {
    /// Retire one catalogued physical dataspace from parent execution.
    ///
    /// Every check precedes the single protected parameter write. The catalog, lanes, native
    /// stores and history are untouched.
    ///
    /// # Errors
    /// Refuses unless the authority is the exact active SNS owner and generation, the dataspace is
    /// a catalogued non-default physical dataspace with exactly the named lanes, its native lanes
    /// are closed and removed, its lanes are restricted, it homes no asset definition or active
    /// contract, and it is neither already retired nor a registered private root.
    pub(crate) fn retire_dataspace(
        &mut self,
        authority: &AccountId,
        request: &DataSpaceRetirementRequestV1,
    ) -> Result<(), InstructionExecutionError> {
        let record = match self.prepare_dataspace_retirement(authority, request) {
            Ok(record) => record,
            Err(error) => {
                return Err(self.attempt_error_to_instruction_error(error.map_rejection(
                    |reason| {
                        InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(format!(
                                "invalid dataspace retirement: {reason}"
                            )),
                        )
                    },
                )));
            }
        };
        let prepared = self
            .prepare_parameter_change(&Parameter::Custom(record), None, None)
            .map_err(|original| self.parameter_change_instruction_error(original))?;
        self.world
            .install_parameter_change(prepared)
            .map_err(|original| self.parameter_change_instruction_error(original))
    }

    fn prepare_dataspace_retirement(
        &self,
        authority: &AccountId,
        request: &DataSpaceRetirementRequestV1,
    ) -> Result<CustomParameter, ExecutionAttemptError<String>> {
        let reject = |reason: &str| ExecutionAttemptError::Rejected(reason.to_owned());
        request
            .validate_structure()
            .map_err(|error| ExecutionAttemptError::Rejected(error.to_string()))?;
        let dataspace = request.dataspace_id;
        let height = self.block_height();
        if height <= 1 {
            return Err(reject("retirement requires committed post-genesis history"));
        }
        if crate::sumeragi::lanes::routing::read_routing_root_scope(&self.world)
            .map_err(ExecutionAttemptError::Deferred)?
            != Some(SumeragiRootScope::Global)
        {
            return Err(reject("retirement requires the committed global root"));
        }
        if self.lane_lifecycle_already_staged_in_block || self.pending_lane_lifecycle.is_some() {
            return Err(reject(
                "a catalog or lane lifecycle transition is already staged in this block",
            ));
        }
        if authority != &request.owner {
            return Err(reject("retirement must be signed by its exact SNS owner"));
        }
        let descriptor = self
            .nexus
            .dataspace_catalog
            .by_id(dataspace)
            .ok_or_else(|| reject("retirement targets an unknown physical dataspace"))?;
        if descriptor.alias != request.alias {
            return Err(reject(
                "retirement alias differs from the physical catalog descriptor",
            ));
        }
        if dataspace == self.nexus.routing_policy.default_dataspace {
            return Err(reject("the default routing dataspace cannot be retired"));
        }
        // The re-admitted private root must claim exactly this identity; refuse a retirement
        // that could never be followed by the documented re-admission.
        let selector = crate::sns::selector_for_dataspace_alias(&request.alias)
            .map_err(|error| ExecutionAttemptError::Rejected(error.to_string()))?;
        if selector.normalized_label() != request.alias
            || DataSpaceId::from_hash(&selector.name_hash()) != dataspace
        {
            return Err(reject(
                "retirement requires the canonical alias and its SNS-derived dataspace identity",
            ));
        }
        let now = self.block_unix_timestamp_ms();
        let sns = |error: crate::sns::SnsError| error.into_attempt_error(|error| error.to_string());
        if crate::sns::resolve_active_dataspace_id_by_alias(
            &self.world,
            &self.nexus.dataspace_catalog,
            &request.alias,
            now,
        )
        .map_err(sns)?
            != dataspace
        {
            return Err(reject(
                "catalog and active SNS directories map the alias to another dataspace",
            ));
        }
        if crate::sns::active_dataspace_owner_and_generation_by_alias(
            &self.world,
            &request.alias,
            now,
        )
        .map_err(sns)?
            != Some((request.owner.clone(), request.expected_ownership_generation))
        {
            return Err(reject(
                "retirement requires the exact current SNS owner and ownership generation",
            ));
        }
        let previous = retirements(self.world.parameters())?.unwrap_or_default();
        if previous.is_retired(dataspace) {
            return Err(reject("dataspace is already retired"));
        }
        if self.world.private_dataspaces().get(dataspace).is_some() {
            return Err(reject("dataspace is already a registered private root"));
        }
        let catalog_lanes: Vec<LaneId> = self
            .nexus
            .lane_catalog
            .lanes()
            .iter()
            .filter(|lane| lane.dataspace_id == dataspace)
            .map(|lane| lane.id)
            .collect();
        if catalog_lanes != request.lanes {
            return Err(reject(
                "retirement must name exactly every physical lane of the dataspace",
            ));
        }
        // Public lanes promise parent-replicated, publicly discoverable execution; a private
        // root cannot inherit that promise, so only restricted dataspaces may leave.
        if catalog_lanes.is_empty()
            || self.nexus.lane_catalog.lanes().iter().any(|lane| {
                lane.dataspace_id == dataspace && lane.visibility != LaneVisibility::Restricted
            })
        {
            return Err(reject(
                "only a restricted physical dataspace with lanes can leave parent execution",
            ));
        }
        if request.lanes.iter().any(|lane| {
            *lane == LaneId::SINGLE
                || *lane == self.nexus.routing_policy.default_lane
                || self.touched_lanes.contains(lane)
        }) {
            return Err(reject(
                "retirement cannot claim the primary or default lane, or a lane with work in this block",
            ));
        }
        self.ensure_native_lanes_closed(dataspace, &request.lanes)?;
        self.ensure_retiring_dataspace_drained(dataspace)?;
        let next = previous
            .with_retirement(RetiredDataSpaceV1 {
                dataspace_id: dataspace,
                alias: request.alias.clone(),
                owner: request.owner.clone(),
                ownership_generation: request.expected_ownership_generation,
                lanes: request.lanes.clone(),
                retirement_height: height,
            })
            .map_err(|error| ExecutionAttemptError::Rejected(error.to_string()))?;
        super::nexus_catalog_validation_attempt(|| next.into_custom_parameter())
            .map_err(|error| error.map_rejection(|error| error.to_string()))
    }

    /// Native Sumeragi closure must already have removed every record and policy binding.
    fn ensure_native_lanes_closed(
        &self,
        dataspace: DataSpaceId,
        lanes: &[LaneId],
    ) -> Result<(), ExecutionAttemptError<String>> {
        let pending = |reason: &str| {
            ExecutionAttemptError::Rejected(format!(
                "{reason}; close the native lane through sumeragi_lane_policy and wait for its removal at c + A + 1"
            ))
        };
        if self
            .world
            .sumeragi_lanes()
            .lanes
            .iter()
            .any(|record| record.dataspace == dataspace || lanes.contains(&record.lane))
        {
            return Err(pending(
                "a native lane of the dataspace is still active or closing",
            ));
        }
        let policy = crate::sumeragi::lanes::lane_policy(&self.world)
            .map_err(ExecutionAttemptError::Deferred)?;
        if let Some(policy) = policy
            && (policy
                .fixed
                .iter()
                .any(|fixed| fixed.dataspace == dataspace || lanes.contains(&fixed.lane))
                || policy
                    .routes
                    .iter()
                    .any(|route| lanes.contains(&route.lane))
                || lanes.iter().any(|lane| policy.is_elastic(*lane))
                || policy
                    .autoscale
                    .as_ref()
                    .is_some_and(|autoscale| autoscale.dataspace == dataspace))
        {
            return Err(pending(
                "the native lane policy still binds the dataspace or one of its lanes",
            ));
        }
        Ok(())
    }

    /// Value and code that only parent execution could reach must be gone before retirement.
    fn ensure_retiring_dataspace_drained(
        &self,
        dataspace: DataSpaceId,
    ) -> Result<(), ExecutionAttemptError<String>> {
        for (asset_definition_id, definition) in self.world.asset_definitions().iter() {
            if crate::read_scope::home_dataspace(&self.world, definition)
                .ok()
                .flatten()
                == Some(dataspace)
            {
                return Err(ExecutionAttemptError::Rejected(format!(
                    "retiring dataspace still homes asset definition {asset_definition_id}"
                )));
            }
        }
        for (address, _) in self.world.contract_instances().iter() {
            if address.dataspace_id().ok() == Some(dataspace) {
                return Err(ExecutionAttemptError::Rejected(format!(
                    "retiring dataspace still has active contract instance {address}"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
