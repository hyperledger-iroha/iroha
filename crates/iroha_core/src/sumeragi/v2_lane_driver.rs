//! Process-lived service of native lane control, physical jobs and exact evidence.
//!
//! The shared reducer is the only view/lock/signing authority. This driver owns
//! bounded physical queues and rotates service among the actual instances; no
//! global height or view participates in its clocks. Candidate consumers receive
//! authenticated evidence, never an acknowledgement of the retained Apply effect.
//! TODO: activate this driver together with the native candidate/Apply consumer
//! and retire the old fresh signer at that single production boundary.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroUsize,
    sync::{Arc, mpsc},
    time::Instant,
};

use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::block::{
    consensus_v2::HeightContextId,
    lane_consensus::{
        LANE_MESSAGE_VERSION_V1, LaneDecisionV1, LaneMessageEnvelopeV1, LaneMessageV1,
    },
};

use super::{
    InboundBlockMessage,
    message::BlockMessage,
    output_guard::ConsensusOutputGuard,
    v2_core as core,
    v2_lane_instance::{
        LaneClosedInstance, LaneCurrentGate, LaneInputOutcome, LaneOutbound, LanePhysicalPool,
        LanePhysicalShutdown, LaneProcessLimits, LaneProcessOwner, LaneProcessProgress,
        LaneWorkerClass,
    },
    v2_lane_wire::LaneAuthenticator,
};
use crate::state::{
    FirstLaneAdmittedInputReadV1, LaneDecisionGroupPreparationV1, State, VerifiedLaneContexts,
    VerifiedLaneDecisionGroupV1,
};

type Result<T> = std::result::Result<T, String>;

/// Physical queue bounds supplied by the existing process configuration owner.
/// These bounds do not alter native quorum, opening or payload rules.
#[derive(Clone, Copy)]
pub(crate) struct NativeLaneDriverLimits {
    pub(crate) process: LaneProcessLimits,
    /// Immutable node role; committee membership alone never enables an observer to sign.
    pub(crate) voting_enabled: bool,
    pub(crate) ingress: NonZeroUsize,
    pub(crate) outbound: NonZeroUsize,
    pub(crate) maximum_message_bytes: NonZeroUsize,
}

/// Untrusted native ingress. Decision evidence is also needed by global leaders
/// outside a route committee; it grants no voting or body-readiness authority.
#[derive(Debug)]
pub(crate) enum NativeLaneInput {
    Control(LaneMessageEnvelopeV1),
    Decision(LaneDecisionV1),
}

#[derive(Clone, Copy)]
enum NativeLaneInputRef<'a> {
    Control(&'a LaneMessageEnvelopeV1),
    Decision(&'a LaneDecisionV1),
}

impl NativeLaneInputRef<'_> {
    fn instance(self) -> HeightContextId {
        let hash = match self {
            Self::Control(envelope) => message_instance(&envelope.message),
            Self::Decision(decision) => decision.value().instance_id,
        };
        HeightContextId(HashOf::from_untyped_unchecked(hash))
    }
}

impl NativeLaneInput {
    fn as_ref(&self) -> NativeLaneInputRef<'_> {
        match self {
            Self::Control(envelope) => NativeLaneInputRef::Control(envelope),
            Self::Decision(decision) => NativeLaneInputRef::Decision(decision),
        }
    }
}

/// Admission never discards a full-queue or stale-observation input.
#[derive(Debug)]
pub(crate) enum NativeLaneAdmission {
    Accepted,
    Retry(NativeLaneInput),
    Rejected {
        input: NativeLaneInput,
        reason: String,
    },
}

/// The exact fair-ingress occurrence returns intact when admission cannot progress.
/// Accepted means its original payload moved into the process owner; no replay,
/// decode, clone or new physical admission ordinal stands in for that transfer.
pub(crate) enum NativeLaneOwnedAdmission {
    Accepted,
    Retry(InboundBlockMessage),
    Rejected {
        inbound: InboundBlockMessage,
        reason: String,
    },
}

enum NativeLaneAdmissionCheck {
    Ready { id: HeightContextId, local: bool },
    Duplicate,
    Retry,
    Rejected(String),
}

pub(super) fn message_instance(message: &LaneMessageV1) -> Hash {
    match message {
        LaneMessageV1::Proposal(proposal) => proposal.body.round.instance_id,
        LaneMessageV1::Vote(vote) => vote.statement.round.instance_id,
        LaneMessageV1::QuorumCertificate(qc) => qc.statement.round.instance_id,
        LaneMessageV1::TimeoutVote(vote) => vote.body.round.instance_id,
        LaneMessageV1::TimeoutCertificate(tc) => tc.round.instance_id,
    }
}

