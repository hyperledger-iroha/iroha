//! Global QueuePlan admission custody, independent of lane production or signing.
use super::{
    output_guard::ConsensusOutputGuard,
    v2::VerifiedHeightContext,
    v2_lane_work::{V2LaneIngressOutcome, V2LaneWorkEffect, V2LaneWorkError},
};
use crate::{
    kura::Kura,
    queue::Queue,
    state::{
        PendingQueuePlanAdmissionDisposition, PendingQueuePlanAdmissionPersistenceOutcome, State,
    },
};
use iroha_crypto::Hash;
use iroha_data_model::block::consensus_v2 as wire;
use iroha_model_base::peer::PeerId;
use norito::codec::Encode;
use std::{
    collections::{BTreeSet, VecDeque},
    num::NonZeroUsize,
    sync::Arc,
};

/// One global-height admission owner retaining the original live storage and output corridor.
pub(crate) struct QueuePlanAdmissionOwner {
    context: wire::HeightContext,
    local_peer: PeerId,
    voting_enabled: bool,
    state: Arc<State>,
    kura: Arc<Kura>,
    queue: Arc<Queue>,
    output_guard: Arc<ConsensusOutputGuard>,
    effect_capacity: NonZeroUsize,
    effects: VecDeque<V2LaneWorkEffect>,
    effect_keys: BTreeSet<Hash>,
    queue_plan_admission_handoff: QueuePlanAdmissionHandoffState,
    queue_plan_admission_handoff_cursor: usize,
}

impl QueuePlanAdmissionOwner {
    /// Compare the original resources and immutable height before transferring output.
    pub(in crate::sumeragi) fn matches_lifecycle_dependencies(
        &self,
        context: &wire::HeightContext,
        state: &Arc<State>,
        kura: &Arc<Kura>,
        output_guard: &Arc<ConsensusOutputGuard>,
        local_peer: &PeerId,
    ) -> bool {
        self.context == *context
            && Arc::ptr_eq(&self.state, state)
            && Arc::ptr_eq(&self.kura, kura)
            && Arc::ptr_eq(&self.output_guard, output_guard)
            && self.local_peer == *local_peer
    }

    /// Preserve inherited classifier fixtures with deliberately obsolete height authority.
    #[cfg(test)]
    pub(super) fn set_context_for_test(&mut self, context: wire::HeightContext) {
        // Inherited classifier fixtures deliberately hold an obsolete process
        // height while advancing and reverting their committed World frontier.
        self.context = context;
    }

