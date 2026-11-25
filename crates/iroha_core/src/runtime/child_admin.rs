//! Selected administration of an independently founded child root.
//!
//! Only original signed-genesis installation can populate the authority cell. Ordinary actions
//! read that canonical cell, the authenticated native root and a one-use direct signed source.
//! Administrator approval schedules work; unanimous validator support and lifecycle readiness
//! remain separate prerequisites for activation. Universal Parliament definitions are unchanged.

use iroha_crypto::Hash;
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    block::consensus::SumeragiRootScope,
    events::data::runtime_upgrade::{
        RuntimeUpgradeCanceled, RuntimeUpgradeEvent, RuntimeUpgradeScheduled,
    },
    isi::{
        child_runtime::{AttestChildRuntimeSupportV1, AuthorizeChildRuntimeActionV1},
        error::InstructionExecutionError as Error,
    },
    runtime::{
        child_admin::{
            CHILD_ADMIN_SERVICE_CLOSURE_V1, CHILD_ADMIN_SERVICE_V1, ChildAdminErrorV1,
            ChildRootIdentityV1, ChildRuntimeAdminActionV1, ChildRuntimeAdminCertificateV1,
            ChildRuntimeAdminStateV1,
        },
        definition::RootSelectionV1,
        lifecycle::{
            ChangeSetId, ChangeSetRecordV1, ChangeSetStatusV1, RootScopeV1,
            check_change_set_provenance,
        },
    },
};
use mv::storage::StorageReadOnly;

use super::change_set::{self, PublishedDefinitions};
use crate::{
    execution_attempt::{
        ExecutionAttemptError, canonical_decode_attempt_error, json_decode_attempt_error,
    },
    smartcontracts::Execute,
    state::{StateReadOnly, StateTransaction},
};

mod direct;
pub(crate) use direct::{DirectInstruction, capture_direct};

fn rejected(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("child runtime administration: {reason}").into())
}

fn model_error(error: ChildAdminErrorV1, state: &mut StateTransaction<'_, '_>) -> Error {
    let attempt = match error {
        ChildAdminErrorV1::Resource(original) => canonical_decode_attempt_error(original, rejected),
        ChildAdminErrorV1::CryptoUnavailable(original) => ExecutionAttemptError::Deferred(
            ivm::ExecutionDeferral::CryptoImplementationUnavailable(original).into(),
        ),
        ChildAdminErrorV1::Json(original) => json_decode_attempt_error(original, rejected),
        error => ExecutionAttemptError::Rejected(rejected(error)),
    };
    state.attempt_error_to_instruction_error(attempt)
}

/// Whether the exact new service and its own addition storage are selected together.
/// A populated but unselected cell has no authority.
pub(crate) fn enabled(selection: &RootSelectionV1) -> Result<bool, String> {
    let catalog = super::release_catalog().map_err(|error| error.to_string())?;
    let service = catalog
        .service_enabled(
            selection.services_profile,
            CHILD_ADMIN_SERVICE_V1,
            Hash::new(CHILD_ADMIN_SERVICE_CLOSURE_V1).into(),
        )
        .map_err(|error| error.to_string())?;
    Ok(service
        && selection
            .installed_storage
            .contains(&catalog.world_storage["world.runtime_child_admin_state"])
        && super::migration::lifecycle_installed(selection))
}

/// Derive all identity components from the actual immutable signed root owner.
pub(crate) fn authenticated_identity(
    state: &mut StateTransaction<'_, '_>,
) -> Result<ChildRootIdentityV1, Error> {
    let scope = crate::executor::root_scope::execution_root_scope(state).map_err(rejected)?;
    let SumeragiRootScope::Dataspace {
        parent_network_id,
        dataspace_id,
    } = scope
    else {
        return Err(rejected("the Universal root uses SORA Parliament"));
    };
    let network = *state.network_id();
    let instance = scope
        .instance_id(
            &crate::sumeragi::crypto::BlsCrypto::new(),
            network,
            state.chain_id().as_str(),
        )
        .map_err(rejected)?;
    Ok(ChildRootIdentityV1 {
        network,
        parent_network: parent_network_id,
        dataspace: dataspace_id,
        instance: instance.0,
    })
}

fn ordinary_context(state: &StateTransaction<'_, '_>) -> bool {
    !crate::executor::is_initial_genesis_context(state)
        && state.genesis_execution_scope.is_none()
        && !state.execution_callback_active()
        && state.completing_obligation.is_none()
        && state.native_execution.is_none()
        && state.deferred_expectations.is_empty()
        && state.world.runtime_selection.get() == state.world.runtime_selection.canonical()
}