/// Original process owner; keep this outside the global-height loop.
pub(crate) struct NativeLaneDriver {
    state: Arc<State>,
    guard: Arc<ConsensusOutputGuard>,
    key: KeyPair,
    limits: NativeLaneDriverLimits,
    process: LaneProcessOwner,
    pool: LanePhysicalPool,
    ingress: VecDeque<(HeightContextId, LaneMessageEnvelopeV1)>,
    /// One authenticated Commit value per currently observed immutable instance.
    /// This cache contains evidence, not a local reducer or an Apply owner.
    decisions: BTreeMap<HeightContextId, LaneDecisionV1>,
    send: mpsc::SyncSender<LaneOutbound>,
    receive: mpsc::Receiver<LaneOutbound>,
    last_serviced: Option<HeightContextId>,
    last_retirement: Option<HeightContextId>,
    #[cfg(test)]
    held_body: Option<(HeightContextId, Box<dyn FnOnce() + Send>)>,
}

impl NativeLaneDriver {
    /// No disk operation runs here. Actual opens are issued to the bounded pool.
    pub(crate) fn new(
        state: Arc<State>,
        guard: Arc<ConsensusOutputGuard>,
        key: KeyPair,
        limits: NativeLaneDriverLimits,
    ) -> Result<Self> {
        let process = LaneProcessOwner::new(Arc::clone(&state), Arc::clone(&guard), limits.process)
            .map_err(|error| error.to_string())?;
        let pool = LanePhysicalPool::new(Arc::clone(&state), Arc::clone(&guard), limits.process)
            .map_err(|error| error.to_string())?;
        let (send, receive) = mpsc::sync_channel(limits.outbound.get());
        Ok(Self {
            state,
            guard,
            key,
            limits,
            process,
            pool,
            ingress: VecDeque::new(),
            decisions: BTreeMap::new(),
            send,
            receive,
            last_serviced: None,
            last_retirement: None,
            #[cfg(test)]
            held_body: None,
        })
    }

    /// Authenticate and inspect capacity before moving any physical payload.
    fn check_admission(
        &self,
        observed: &VerifiedLaneContexts,
        input: NativeLaneInputRef<'_>,
    ) -> NativeLaneAdmissionCheck {
        if self.guard.restart_required() || !observed.is_current(&self.state) {
            return NativeLaneAdmissionCheck::Retry;
        }
        let id = input.instance();
        let Some(lane) = observed
            .contexts()
            .iter()
            .find(|lane| lane.instance_id() == id)
        else {
            return NativeLaneAdmissionCheck::Rejected(
                "native ingress has no exact current opening".into(),
            );
        };
        let local = self.limits.voting_enabled
            && lane
                .frozen()
                .committee
                .iter()
                .any(|peer| peer.public_key() == self.key.public_key());
        let authenticate = || -> Result<()> {
            let bytes = match input {
                NativeLaneInputRef::Control(envelope) => {
                    if !local {
                        return Err(
                            "native control targets a committee without this voting signer".into(),
                        );
                    }
                    if envelope.version != LANE_MESSAGE_VERSION_V1 {
                        return Err("unsupported native lane envelope revision".into());
                    }
                    envelope
                        .message
                        .validate_shape(lane.frozen().committee.len())
                        .map_err(|error| error.to_string())?;
                    let context = lane.reducer_context();
                    let tag = core::EventTag::new(context.height(), 0, core::Generation::INITIAL);
                    LaneAuthenticator::new(lane)
                        .event(&envelope.message, tag)
                        .map_err(|error| error.to_string())?;
                    norito::encode_canonical(envelope).map_err(|error| error.to_string())?
                }
                NativeLaneInputRef::Decision(decision) => {
                    LaneAuthenticator::new(lane)
                        .decision_certificate(decision)
                        .map_err(|error| error.to_string())?;
                    norito::encode_canonical(decision).map_err(|error| error.to_string())?
                }
            };
            if bytes.len() > self.limits.maximum_message_bytes.get() {
                return Err("native message exceeds its admitted frame bound".into());
            }
            Ok(())
        };
        if let Err(reason) = authenticate() {
            return NativeLaneAdmissionCheck::Rejected(reason);
        }
        match input {
            NativeLaneInputRef::Control(_) if self.ingress.len() == self.limits.ingress.get() => {
                return NativeLaneAdmissionCheck::Retry;
            }
            NativeLaneInputRef::Decision(decision) => {
                if let Some(previous) = self.decisions.get(&id) {
                    if previous.manifest != decision.manifest
                        || previous.commit_qc.statement.value != decision.commit_qc.statement.value
                    {
                        return NativeLaneAdmissionCheck::Rejected(
                            "native instance already retains a different Commit value".into(),
                        );
                    }
                    return NativeLaneAdmissionCheck::Duplicate;
                }
                if self.decisions.len() == self.limits.process.instances.get()
                    || (local && self.ingress.len() == self.limits.ingress.get())
                {
                    return NativeLaneAdmissionCheck::Retry;
                }
            }
            _ => {}
        }
        NativeLaneAdmissionCheck::Ready { id, local }
    }