    /// Retain authenticated height authority and the original process resources.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        context: &VerifiedHeightContext,
        local_peer: PeerId,
        voting_enabled: bool,
        state: Arc<State>,
        kura: Arc<Kura>,
        queue: Arc<Queue>,
        output_guard: Arc<ConsensusOutputGuard>,
        effect_capacity: NonZeroUsize,
    ) -> Result<Self, V2LaneWorkError> {
        Self::from_context(
            context.context().clone(),
            local_peer,
            voting_enabled,
            state,
            kura,
            queue,
            output_guard,
            effect_capacity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_context(
        context: wire::HeightContext,
        local_peer: PeerId,
        voting_enabled: bool,
        state: Arc<State>,
        kura: Arc<Kura>,
        queue: Arc<Queue>,
        output_guard: Arc<ConsensusOutputGuard>,
        effect_capacity: NonZeroUsize,
    ) -> Result<Self, V2LaneWorkError> {
        Self::validate_context(&context, &state, &kura)?;
        if output_guard.restart_required() {
            return Err(V2LaneWorkError::RestartRequired);
        }
        Ok(Self {
            context,
            local_peer,
            voting_enabled,
            state,
            kura,
            queue,
            output_guard,
            effect_capacity,
            effects: VecDeque::new(),
            effect_keys: BTreeSet::new(),
            queue_plan_admission_handoff: QueuePlanAdmissionHandoffState::Unobserved,
            queue_plan_admission_handoff_cursor: 0,
        })
    }

    fn validate_context(
        context: &wire::HeightContext,
        state: &State,
        kura: &Arc<Kura>,
    ) -> Result<(), V2LaneWorkError> {
        context
            .validate()
            .map_err(|error| V2LaneWorkError::InvalidContext(error.to_string()))?;
        if !state.matches_kura_instance(kura) || state.network_id_ref() != &context.network_id {
            return Err(V2LaneWorkError::InvalidContext(
                "QueuePlan admission must retain the original State/Kura and network".to_owned(),
            ));
        }
        let height = u64::try_from(state.committed_height())
            .map_err(|_| V2LaneWorkError::StateHeightMismatch)?;
        if height != context.height && height.checked_add(1) != Some(context.height) {
            return Err(V2LaneWorkError::StateHeightMismatch);
        }
        Ok(())
    }

    /// Replace only authenticated height authority; original resources remain retained.
    pub(crate) fn rollover(
        &mut self,
        context: &VerifiedHeightContext,
    ) -> Result<(), V2LaneWorkError> {
        self.rollover_context(context.context())
    }

    fn rollover_context(&mut self, context: &wire::HeightContext) -> Result<(), V2LaneWorkError> {
        Self::validate_context(context, &self.state, &self.kura)?;
        if context.id() == self.context.id() {
            return Ok(());
        }
        if self.context.height.checked_add(1) != Some(context.height) {
            return Err(V2LaneWorkError::InvalidContext(
                "QueuePlan admission rollover requires the exact successor height".to_owned(),
            ));
        }
        if self.output_guard.restart_required() {
            return Err(V2LaneWorkError::RestartRequired);
        }
        // Only this owner's old-height occurrences retire. Already transferred
        // output is owned by its original worker; durable certificate bytes remain in Kura.
        self.context = context.clone();
        self.effects.clear();
        self.effect_keys.clear();
        self.queue_plan_admission_handoff = QueuePlanAdmissionHandoffState::Unobserved;
        self.queue_plan_admission_handoff_cursor = 0;
        Ok(())
    }

    /// Reconcile durable inputs under the shared fail-stop barrier.
    pub(crate) fn reconcile(&mut self, view: wire::View) -> Result<Vec<Vec<u8>>, V2LaneWorkError> {
        let guard = Arc::clone(&self.output_guard);
        let operation = guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        let selected = self.reconcile_inner(view)?;
        operation.complete();
        Ok(selected)
    }

    /// Service a bounded inventory turn, without claiming network delivery.
    pub(crate) fn refresh(&mut self, view: wire::View) -> Result<bool, V2LaneWorkError> {
        self.reconcile(view)?;
        Ok(self.queue_plan_admission_handoff.is_enqueued())
    }

    /// Admit one complete certificate through the original State persistence corridor.
    pub(crate) fn accept_certificate(
        &mut self,
        sender: PeerId,
        certificate: Arc<Vec<u8>>,
        view: wire::View,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        let guard = Arc::clone(&self.output_guard);
        let operation = guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        let outcome = self.accept_certificate_inner(sender, certificate, view)?;
        operation.complete();
        Ok(outcome)
    }

    /// Inspect the same retained occurrence until downstream explicitly accepts it.
    pub(crate) fn next_effect(&self) -> Option<V2LaneWorkEffect> {
        let _permit = self.output_guard.acquire()?;
        self.effects.front().cloned()
    }

    /// Retire exactly the inspected occurrence after successful downstream admission.
    pub(crate) fn acknowledge_effect(&mut self, effect: &V2LaneWorkEffect) -> bool {
        let guard = Arc::clone(&self.output_guard);
        let Some(_permit) = guard.acquire() else {
            return false;
        };
        let exact = match (self.effects.front(), effect) {
            (
                Some(V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                    peer,
                    view,
                    certificate,
                }),
                V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                    peer: accepted_peer,
                    view: accepted_view,
                    certificate: accepted_certificate,
                },
            ) => {
                peer == accepted_peer
                    && view == accepted_view
                    && Arc::ptr_eq(certificate, accepted_certificate)
            }
            _ => false,
        };
        if !exact {
            return false;
        }
        self.effects.pop_front();
        self.effect_keys.remove(&queue_plan_effect_key(effect));
        true
    }

    /// Preserve an existing owner's allocation when downstream is backpressured.
    pub(crate) fn rotate_next_effect(&mut self) -> bool {
        let guard = Arc::clone(&self.output_guard);
        let Some(_permit) = guard.acquire() else {
            return false;
        };
        let Some(effect) = self.effects.pop_front() else {
            return false;
        };
        self.effects.push_back(effect);
        true
    }

    /// Count source-owned occurrences, excluding output already accepted by workers.
    pub(crate) fn effect_count(&self) -> usize {
        self.effects.len()
    }

    fn local_validator_index(&self) -> Option<wire::ValidatorIndex> {
        if !self.voting_enabled {
            return None;
        }
        self.context
            .roster
            .iter()
            .position(|entry| entry.validator == self.local_peer)
            .and_then(|index| u32::try_from(index).ok())
    }

    fn push_effect(&mut self, effect: V2LaneWorkEffect) -> bool {
        if self.output_guard.restart_required() {
            return false;
        }
        let key = queue_plan_effect_key(&effect);
        if self.effect_keys.contains(&key) {
            return true;
        }
        if self.effects.len() >= self.effect_capacity.get() {
            return false;
        }
        self.effect_keys.insert(key);
        self.effects.push_back(effect);
        true
    }
}