fn committed_authority(
    state: &mut StateTransaction<'_, '_>,
) -> Result<ChildRuntimeAdminStateV1, Error> {
    if !ordinary_context(state) {
        return Err(rejected(
            "requires ordinary direct execution outside genesis, callbacks and retained contexts",
        ));
    }
    if !enabled(state.world.runtime_selection.canonical()).map_err(rejected)? {
        return Err(rejected(
            "the authenticated child administrator service is not selected",
        ));
    }
    let root = authenticated_identity(state)?;
    super::lifecycle_activation::validate_child_foundation(&*state.world, root.network)
        .map_err(|error| state.attempt_error_to_instruction_error(error.map_rejection(rejected)))?;
    let committed = state
        .world
        .runtime_child_admin_state
        .get()
        .clone()
        .ok_or_else(|| {
            rejected(
                "selected empty storage does not establish a genesis-founded administrator policy",
            )
        })?;
    committed
        .validate()
        .map_err(|error| model_error(error, state))?;
    if committed.root != root {
        return Err(rejected(
            "committed policy does not belong to this authenticated child instance",
        ));
    }
    Ok(committed)
}

/// Scope review of only the two new typed instruction carriers. Normal execution still consumes
/// the exact direct source and verifies the certificate or current validator signature.
pub(crate) fn review_scope(
    instruction: &iroha_data_model::isi::InstructionBox,
    state: &mut StateTransaction<'_, '_>,
) -> Option<Result<(), ValidationFail>> {
    let any = instruction.as_any();
    let expected = if let Some(action) = any.downcast_ref::<AuthorizeChildRuntimeActionV1>() {
        Some((
            action.certificate.plan.root.network,
            RootScopeV1::DataSpace(action.certificate.plan.root.dataspace),
        ))
    } else {
        any.downcast_ref::<AttestChildRuntimeSupportV1>()
            .map(|support| {
                (
                    support.attestation.attestation.network,
                    support.attestation.attestation.root,
                )
            })
    }?;
    Some((|| {
        let current = committed_authority(state)
            .map_err(|error| ValidationFail::NotPermitted(error.to_string()))?;
        if expected
            != (
                current.root.network,
                RootScopeV1::DataSpace(current.root.dataspace),
            )
        {
            return Err(ValidationFail::NotPermitted(
                "child runtime instruction names another root".into(),
            ));
        }
        Ok(())
    })())
}

// Prepared actions contain every fallible semantic result. Mutation begins only after both the
// exact signed source and all current policy, provenance, timeline and root checks have passed.
enum PreparedAction {
    Schedule(ChangeSetId, ChangeSetRecordV1),
    Cancel(ChangeSetId, ChangeSetRecordV1),
    Rotate,
}