    fn insert_admitted(&mut self, id: HeightContextId, local: bool, input: NativeLaneInput) {
        match input {
            NativeLaneInput::Control(envelope) => self.ingress.push_back((id, envelope)),
            NativeLaneInput::Decision(decision) => {
                if local {
                    self.ingress.push_back((
                        id,
                        LaneMessageEnvelopeV1 {
                            version: LANE_MESSAGE_VERSION_V1,
                            message: LaneMessageV1::QuorumCertificate(decision.commit_qc.clone()),
                        },
                    ));
                }
                self.decisions.insert(id, decision);
            }
        }
    }

    /// Admit authenticated evidence for the original current instance.
    pub(crate) fn admit(
        &mut self,
        observed: &VerifiedLaneContexts,
        input: NativeLaneInput,
    ) -> NativeLaneAdmission {
        let checked = self.check_admission(observed, input.as_ref());
        let state = Arc::clone(&self.state);
        let _lease = state.consensus_publication_lease();
        if !observed.is_current(&state) {
            return NativeLaneAdmission::Retry(input);
        }
        match checked {
            NativeLaneAdmissionCheck::Ready { id, local } => self.insert_admitted(id, local, input),
            NativeLaneAdmissionCheck::Duplicate => {}
            NativeLaneAdmissionCheck::Retry => return NativeLaneAdmission::Retry(input),
            NativeLaneAdmissionCheck::Rejected(reason) => {
                return NativeLaneAdmission::Rejected { input, reason };
            }
        }
        NativeLaneAdmission::Accepted
    }

    /// Transfer one exact dequeued fair-ingress carrier without replacing its payload.
    pub(crate) fn admit_owned(
        &mut self,
        inbound: InboundBlockMessage,
    ) -> Result<NativeLaneOwnedAdmission> {
        let Some(ownership) = inbound.ingress_ownership() else {
            self.guard.close_admission_for_restart();
            return Err("native ingress lost its original fair ownership".into());
        };
        if !ownership.validate_exact()
            || !ownership.matches_message(inbound.message())
            || !ownership.matches_semantic_origin(inbound.sender())
            || !ownership.matches_reply_routes(inbound.reply_routes())
        {
            self.guard.close_admission_for_restart();
            return Err("native ingress changed its original fair ownership".into());
        }
        let Some(observed) = self.state.verified_lane_consensus_contexts()? else {
            return Ok(NativeLaneOwnedAdmission::Retry(inbound));
        };
        let input = match inbound.message() {
            BlockMessage::NativeLane(envelope) => NativeLaneInputRef::Control(envelope),
            BlockMessage::NativeLaneDecision(decision) => NativeLaneInputRef::Decision(decision),
            _ => {
                self.guard.close_admission_for_restart();
                return Err("native ingress consumer received another message family".into());
            }
        };
        let checked = self.check_admission(&observed, input);
        let state = Arc::clone(&self.state);
        let _lease = state.consensus_publication_lease();
        if !observed.is_current(&state) {
            return Ok(NativeLaneOwnedAdmission::Retry(inbound));
        }
        match checked {
            NativeLaneAdmissionCheck::Ready { id, local } => {
                let (message, _, _) = inbound.into_message_sender_and_reply_routes();
                let input = match message {
                    BlockMessage::NativeLane(envelope) => NativeLaneInput::Control(envelope),
                    BlockMessage::NativeLaneDecision(decision) => {
                        NativeLaneInput::Decision(*decision)
                    }
                    _ => {
                        self.guard.close_admission_for_restart();
                        return Err("native carrier changed family after admission".into());
                    }
                };
                self.insert_admitted(id, local, input);
            }
            NativeLaneAdmissionCheck::Duplicate => {}
            NativeLaneAdmissionCheck::Retry => return Ok(NativeLaneOwnedAdmission::Retry(inbound)),
            NativeLaneAdmissionCheck::Rejected(reason) => {
                return Ok(NativeLaneOwnedAdmission::Rejected { inbound, reason });
            }
        }
        Ok(NativeLaneOwnedAdmission::Accepted)
    }