fn queue_plan_effect_key(effect: &V2LaneWorkEffect) -> Hash {
    let mut encoded = Vec::new();
    if let V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
        peer,
        view,
        certificate,
    } = effect
    {
        encoded.push(6);
        encoded.extend(peer.encode());
        encoded.extend(view.encode());
        encoded.extend(certificate.as_ref().encode());
    }
    Hash::new(encoded)
}

/// One observed durable inventory routed under one exact consensus authority.
/// The sorted certificate hashes are the pending-admission generation: coalesced
/// wakeups need no independent sequence number or mutable transport authority.
#[derive(Clone, Debug, PartialEq, Eq)]
struct QueuePlanAdmissionHandoffGeneration {
    round: wire::ConsensusRound,
    leader: PeerId,
    pending: BTreeSet<Hash>,
}

impl QueuePlanAdmissionHandoffGeneration {
    fn same_destination(&self, other: &Self) -> bool {
        self.round == other.round && self.leader == other.leader
    }
}

#[derive(Debug)]
struct QueuePlanAdmissionHandoffProgress {
    generation: QueuePlanAdmissionHandoffGeneration,
    enqueued: BTreeSet<Hash>,
}

/// Readiness is scoped to the exact view and durable certificate inventory.
/// `Enqueued` means ownership reached the retained owner/output corridor; it
/// never means network delivery, canonical admission, or transaction finality.
#[derive(Debug)]
enum QueuePlanAdmissionHandoffState {
    Unobserved,
    Pending(QueuePlanAdmissionHandoffProgress),
    Enqueued(QueuePlanAdmissionHandoffProgress),
}

impl QueuePlanAdmissionHandoffState {
    fn progress(&self) -> Option<&QueuePlanAdmissionHandoffProgress> {
        match self {
            Self::Unobserved => None,
            Self::Pending(progress) | Self::Enqueued(progress) => Some(progress),
        }
    }

    fn needs_refresh(&self, round: wire::ConsensusRound, leader: &PeerId) -> bool {
        match self {
            Self::Enqueued(progress) => {
                progress.generation.round != round || &progress.generation.leader != leader
            }
            Self::Unobserved | Self::Pending(_) => true,
        }
    }