fn prepare_action(
    certificate: &ChildRuntimeAdminCertificateV1,
    committed: &mut ChildRuntimeAdminStateV1,
    state: &mut StateTransaction<'_, '_>,
) -> Result<PreparedAction, Error> {
    let height = state._curr_block.height().get();
    certificate
        .verify_against(committed, height)
        .map_err(|error| model_error(error, state))?;
    // Certificate validation rejects exhaustion before this operation; keep checked arithmetic
    // here so the mutation plan never depends on a wrapping counter even if validation evolves.
    committed.next_nonce = committed
        .next_nonce
        .checked_add(1)
        .ok_or_else(|| rejected("administrator nonce exhausted"))?;
    match &certificate.plan.action {
        ChildRuntimeAdminActionV1::Schedule {
            change_set,
            provenance,
        } => {
            let notice = change_set::notice_for(&committed.policy.notice, change_set);
            if change_set
                .effective_height
                .checked_sub(height)
                .is_none_or(|distance| distance < notice.blocks)
            {
                return Err(rejected(
                    "scheduled child change does not satisfy its committed notice",
                ));
            }
            let bytes =
                norito::core::classify_decode_attempt(|| norito::encode_canonical(change_set))
                    .map_err(|error| {
                        state.attempt_error_to_instruction_error(canonical_decode_attempt_error(
                            error, rejected,
                        ))
                    })?;
            let id = ChangeSetId(
                Hash::new_from_chunks(&[
                    iroha_data_model::runtime::lifecycle::CHANGE_SET_ID_DOMAIN,
                    &bytes,
                ])
                .into(),
            );
            if state.world.runtime_change_sets.get(&id).is_some() {
                return Err(rejected("change set has already been recorded"));
            }
            match &committed.policy.provenance {
                Some(policy) => check_change_set_provenance(
                    id,
                    &policy.signers,
                    policy.threshold as usize,
                    provenance,
                )
                .map_err(rejected)?,
                None if !provenance.is_empty() => {
                    return Err(rejected(
                        "the explicit policy does not select provenance endorsements",
                    ));
                }
                None => {}
            }
            let due = |record: &ChangeSetRecordV1| {
                record.status == ChangeSetStatusV1::Scheduled
                    && record.change_set.effective_height < change_set.effective_height
            };
            let count = state
                .world
                .runtime_change_sets
                .iter()
                .filter(|(_, record)| due(record))
                .count();
            // Reserve only borrowed record coordinates; payloads stay with the original MV owner.
            let mut scheduled = norito::core::classify_decode_attempt(|| {
                let bytes = count
                    .checked_mul(core::mem::size_of::<(&ChangeSetId, &ChangeSetRecordV1)>())
                    .ok_or(norito::Error::LengthMismatch)?;
                norito::core::reserve_decode_allocation(bytes)?;
                let mut records = Vec::new();
                records
                    .try_reserve_exact(count)
                    .map_err(|_| norito::Error::AllocationFailed {
                        bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
                    })?;
                Ok(records)
            })
            .map_err(|error| {
                state.attempt_error_to_instruction_error(canonical_decode_attempt_error(
                    error, rejected,
                ))
            })?;
            for (id, record) in state.world.runtime_change_sets.iter() {
                if record.status != ChangeSetStatusV1::Scheduled {
                    continue;
                }
                if record.change_set.network != committed.root.network
                    || record.change_set.root != RootScopeV1::DataSpace(committed.root.dataspace)
                {
                    return Err(rejected(
                        "the child's scheduled timeline contains a foreign root",
                    ));
                }
                if due(record) {
                    scheduled.push((id, record));
                }
            }
            scheduled
                .sort_unstable_by_key(|(id, record)| (record.change_set.effective_height, **id));
            change_set::successor_from_ordered_records(
                change_set,
                id,
                state.world.runtime_selection.canonical(),
                &scheduled,
                &PublishedDefinitions(&state.world.runtime_definitions),
            )?;
            let now = change_set::block_ms(state);
            Ok(PreparedAction::Schedule(
                id,
                ChangeSetRecordV1 {
                    change_set: change_set.clone(),
                    status: ChangeSetStatusV1::Scheduled,
                    proposed_ms: now,
                    approved_height: height,
                    approved_ms: now,
                    notice,
                    attestations: Vec::new(),
                },
            ))
        }
        ChildRuntimeAdminActionV1::Cancel(id) => {
            let mut record = state
                .world
                .runtime_change_sets
                .get(id)
                .cloned()
                .ok_or_else(|| rejected("change set is not recorded"))?;
            if record.status != ChangeSetStatusV1::Scheduled
                || record.change_set.network != committed.root.network
                || record.change_set.root != RootScopeV1::DataSpace(committed.root.dataspace)
            {
                return Err(rejected(
                    "cancellation requires a scheduled change belonging to this child",
                ));
            }
            record.status = ChangeSetStatusV1::Cancelled(height);
            Ok(PreparedAction::Cancel(*id, record))
        }
        ChildRuntimeAdminActionV1::RotatePolicy(policy) => {
            committed.policy = policy.clone();
            Ok(PreparedAction::Rotate)
        }
    }
}

impl Execute for AuthorizeChildRuntimeActionV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        direct::consume(state, Self::WIRE_ID, &self)?;
        let mut committed = committed_authority(state)?;
        let prepared = prepare_action(&self.certificate, &mut committed, state)?;
        let event = match prepared {
            PreparedAction::Schedule(id, record) => {
                let event = RuntimeUpgradeEvent::Scheduled(RuntimeUpgradeScheduled {
                    id,
                    effective_height: record.change_set.effective_height,
                });
                super::containment::record_review(
                    &mut state.world,
                    id,
                    &record.change_set,
                    state._curr_block.height().get(),
                );
                state.world.runtime_change_sets.insert(id, record);
                Some(event)
            }
            PreparedAction::Cancel(id, record) => {
                state.world.runtime_change_sets.insert(id, record);
                Some(RuntimeUpgradeEvent::Canceled(RuntimeUpgradeCanceled {
                    id,
                    height: state._curr_block.height().get(),
                }))
            }
            PreparedAction::Rotate => None,
        };
        *state.world.runtime_child_admin_state.get_mut() = Some(committed);
        state.world.emit_events(event);
        Ok(())
    }
}

impl Execute for AttestChildRuntimeSupportV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        direct::consume(state, Self::WIRE_ID, &self)?;
        let committed = committed_authority(state)?;
        let attestation = &self.attestation.attestation;
        if attestation.network != committed.root.network
            || attestation.root != RootScopeV1::DataSpace(committed.root.dataspace)
        {
            return Err(rejected("validator support belongs to another root"));
        }
        change_set::attest(state, self.attestation)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod certified_tests;
