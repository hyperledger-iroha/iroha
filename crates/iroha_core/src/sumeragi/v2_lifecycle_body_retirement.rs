//! Durable cancellation of exact ordinary body work with a superseded reducer owner.

use super::{
    CapacityClass, LifecycleCoordinator, LifecyclePhase, LifecycleStageKind, LifecycleState,
    LifecycleWorkClass, PredecessorScope, TerminalOutcome, TurnLease, TurnOutcome,
    body_pipeline_transition::BodyStageTransitionError,
    schema::{DurableContinuation, DurablePayloadReference},
    work_registry::{CancelledCertifiedBodyWorkV1, PreparedCertifiedBodyRetirementV1},
};
use crate::sumeragi::v2::VerifiedHeightContext;

/// Exact cancellation staged while the registry and reducer remain exclusively borrowed.
#[must_use = "the staged obsolete body cancellation has not been published"]
pub(super) struct PreparedBodyRetirementTransitionV1<'coordinator, 'registry, 'adapter> {
    coordinator: &'coordinator mut LifecycleCoordinator,
    retirement: PreparedCertifiedBodyRetirementV1<'registry, 'adapter>,
    staged: LifecycleCoordinator,
}

impl LifecycleCoordinator {
    /// Authenticate and stage one no-child Cancelled row without changing live state.
    pub(super) fn prepare_body_retirement_transition<'coordinator, 'registry, 'adapter>(
        &'coordinator mut self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        retirement: PreparedCertifiedBodyRetirementV1<'registry, 'adapter>,
    ) -> Result<
        PreparedBodyRetirementTransitionV1<'coordinator, 'registry, 'adapter>,
        BodyStageTransitionError,
    > {
        if self.active_context != super::projection::lifecycle_context(verified.context()) {
            return Err(BodyStageTransitionError::StaleLease);
        }
        let payload = retirement
            .project_for_cancellation(lease, verified)
            .map_err(|_| BodyStageTransitionError::InvalidBodyFrameReference)?;
        let staged = stage_body_retirement(self, lease, payload)?;
        Ok(PreparedBodyRetirementTransitionV1 {
            coordinator: self,
            retirement,
            staged,
        })
    }
}