    fn begin(&mut self, generation: QueuePlanAdmissionHandoffGeneration) {
        let enqueued = self
            .progress()
            .filter(|progress| progress.generation.same_destination(&generation))
            .map(|progress| {
                progress
                    .enqueued
                    .intersection(&generation.pending)
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        *self = Self::Pending(QueuePlanAdmissionHandoffProgress {
            generation,
            enqueued,
        });
    }

    fn is_enqueued(&self) -> bool {
        matches!(self, Self::Enqueued(_))
    }

    fn contains(&self, hash: &Hash) -> bool {
        self.progress()
            .is_some_and(|progress| progress.enqueued.contains(hash))
    }

    fn admit(&mut self, generation: &QueuePlanAdmissionHandoffGeneration, hash: Hash) -> bool {
        match self {
            Self::Pending(progress)
                if progress.generation == *generation && generation.pending.contains(&hash) =>
            {
                progress.enqueued.insert(hash);
                true
            }
            _ => false,
        }
    }

    fn finish(&mut self, generation: &QueuePlanAdmissionHandoffGeneration) -> bool {
        if !matches!(self, Self::Pending(progress) if progress.generation == *generation) {
            return false;
        }
        if let Self::Pending(progress) = std::mem::replace(self, Self::Unobserved) {
            *self = Self::Enqueued(progress);
            true
        } else {
            false
        }
    }
}

impl QueuePlanAdmissionOwner {
    /// A new destination or a capacity-pending inventory needs another bounded turn.
    pub(crate) fn needs_refresh(&self, active_view: wire::View) -> Result<bool, V2LaneWorkError> {
        let leader = self
            .context
            .roster
            .get(usize::try_from(self.context.leader(active_view)).unwrap_or(usize::MAX))
            .map(|entry| &entry.validator)
            .ok_or_else(|| {
                V2LaneWorkError::InvalidContext(
                    "QueuePlan handoff leader is outside the frozen roster".to_owned(),
                )
            })?;
        Ok(self.queue_plan_admission_handoff.needs_refresh(
            wire::ConsensusRound {
                context_id: self.context.id(),
                height: self.context.height,
                view: active_view,
            },
            leader,
        ))
    }

    fn reconcile_inner(
        &mut self,
        active_view: wire::View,
    ) -> Result<Vec<Vec<u8>>, V2LaneWorkError> {
        let leader_index = self.context.leader(active_view);
        let leader_peer = self
            .context
            .roster
            .get(usize::try_from(leader_index).unwrap_or(usize::MAX))
            .map(|entry| entry.validator.clone())
            .ok_or_else(|| {
                V2LaneWorkError::InvalidContext(
                    "current global leader is outside the frozen roster".to_owned(),
                )
            })?;
        let local_is_leader = self.local_validator_index() == Some(leader_index);
        let mut admissions = Vec::new();
        let mut pending = self
            .kura
            .pending_queue_plan_admission_certificates_bounded(
                self.kura.pending_queue_plan_admission_capacity(),
            )
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        let generation = QueuePlanAdmissionHandoffGeneration {
            round: wire::ConsensusRound {
                context_id: self.context.id(),
                height: self.context.height,
                view: active_view,
            },
            leader: leader_peer.clone(),
            pending: pending.iter().map(|(hash, _)| *hash).collect(),
        };
        // A certified view transition supersedes only the old owner-held
        // handoff. Kura remains the exact source; already transferred worker
        // occurrences cannot complete this new generation.
        self.effects.retain(|effect| match effect {
            V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                peer,
                view,
                certificate,
            } => {
                *view == active_view
                    && *peer == leader_peer
                    && generation
                        .pending
                        .contains(&Hash::new(certificate.as_slice()))
            }
            _ => true,
        });
        self.effect_keys = self.effects.iter().map(queue_plan_effect_key).collect();
        self.queue_plan_admission_handoff.begin(generation.clone());
        let count = pending.len();
        let start = if local_is_leader || count == 0 {
            0
        } else {
            self.queue_plan_admission_handoff_cursor % count
        };
        pending.rotate_left(start);
        let mut completed = true;
        for (offset, (certificate_hash, certificate_bytes)) in pending.into_iter().enumerate() {
            let (admission, disposition) = self
                .state
                .classify_pending_queue_plan_admission(&certificate_bytes, self.context.height)
                .map_err(|error| {
                    V2LaneWorkError::Persistence(format!(
                        "pending QueuePlan admission certificate cannot be reconciled: {error}"
                    ))
                })?;
            match disposition {
                PendingQueuePlanAdmissionDisposition::ExactPending => {
                    // The finalized first-admission carrier now retains the exact input.
                    // Keep this bounded pending copy until terminal application so existing
                    // queue/reservation cleanup keeps its exact durable handoff owner.
                }
                PendingQueuePlanAdmissionDisposition::Applied => {
                    self.state
                        .remove_pending_queue_plan_admission_certificate(certificate_hash)
                        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
                }
                PendingQueuePlanAdmissionDisposition::DefinitiveConflict
                | PendingQueuePlanAdmissionDisposition::Stale => {
                    self.queue
                        .reject_exact_queue_plan_admission_claim(&admission.certificate.binding)
                        .map_err(|error| {
                            V2LaneWorkError::Persistence(format!(
                                "losing QueuePlan admission queue claim cannot be durably rejected: {error}"
                            ))
                        })?;
                    self.state
                        .remove_pending_queue_plan_admission_certificate(certificate_hash)
                        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
                }
                PendingQueuePlanAdmissionDisposition::EligibleAbsent if local_is_leader => {
                    admissions.push((
                        admission.registry_key,
                        admission.registry_value,
                        certificate_bytes,
                    ));
                }
                PendingQueuePlanAdmissionDisposition::EligibleAbsent => {
                    if self
                        .queue_plan_admission_handoff
                        .contains(&certificate_hash)
                    {
                        continue;
                    }
                    let effect = V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                        peer: leader_peer.clone(),
                        view: active_view,
                        certificate: Arc::new(certificate_bytes),
                    };
                    let queued = self.effect_keys.contains(&queue_plan_effect_key(&effect));
                    if !queued && !self.push_effect(effect) {
                        self.queue_plan_admission_handoff_cursor = (start + offset) % count;
                        completed = false;
                        break;
                    }
                    if !self
                        .queue_plan_admission_handoff
                        .admit(&generation, certificate_hash)
                    {
                        return Err(V2LaneWorkError::InvalidContext(
                            "QueuePlan handoff admission lost its exact generation".to_owned(),
                        ));
                    }
                }
                PendingQueuePlanAdmissionDisposition::Future { .. }
                | PendingQueuePlanAdmissionDisposition::DeferredCarrier => {}
            }
        }
        if !local_is_leader && count != 0 && completed {
            self.queue_plan_admission_handoff_cursor = (start + 1) % count;
        }
        if completed && !self.queue_plan_admission_handoff.finish(&generation) {
            return Err(V2LaneWorkError::InvalidContext(
                "QueuePlan handoff completion lost its exact generation".to_owned(),
            ));
        }
        admissions.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        let mut selected = Vec::new();
        let mut selected_bytes = 0usize;
        let mut previous_registry_key = None;
        for (registry_key, _, certificate) in admissions {
            if previous_registry_key.as_ref() == Some(&registry_key) {
                continue;
            }
            let Some(next_bytes) = selected_bytes.checked_add(certificate.len()) else {
                break;
            };
            if selected.len() == iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK
                || next_bytes > iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSIONS_BYTES
            {
                break;
            }
            selected_bytes = next_bytes;
            previous_registry_key = Some(registry_key.clone());
            selected.push((registry_key, certificate));
        }
        Ok(selected
            .into_iter()
            .map(|(_, certificate)| certificate)
            .collect())
    }