    /// Require the original output barrier when a retained fair row is retried.
    pub(crate) fn matches_output_guard(&self, guard: &Arc<ConsensusOutputGuard>) -> bool {
        Arc::ptr_eq(&self.guard, guard)
    }

    /// One bounded fair turn: completion, opening, one instance's timer/control,
    /// one ingress occurrence, and one dispatch per independent physical class.
    /// Slow body work never owns the timeout/WAL queue or the control thread.
    pub(crate) fn poll(&mut self, observed: &VerifiedLaneContexts, now: Instant) -> Result<()> {
        if self.guard.restart_required() {
            return Err("native driver output requires restart".into());
        }
        if self.process.reconcile(observed) != LaneCurrentGate::Current {
            return Ok(());
        }
        let current = observed
            .contexts()
            .iter()
            .map(|lane| lane.instance_id())
            .collect::<BTreeSet<_>>();
        self.decisions.retain(|id, _| current.contains(id));
        if let Some(completed) = self
            .pool
            .try_completion()
            .map_err(|error| error.to_string())?
        {
            self.process
                .accept_completion(completed, observed)
                .map_err(|(error, _retained)| error.to_string())?;
        }
        let known = self.process.instance_ids().collect::<BTreeSet<_>>();
        if self.limits.voting_enabled
            && self.process.occupancy().instances < self.limits.process.instances.get()
            && let Some(lane) = observed.contexts().iter().find(|lane| {
                !known.contains(&lane.instance_id())
                    && lane
                        .frozen()
                        .committee
                        .iter()
                        .any(|peer| peer.public_key() == self.key.public_key())
            })
        {
            let required =
                super::v2_lane_frame_bounds::maximum_message_bytes(lane.frozen().committee.len())?;
            if required > self.limits.maximum_message_bytes.get() {
                return Err(format!(
                    "native control frame capacity {} is below required {required} before opening",
                    self.limits.maximum_message_bytes
                ));
            }
            self.process
                .reserve_opening(observed, lane, self.key.clone(), now)
                .map_err(|error| error.to_string())?;
        }
        let ids = self.process.instance_ids().collect::<Vec<_>>();
        let next = ids
            .iter()
            .copied()
            .find(|id| self.last_serviced.is_none_or(|last| *id > last))
            .or_else(|| ids.first().copied());
        if let Some(id) = next {
            self.last_serviced = Some(id);
            match self
                .process
                .settle_opening(id, observed)
                .map_err(|error| error.to_string())?
            {
                LaneProcessProgress::Failed(reason) => return Err(reason),
                _ => {}
            }
            if !current.contains(&id) {
                self.process
                    .prepare_closed_drain(id)
                    .map_err(|error| error.to_string())?;
            } else if self.process.is_productive(id) {
                self.process
                    .service_one(id, observed, now)
                    .map_err(|error| error.to_string())?;
                self.process
                    .poll_clock(id, observed, now)
                    .map_err(|error| error.to_string())?;
                self.process
                    .service_body_completion(id, observed)
                    .map_err(|error| error.to_string())?;
                self.process
                    .prepare_persistence(id)
                    .map_err(|error| error.to_string())?;
                self.process
                    .prepare_body(id, observed)
                    .map_err(|error| error.to_string())?;
                self.process
                    .flush_one(id, observed, &self.send)
                    .map_err(|error| error.to_string())?;
            }
        }
        if let Some((id, envelope)) = self.ingress.pop_front() {
            if current.contains(&id) {
                let retry = if self.process.is_productive(id) {
                    matches!(
                        self.process
                            .offer(id, observed, &envelope.message)
                            .map_err(|error| error.to_string())?,
                        LaneInputOutcome::Backpressured
                            | LaneInputOutcome::Gate(LaneCurrentGate::ObservationChanged)
                    )
                } else {
                    true
                };
                if retry {
                    self.ingress.push_back((id, envelope));
                }
            }
            // Only the complete authenticated set can retire an obsolete ingress
            // occurrence. This never retires the instance's Decision/Apply owner.
        }
        #[cfg(test)]
        if self.held_body.as_ref().is_some_and(|(id, _)| {
            self.process
                .has_queued_job_for_test(*id, LaneWorkerClass::Body)
        }) {
            let (id, before) = self.held_body.take().expect("one exact queued body hook");
            self.process
                .hold_next_completion_for_test(id, LaneWorkerClass::Body, before)
                .map_err(|error| error.to_string())?;
        }
        for class in [
            LaneWorkerClass::Opening,
            LaneWorkerClass::Wal,
            LaneWorkerClass::Body,
        ] {
            self.process
                .dispatch_one(&self.pool, class)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Hold the next real body completion before delivery, retaining its original result.
    #[cfg(test)]
    pub(crate) fn hold_next_body_completion_for_test(
        &mut self,
        id: HeightContextId,
        before: impl FnOnce() + Send + 'static,
    ) {
        assert!(self.held_body.is_none());
        self.held_body = Some((id, Box::new(before)));
    }

    /// Keep original real obligations while removing only test capacity headroom.
    #[cfg(test)]
    pub(crate) fn restrict_effect_capacity_to_retained_for_test(&mut self, id: HeightContextId) {
        self.process
            .restrict_effect_capacity_to_retained_for_test(id);
    }

    /// One production cleanup turn preserves original capacity until the existing
    /// physical worker returns. Round-robin selection never waits for cleanup.
    pub(crate) fn prepare_one_retirement(&mut self) -> Result<bool> {
        let id = self
            .process
            .instance_ids()
            .find(|id| self.last_retirement.is_none_or(|last| *id > last))
            .or_else(|| self.process.instance_ids().next());
        let Some(id) = id else {
            return Ok(false);
        };
        self.last_retirement = Some(id);
        self.process
            .prepare_retirement(id)
            .map_err(|error| error.to_string())
    }

    /// Deadline belongs to native instances, independently of global view changes.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.process.next_deadline()
    }

    /// Move one exact packet into its transport owner. Acceptance here is custody,
    /// not delivery. The transport must retain each unfinished fanout destination.
    pub(crate) fn take_outbound(&self) -> Result<Option<LaneOutbound>> {
        match self.receive.try_recv() {
            Ok(packet) => Ok(Some(packet)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err("native outbox disconnected".into()),
        }
    }

    /// Borrow actual local custody for recovery and exact publication settlement.
    pub(crate) fn process(&self) -> &LaneProcessOwner {
        &self.process
    }

    /// Rejoin an authenticated historical response to its original source owner.
    /// Failure retains that exact recovery requirement for retry. Success does
    /// not acknowledge transport custody or confer native body/Ready/Apply authority.
    pub(crate) fn complete_source_recovery(
        &mut self,
        id: HeightContextId,
        request: &super::v2_transport::AuthenticatedCertifiedBodyRequest,
        response: &super::v2_transport::AuthenticatedCertifiedBodyResponse,
    ) -> Result<()> {
        self.process
            .complete_source_recovery(id, request, response)
            .map_err(|error| error.to_string())
    }

    /// Transfer one retained diagnostic to the reporting consumer. It must not
    /// remain indefinitely in the bounded effect queue behind ordinary traffic.
    pub(crate) fn take_diagnostic(&mut self, id: HeightContextId) -> Option<core::Effect> {
        self.process.take_diagnostic(id)
    }

    /// Move one original retired operation/packet to its explicit consumer.
    /// Poll results are non-owning: every producer keeps these values in the
    /// original instance and under that instance's existing descriptor capacity.
    pub(crate) fn take_retirement(
        &mut self,
        id: HeightContextId,
    ) -> Option<super::v2_lane_instance::LaneRetirement> {
        self.process.take_retirement(id)
    }

    /// Transfer closed obligations explicitly; absence from the current set alone
    /// is never an ApplicationCompleted event or permission to discard this owner.
    pub(crate) fn take_closed(&mut self, id: HeightContextId) -> Option<LaneClosedInstance> {
        self.process.take_closed(id)
    }

    /// Capture bounded evidence for an off-control candidate worker. All actual
    /// reducer Decisions, body handles and Apply effects remain with this driver.
    pub(crate) fn capture_decisions(
        &self,
        observed: &VerifiedLaneContexts,
    ) -> Result<Option<NativeLaneDecisionHandoff>> {
        if !observed.is_current(&self.state) {
            return Ok(None);
        }
        let mut decisions = BTreeMap::new();
        for lane in observed.contexts() {
            let id = lane.instance_id();
            let local = self
                .process
                .instance(id)
                .map(|owner| owner.native_decision())
                .transpose()
                .map_err(|error| error.to_string())?
                .flatten();
            if let Some(decision) = local.or_else(|| self.decisions.get(&id).cloned()) {
                if decisions.len() == self.limits.process.instances.get() {
                    return Err("native Decision handoff exceeds its instance bound".into());
                }
                decisions.insert(id, decision);
            }
        }
        if !observed.is_current(&self.state) {
            return Ok(None);
        }
        Ok(Some(NativeLaneDecisionHandoff {
            state: Arc::clone(&self.state),
            decisions,
            recovered_sources: BTreeMap::new(),
        }))
    }

    /// Settle only the original local Apply using genuine global publication.
    /// Inclusion, rollover and a missing current opening cannot call this API.
    pub(crate) fn settle_published_apply(
        &mut self,
        id: HeightContextId,
        published: &crate::state::PublishedNativeApply<'_>,
    ) -> Result<Option<super::v2_lane_instance::LaneApplySettlement>> {
        self.process
            .settle_published_apply(id, published)
            .map_err(|error| error.to_string())
    }

    /// Hold the original published carrier until every matching local instance
    /// has settled its original Apply and completed physical retirement.
    pub(crate) fn settle_published_carrier(
        &mut self,
        published: &crate::state::PublishedNativeApply<'_>,
    ) -> Result<bool> {
        let mut complete = true;
        for id in published.instance_ids() {
            complete &= self
                .process
                .settle_and_retire_published(id, published)
                .map_err(|error| error.to_string())?;
        }
        Ok(complete)
    }

    /// One authenticated local Decision for relay to the current global roster.
    /// The original reducer Apply and Decision remain inside this driver.
    pub(crate) fn next_unrelayed_decision(
        &self,
        observed: &VerifiedLaneContexts,
        relayed: &std::collections::BTreeSet<HeightContextId>,
    ) -> Result<Option<LaneDecisionV1>> {
        if !observed.is_current(&self.state) {
            return Ok(None);
        }
        for lane in observed.contexts() {
            let id = lane.instance_id();
            if relayed.contains(&id) {
                continue;
            }
            if let Some(instance) = self.process.instance(id)
                && let Some(decision) = instance
                    .native_decision()
                    .map_err(|error| error.to_string())?
            {
                return Ok(Some(decision));
            }
        }
        Ok(None)
    }

    /// Stop physical admission; the returned join owner belongs on a blocking
    /// shutdown worker. Original process custody remains fail-stop on drop.
    pub(crate) fn shutdown(self) -> LanePhysicalShutdown {
        self.pool.shutdown()
    }
}

/// Bounded immutable candidate input evidence, safe to move to a worker. It does
/// not export a live voting context, mutable State or detached publication token.
pub(crate) struct NativeLaneDecisionHandoff {
    state: Arc<State>,
    decisions: BTreeMap<HeightContextId, LaneDecisionV1>,
    recovered_sources: BTreeMap<Hash, Arc<crate::state::VerifiedFirstLaneAdmittedInputV1>>,
}

/// Complete groups and exact waits retain canonical input order. The candidate
/// owner must service recovery requirements; this is not an empty-block fallback.
pub(crate) struct NativeLaneDecisionPreparation {
    pub(crate) groups: Vec<VerifiedLaneDecisionGroupV1>,
    pub(crate) waits: Vec<LaneDecisionGroupPreparationV1>,
}

impl NativeLaneDecisionHandoff {
    /// Retain authenticated source evidence; current route readiness is still rechecked.
    pub(crate) fn with_recovered_sources(
        mut self,
        recovered: BTreeMap<Hash, Arc<crate::state::VerifiedFirstLaneAdmittedInputV1>>,
    ) -> Self {
        self.recovered_sources = recovered;
        self
    }

    /// Match this immutable handoff to the original committed State owner.
    pub(crate) fn belongs_to(&self, state: &State) -> bool {
        std::ptr::eq(self.state.as_ref(), state)
    }

    /// Prepare candidate input on a worker while preserving original reducer
    /// Decisions and Apply effects. The proof retains the exact observed State;
    /// global assembly rechecks it under the publication lease before signing.
    pub(crate) fn prepare_candidate(&self) -> Result<NativeLaneCandidatePreparation> {
        let Some(observed) = self.state.verified_lane_consensus_contexts()? else {
            return Ok(NativeLaneCandidatePreparation {
                work: None,
                waits: vec![LaneDecisionGroupPreparationV1::ObservationChanged],
            });
        };
        let prepared = self.prepare_groups()?;
        let batch = (|| -> Result<_> {
            let Some(first) = prepared.groups.first() else {
                return Ok(None);
            };
            let mut batch = self
                .state
                .prepare_lane_decision_batch(std::slice::from_ref(first))
                .map_err(|error| error.to_string())?;
            // The protocol source cap precedes the actual carrier cap. A full
            // aggregate must not strand individually feasible decided groups.
            for group in &prepared.groups[1..] {
                batch.groups.push(group.to_wire());
                batch.validate_structure()?;
                if norito::encode_canonical(&batch)
                    .map_err(|error| error.to_string())?
                    .len()
                    > iroha_data_model::merge::MAX_MERGE_EXECUTION_BATCH_BYTES
                {
                    batch.groups.pop();
                    break;
                }
            }
            let deferred_groups = prepared.groups.len() - batch.groups.len();
            Ok(Some((batch, deferred_groups)))
        })();
        if !observed.is_current(&self.state) {
            return Ok(NativeLaneCandidatePreparation {
                work: None,
                waits: vec![LaneDecisionGroupPreparationV1::ObservationChanged],
            });
        }
        Ok(NativeLaneCandidatePreparation {
            work: batch?.map(|(batch, deferred_groups)| NativeLaneCandidateBatch {
                state: Arc::clone(&self.state),
                observed: Arc::new(observed),
                batch,
                deferred_groups,
            }),
            waits: prepared.waits,
        })
    }
    /// Reauthenticate the current set and complete first-carrier/input/RS16 join
    /// on the original State. Expensive canonical-body and crypto work stays off
    /// the runner control turn; a stale handoff cannot open a replacement slot.
    pub(crate) fn prepare_groups(&self) -> Result<NativeLaneDecisionPreparation> {
        let Some(observed) = self.state.verified_lane_consensus_contexts()? else {
            return Ok(NativeLaneDecisionPreparation {
                groups: Vec::new(),
                waits: vec![LaneDecisionGroupPreparationV1::ObservationChanged],
            });
        };
        let mut groups = Vec::new();
        let mut waits = Vec::new();
        let mut visited = BTreeSet::new();
        for lane in observed.contexts() {
            if !self.decisions.contains_key(&lane.instance_id())
                || !visited.insert(lane.frozen().admitted_binding_hash)
            {
                continue;
            }
            let source = if let Some(source) = self
                .recovered_sources
                .get(&lane.frozen().admitted_binding_hash)
            {
                Arc::clone(source)
            } else {
                match self.state.first_lane_admitted_input(&observed, lane)? {
                    FirstLaneAdmittedInputReadV1::Ready(source) => Arc::new(source),
                    FirstLaneAdmittedInputReadV1::CanonicalBodyRecoveryRequired(source) => {
                        waits.push(
                            LaneDecisionGroupPreparationV1::CanonicalBodyRecoveryRequired(source),
                        );
                        continue;
                    }
                    FirstLaneAdmittedInputReadV1::ObservationChanged => {
                        return Ok(NativeLaneDecisionPreparation {
                            groups: Vec::new(),
                            waits: vec![LaneDecisionGroupPreparationV1::ObservationChanged],
                        });
                    }
                    FirstLaneAdmittedInputReadV1::InstanceNotCurrent => {
                        waits.push(LaneDecisionGroupPreparationV1::InstanceNotCurrent);
                        continue;
                    }
                }
            };
            let decisions = self
                .decisions
                .values()
                .filter(|decision| {
                    decision.value().admitted_binding_hash == lane.frozen().admitted_binding_hash
                })
                .cloned()
                .collect::<Vec<_>>();
            match self
                .state
                .prepare_lane_decision_group(&observed, lane, &source, &decisions)?
            {
                LaneDecisionGroupPreparationV1::Ready(group) => groups.push(group),
                LaneDecisionGroupPreparationV1::ObservationChanged => {
                    return Ok(NativeLaneDecisionPreparation {
                        groups: Vec::new(),
                        waits: vec![LaneDecisionGroupPreparationV1::ObservationChanged],
                    });
                }
                wait => waits.push(wait),
            }
        }
        groups.sort_by_key(|group| group.body().payload().descriptor.admission_priority);
        if !observed.is_current(&self.state) {
            return Ok(NativeLaneDecisionPreparation {
                groups: Vec::new(),
                waits: vec![LaneDecisionGroupPreparationV1::ObservationChanged],
            });
        }
        Ok(NativeLaneDecisionPreparation { groups, waits })
    }
}

/// Complete immutable source preparation plus exact recoverable dependencies.
/// A partial group is never inserted into `work` or converted to ordinary input.
pub(crate) struct NativeLaneCandidatePreparation {
    pub(crate) work: Option<NativeLaneCandidateBatch>,
    pub(crate) waits: Vec<LaneDecisionGroupPreparationV1>,
}

/// Candidate-only proof minted after the complete first-carrier/Decision join.
/// It contains no execution result or authority to settle the native Apply owner.
#[derive(Clone)]
pub(crate) struct NativeLaneCandidateBatch {
    state: Arc<State>,
    observed: Arc<VerifiedLaneContexts>,
    batch: iroha_data_model::block::lane_decision_batch::LaneDecisionBatchV1,
    deferred_groups: usize,
}

impl std::fmt::Debug for NativeLaneCandidateBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeLaneCandidateBatch")
            .field("batch", &self.batch)
            .finish_non_exhaustive()
    }
}

