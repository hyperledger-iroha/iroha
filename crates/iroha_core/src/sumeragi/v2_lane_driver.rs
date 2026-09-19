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

impl NativeLaneInput {
    fn instance(&self) -> HeightContextId {
        let hash = match self {
            Self::Control(envelope) => message_instance(&envelope.message),
            Self::Decision(decision) => decision.value().instance_id,
        };
        HeightContextId(HashOf::from_untyped_unchecked(hash))
    }
}

/// Admission never discards a full-queue or stale-observation input. The caller
/// retains that exact occurrence until its transport owner can retry or retire it.
#[derive(Debug)]
pub(crate) enum NativeLaneAdmission {
    Accepted,
    Retry(NativeLaneInput),
    Rejected {
        input: NativeLaneInput,
        reason: String,
    },
}

fn message_instance(message: &LaneMessageV1) -> Hash {
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
        })
    }

    /// Admit only bounded, cryptographically authenticated evidence for an exact
    /// current instance. Bad peer input does not poison the local output guard.
    pub(crate) fn admit(
        &mut self,
        observed: &VerifiedLaneContexts,
        input: NativeLaneInput,
    ) -> NativeLaneAdmission {
        if !observed.is_current(&self.state) {
            return NativeLaneAdmission::Retry(input);
        }
        let id = input.instance();
        let Some(lane) = observed
            .contexts()
            .iter()
            .find(|lane| lane.instance_id() == id)
        else {
            return NativeLaneAdmission::Rejected {
                input,
                reason: "native ingress has no exact current opening".into(),
            };
        };
        let local = lane
            .frozen()
            .committee
            .iter()
            .any(|peer| peer.public_key() == self.key.public_key());
        let authenticate = || -> Result<()> {
            let bytes = match &input {
                NativeLaneInput::Control(envelope) => {
                    if !local {
                        return Err("native control targets a committee without this signer".into());
                    }
                    if envelope.version != LANE_MESSAGE_VERSION_V1 {
                        return Err("unsupported native lane envelope revision".into());
                    }
                    envelope
                        .message
                        .validate_shape(lane.frozen().committee.len())
                        .map_err(|error| error.to_string())?;
                    // Authentication does not require a payload or an open WAL.
                    // This tag is only the read-only event wrapper; the actual
                    // reducer chooses its own current tag when consuming ingress.
                    let context = lane.reducer_context();
                    let tag = core::EventTag::new(context.height(), 0, core::Generation::INITIAL);
                    LaneAuthenticator::new(lane)
                        .event(&envelope.message, tag)
                        .map_err(|error| error.to_string())?;
                    norito::encode_canonical(envelope).map_err(|error| error.to_string())?
                }
                NativeLaneInput::Decision(decision) => {
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
            return NativeLaneAdmission::Rejected { input, reason };
        }
        let _lease = self.state.consensus_publication_lease();
        if !observed.is_current(&self.state) {
            return NativeLaneAdmission::Retry(input);
        }
        match input {
            NativeLaneInput::Control(envelope) => {
                if self.ingress.len() == self.limits.ingress.get() {
                    return NativeLaneAdmission::Retry(NativeLaneInput::Control(envelope));
                }
                self.ingress.push_back((id, envelope));
            }
            NativeLaneInput::Decision(decision) => {
                if let Some(previous) = self.decisions.get(&id) {
                    if previous.manifest != decision.manifest
                        || previous.commit_qc.statement.value != decision.commit_qc.statement.value
                    {
                        return NativeLaneAdmission::Rejected {
                            input: NativeLaneInput::Decision(decision),
                            reason: "native instance already retains a different Commit value"
                                .into(),
                        };
                    }
                    // A different valid exact quorum for the same immutable value
                    // cannot replace the already retained evidence or consume space.
                    return NativeLaneAdmission::Accepted;
                }
                if self.decisions.len() == self.limits.process.instances.get()
                    || (local && self.ingress.len() == self.limits.ingress.get())
                {
                    return NativeLaneAdmission::Retry(NativeLaneInput::Decision(decision));
                }
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
        NativeLaneAdmission::Accepted
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
        if self.process.occupancy().instances < self.limits.process.instances.get()
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

    /// Borrow actual local custody for recovery and exact future Apply settlement.
    pub(crate) fn process(&self) -> &LaneProcessOwner {
        &self.process
    }

    /// Transfer one retained diagnostic to the reporting consumer. It must not
    /// remain indefinitely in the bounded effect queue behind ordinary traffic.
    pub(crate) fn take_diagnostic(&mut self, id: HeightContextId) -> Option<core::Effect> {
        self.process.take_diagnostic(id)
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
        }))
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
}

/// Complete groups and exact waits retain canonical input order. The candidate
/// owner must service recovery requirements; this is not an empty-block fallback.
pub(crate) struct NativeLaneDecisionPreparation {
    pub(crate) groups: Vec<VerifiedLaneDecisionGroupV1>,
    pub(crate) waits: Vec<LaneDecisionGroupPreparationV1>,
}

impl NativeLaneDecisionHandoff {
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
            let source = match self.state.first_lane_admitted_input(&observed, lane)? {
                FirstLaneAdmittedInputReadV1::Ready(source) => source,
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