    fn accept_certificate_inner(
        &mut self,
        _sender: PeerId,
        certificate: Arc<Vec<u8>>,
        active_view: wire::View,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        let local_is_leader = self
            .context
            .roster
            .get(usize::try_from(self.context.leader(active_view)).unwrap_or(usize::MAX))
            .is_some_and(|entry| entry.validator == self.local_peer);
        if !local_is_leader {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let Ok(outcome) = self.state.persist_classified_queue_plan_admission(
            certificate.as_slice(),
            crate::state::QueuePlanAdmissionPersistenceScope::Admission,
        ) else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        match outcome {
            PendingQueuePlanAdmissionPersistenceOutcome::Applied { .. } => {
                Ok(V2LaneIngressOutcome::Duplicate)
            }
            PendingQueuePlanAdmissionPersistenceOutcome::Rejected { .. } => {
                Ok(V2LaneIngressOutcome::Rejected)
            }
            PendingQueuePlanAdmissionPersistenceOutcome::Durable { inserted, .. } => {
                Ok(if inserted {
                    V2LaneIngressOutcome::Inserted
                } else {
                    V2LaneIngressOutcome::Duplicate
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_plan_handoff_stale_generation_cannot_complete_a_new_destination() {
        let first_view = 1;
        let first_peer = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let next_peer = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let first = QueuePlanAdmissionHandoffGeneration {
            round: wire::ConsensusRound {
                context_id: wire::HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(
                    Hash::new(b"exact context"),
                )),
                height: 2,
                view: first_view,
            },
            leader: first_peer,
            pending: BTreeSet::from([Hash::new(b"exact retained certificate")]),
        };
        let mut next = first.clone();
        next.round.view += 1;
        next.leader = next_peer;
        let hash = *first.pending.first().unwrap();
        let mut state = QueuePlanAdmissionHandoffState::Unobserved;
        state.begin(first.clone());
        assert!(state.admit(&first, hash));
        state.begin(next.clone());
        assert!(!state.contains(&hash));
        assert!(!state.admit(&first, hash));
        assert!(!state.finish(&first));
        assert!(state.needs_refresh(next.round, &next.leader));
        assert!(state.admit(&next, hash));
        assert!(state.finish(&next));
        assert!(!state.needs_refresh(next.round, &next.leader));
        let mut arrival = next.clone();
        let new_hash = Hash::new(b"new durable inventory member");
        arrival.pending.insert(new_hash);
        state.begin(arrival.clone());
        assert!(
            state.contains(&hash),
            "the prior exact transfer survives an arrival"
        );
        assert!(
            !state.finish(&next),
            "an older inventory cannot complete the new generation"
        );
        assert!(state.admit(&arrival, new_hash));
        assert!(state.finish(&arrival));
    }
}