impl NativeLaneCandidateBatch {
    pub(crate) fn batch(
        &self,
    ) -> &iroha_data_model::block::lane_decision_batch::LaneDecisionBatchV1 {
        &self.batch
    }

    pub(crate) fn deferred_groups(&self) -> usize {
        self.deferred_groups
    }

    pub(crate) fn is_current(
        &self,
        state: &State,
        context: &iroha_data_model::block::consensus_v2::HeightContext,
    ) -> bool {
        std::ptr::eq(self.state.as_ref(), state)
            && self.observed.is_current(state)
            && context.network_id == *state.network_id_ref()
            && self.batch.base_state_height == self.observed.carrier_height()
            && self.batch.base_state_height.checked_add(1) == Some(context.height)
    }

    pub(crate) fn retain_prefix(&mut self, count: usize) {
        self.batch.groups.truncate(count);
    }
}

impl super::v2_candidate::CandidateWorkProvider for &NativeLaneCandidateBatch {
    fn prepare(
        &mut self,
        context: &iroha_data_model::block::consensus_v2::HeightContext,
        _view: u64,
        candidates: &[super::v2_candidate::CandidateDescriptor<'_>],
    ) -> std::result::Result<
        super::v2_candidate::PreparedCandidateWork,
        super::v2_candidate::CandidateWorkError,
    > {
        use super::v2_candidate::{
            CandidateWorkDeferral, CandidateWorkError, CandidateWorkUnavailable,
            PreparedCandidateWork,
        };
        if !self.is_current(&self.state, context) {
            return Err(CandidateWorkError::Deferred(
                CandidateWorkDeferral::NativeLaneSource,
            ));
        }
        if !candidates.is_empty() {
            return Err(CandidateWorkUnavailable::new(
                (0..candidates.len()).collect(),
                "native Decisions are the sole economic form in this candidate",
            )
            .into());
        }
        Ok(PreparedCandidateWork {
            native_lane_decisions: Some((*self).clone()),
            ..PreparedCandidateWork::default()
        })
    }
}

/// Exercise the original bounded physical ingress path without opening the
/// production Native entrypoint before the complete signer cutover.
#[cfg(test)]
pub(crate) fn native_driver_owned_ingress_for_test(
    message: BlockMessage,
    sender: iroha_model_base::peer::PeerId,
) -> InboundBlockMessage {
    let ingress = super::FairV2Ingress::new(8, 8 * 1_048_576, 8 * 1_048_576, 0, 0);
    ingress.configure_roster([sender.clone()]).unwrap();
    ingress.open().unwrap();
    ingress
        .try_push_owned_at(
            InboundBlockMessage::from_authenticated_peer(message, sender),
            Instant::now(),
        )
        .unwrap();
    let inbound = ingress.try_recv_if(|_| true).unwrap();
    ingress.close();
    ingress.ensure_closed_drained_cut().unwrap();
    inbound
}