fn stage_body_retirement(
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    payload: DurablePayloadReference,
) -> Result<LifecycleCoordinator, BodyStageTransitionError> {
    if !matches!(
        (
            lease.work_class(),
            lease.key().phase(),
            lease.stage().kind()
        ),
        (
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch | LifecyclePhase::FetchDecision,
            LifecycleStageKind::FetchBody
        ) | (
            LifecycleWorkClass::Store,
            LifecyclePhase::Store | LifecyclePhase::StoreDecision,
            LifecycleStageKind::StoreBody
        )
    ) || lease.stage().predecessor_scope() != PredecessorScope::Independent
        || lease.physical_slots().len() != 1
        || lease
            .physical_slots()
            .keys()
            .any(|slot| slot.capacity_class() != Some(CapacityClass::Effect))
        || lease.output_reservation().is_some()
        || coordinator.active_lease.as_ref() != Some(lease)
        || coordinator.fault.is_some()
        || coordinator.ledger_store.is_none()
    {
        return Err(BodyStageTransitionError::WrongParentShape);
    }
    let parent = coordinator
        .records
        .get(&lease.ordinal())
        .ok_or(BodyStageTransitionError::StaleLease)?;
    let metadata = coordinator
        .durable_records
        .get(&lease.ordinal())
        .ok_or(BodyStageTransitionError::StaleLease)?;
    let slots = lease.physical_slots().keys().copied().collect();
    if parent.ordinal != lease.ordinal()
        || parent.owner != lease.owner()
        || parent.key != lease.key()
        || parent.work_class != lease.work_class()
        || parent.stage != lease.stage()
        || parent.state != LifecycleState::Claimed(lease.id())
        || parent.physical_slots != *lease.physical_slots()
        || parent.episode.slot_universe != slots
        || parent.episode.consumed_slots != slots
        || !parent.episode.frozen_predecessors.is_empty()
        || coordinator
            .episode_authority
            .universe_for(parent.key)
            .as_ref()
            != Some(&parent.episode.universe)
        || !coordinator
            .episode_authority
            .admits_slots(CapacityClass::Effect, &slots)
        || coordinator.key_index.get(&parent.key) != Some(&parent.ordinal)
        || coordinator.owner_index.get(&parent.owner.causal_root()) != Some(&parent.owner)
        || coordinator
            .records
            .values()
            .filter(|record| record.key == parent.key)
            .count()
            != 1
        || coordinator
            .key_index
            .values()
            .filter(|ordinal| **ordinal == parent.ordinal)
            .count()
            != 1
        || coordinator
            .owner_index
            .values()
            .filter(|owner| **owner == parent.owner)
            .count()
            != 1
        || coordinator.ready_index.contains(&parent.ordinal)
        || metadata.payload != payload
        || metadata.reconstruction_source != parent.owner.causal_root().digest()
        || metadata.continuation != DurableContinuation::None
        || !matches!(payload, DurablePayloadReference::BodyFrame(frame) if frame.matches_key(parent.key))
    {
        return Err(BodyStageTransitionError::StaleLease);
    }
    let expected_used = coordinator.capacity_used[&CapacityClass::Effect]
        .checked_sub(1)
        .ok_or(BodyStageTransitionError::InvalidCapacityTransition)?;
    let expected_generation = coordinator.capacity_generation[&CapacityClass::Effect]
        .checked_add(1)
        .ok_or(BodyStageTransitionError::InvalidCapacityTransition)?;
    let terminal_payload = payload
        .terminalized(TerminalOutcome::Cancelled)
        .ok_or(BodyStageTransitionError::InvalidBodyFrameReference)?;
    let terminal_replay = metadata
        .terminalized_replay_authority(
            coordinator.active_context,
            parent.key,
            parent.work_class,
            parent.stage,
            terminal_payload,
        )
        .ok_or(BodyStageTransitionError::InvalidBodyFrameReference)?;
    let mut expected_parent = parent.clone();
    expected_parent.state = LifecycleState::Terminal(TerminalOutcome::Cancelled);
    let mut expected_metadata = metadata.clone();
    expected_metadata.payload = terminal_payload;
    expected_metadata.replay_authority = terminal_replay;
    let mut staged = coordinator.stage_durable_transaction();
    staged.reduce_settle_turn(
        lease.clone(),
        TurnOutcome::Terminal(TerminalOutcome::Cancelled),
        None,
    );
    if let Some(fault) = staged.fault {
        return Err(BodyStageTransitionError::ParentSettlement(fault));
    }
    if staged.active_lease.is_some()
        || staged.next_lease != coordinator.next_lease
        || staged.high_water != coordinator.high_water
        || staged.records.len() != coordinator.records.len()
        || staged.durable_records.len() != coordinator.durable_records.len()
        || staged.records.get(&lease.ordinal()) != Some(&expected_parent)
        || staged.durable_records.get(&lease.ordinal()) != Some(&expected_metadata)
        || staged.records.iter().any(|(ordinal, record)| {
            *ordinal != lease.ordinal() && coordinator.records.get(ordinal) != Some(record)
        })
        || staged.durable_records.iter().any(|(ordinal, metadata)| {
            *ordinal != lease.ordinal()
                && coordinator.durable_records.get(ordinal) != Some(metadata)
        })
        || staged.key_index != coordinator.key_index
        || staged.owner_index != coordinator.owner_index
        || staged.ready_index != coordinator.ready_index
        || staged.admission_waits != coordinator.admission_waits
        || staged.producer_debts != coordinator.producer_debts
        || staged.observed_generation != coordinator.observed_generation
        || staged.capacity_used[&CapacityClass::Effect] != expected_used
        || staged.capacity_generation[&CapacityClass::Effect] != expected_generation
        || CapacityClass::ALL
            .into_iter()
            .filter(|class| *class != CapacityClass::Effect)
            .any(|class| {
                staged.capacity_used[&class] != coordinator.capacity_used[&class]
                    || staged.capacity_generation[&class] != coordinator.capacity_generation[&class]
            })
    {
        return Err(BodyStageTransitionError::InvalidCapacityTransition);
    }
    Ok(staged)
}

impl PreparedBodyRetirementTransitionV1<'_, '_, '_> {
    /// Persist the exact cancelled row before retiring either volatile carrier.
    pub(super) fn persist_exact_cancellation(
        &self,
    ) -> Result<(), super::ledger::LifecycleLedgerError> {
        self.coordinator
            .persist_exact_staged_successor(&self.staged)
    }

    /// Publish only the already-fsynced registry and coordinator moves.
    pub(super) fn commit_after_publication(self) -> CancelledCertifiedBodyWorkV1 {
        let Self {
            coordinator,
            retirement,
            staged,
        } = self;
        let marker = retirement.commit_after_publication();
        *coordinator = staged;
        marker
    }
}
