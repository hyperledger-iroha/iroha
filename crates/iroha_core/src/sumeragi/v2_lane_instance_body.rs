//! Move-owned native input jobs and tagged completions for the same lane reducer.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use iroha_crypto::Signature;
use iroha_data_model::block::lane_consensus::{
    LaneDecisionV1, LaneManifestV1, LaneMessageV1, LaneProposalBodyV1, LaneProposalV1,
    LaneValueRefV1,
};

use super::{
    LaneCurrentGate, LaneInputOutcome, LaneInstance, LaneInstanceError, LaneService,
    LaneStepReceipt, Witnesses, bad, reducer,
};
use crate::{
    state::{
        AuthenticatedLaneAdmittedInputSourceV1, FirstLaneAdmittedInputReadV1,
        LaneInputBodyPreparationV1, LaneInputDependencyV1, State, VerifiedFirstLaneAdmittedInputV1,
        VerifiedLaneContext, VerifiedLaneContexts, VerifiedLaneInputBodyV1,
    },
    sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_lane_body_store::{DurableLaneBodyRead, LaneBodyStore},
        v2_lane_payload::{encode_lane_input, verify_lane_input_manifest},
        v2_lane_wire::{LaneAuthenticator, LaneNativeWitnesses, LaneWalRecordV1},
        v2_transport::{AuthenticatedCertifiedBodyRequest, AuthenticatedCertifiedBodyResponse},
    },
};

type Result<T> = std::result::Result<T, LaneInstanceError>;

/// A retained source/dependency owner, not an unowned generic retry.
#[derive(Debug)]
pub(crate) enum LaneBodyWait {
    /// A worker still owns this instance's single physical body handle.
    WorkerInFlight,
    /// Actual completed body work waits behind an already-issued control operation.
    ControlCompletion,
    /// Read a coherent authenticated current set before launching work.
    CurrentSet(LaneCurrentGate),
    /// Existing global certified-body recovery must settle the retained source.
    FirstCarrierRecovery,
    /// These exact earlier groups own the affected routes.
    EarlierHeads(Vec<LaneInputDependencyV1>),
    /// No body operation or local proposal is currently issued/eligible.
    NoWork,
}

/// One bounded handoff to the existing worker executor; no synchronous runner I/O.
pub(crate) enum LaneBodyLaunch {
    Job(LaneBodyJob),
    Wait(LaneBodyWait),
}

/// Completion dispositions retain the exact source or retired operation.
#[derive(Debug)]
pub(crate) enum LaneBodyProgress {
    /// A corresponding reducer-issued tagged operation completed.
    Stepped(LaneStepReceipt),
    /// Physical validation completed for an already durable body-sign intent.
    SignReady,
    /// Physical custody returned after the core tag changed or the instance closed.
    Retired {
        effect: Option<reducer::Effect>,
        proposal: Option<LaneProposalV1>,
    },
    /// Exact source/earlier dependency retained; clocks and certificates still run.
    Waiting(LaneBodyWait),
    /// A signed proposal failed deterministic input checking before core admission.
    RejectedProposal {
        proposal: LaneProposalV1,
        reason: String,
    },
}

#[derive(Clone)]
enum Purpose {
    Local {
        tag: reducer::EventTag,
    },
    Ingress {
        tag: reducer::EventTag,
        proposal: LaneProposalV1,
    },
    Issued(reducer::Effect),
}
impl Purpose {
    fn tag(&self) -> reducer::EventTag {
        match self {
            Self::Local { tag } | Self::Ingress { tag, .. } => *tag,
            Self::Issued(
                reducer::Effect::FetchBody { tag, .. }
                | reducer::Effect::StoreBody { tag, .. }
                | reducer::Effect::ValidateBody { tag, .. }
                | reducer::Effect::Sign { tag, .. },
            ) => *tag,
            _ => unreachable!("only a body operation is launched"),
        }
    }
    fn effect(&self) -> Option<&reducer::Effect> {
        if let Self::Issued(effect) = self {
            Some(effect)
        } else {
            None
        }
    }
}

struct Ticket {
    identity: Arc<()>,
    purpose: Purpose,
}

/// Retained immutable source/native witnesses and the actual one-job owner.
#[derive(Default)]
pub(super) struct BodyCustody {
    ticket: Option<Ticket>,
    completed: Option<(Purpose, WorkResult)>,
    source: Option<VerifiedFirstLaneAdmittedInputV1>,
    recovery: Option<AuthenticatedLaneAdmittedInputSourceV1>,
    prepared: Option<Arc<VerifiedLaneInputBodyV1>>,
    validated: BTreeMap<reducer::Subject, Arc<DurableLaneBodyRead>>,
    manifests: BTreeMap<reducer::Subject, LaneManifestV1>,
    values: BTreeMap<reducer::Subject, LaneValueRefV1>,
    proposals: BTreeMap<(u64, reducer::Subject), LaneProposalBodyV1>,
    ingress: Option<LaneProposalV1>,
    dependencies: Vec<LaneInputDependencyV1>,
}

/// The real store moves to one worker. Dropping the job before returning its
/// completion closes process output; another body owner cannot be minted.
#[must_use]
pub(crate) struct LaneBodyJob {
    identity: Arc<()>,
    purpose: Purpose,
    lane: VerifiedLaneContext,
    source: Option<VerifiedFirstLaneAdmittedInputV1>,
    target: Option<LaneValueRefV1>,
    manifest: Option<LaneManifestV1>,
    store: Option<LaneBodyStore>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}

struct Prepared {
    body: Arc<VerifiedLaneInputBodyV1>,
    manifest: LaneManifestV1,
}
enum Materialized {
    Prepared(Prepared),
    Stored(Prepared, DurableLaneBodyRead),
    Validated(Prepared, Arc<DurableLaneBodyRead>),
}
enum WorkResult {
    Body(Materialized),
    Recovery(AuthenticatedLaneAdmittedInputSourceV1),
    Earlier(VerifiedFirstLaneAdmittedInputV1, Vec<LaneInputDependencyV1>),
    Gate(LaneCurrentGate),
    RejectedProposal(String),
}

/// Private worker result; exact ticket identity and physical handle never escape.
#[must_use]
pub(crate) struct LaneBodyCompletion {
    identity: Arc<()>,
    store: Option<LaneBodyStore>,
    result: Option<std::result::Result<WorkResult, String>>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl std::fmt::Debug for LaneBodyCompletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaneBodyCompletion").finish_non_exhaustive()
    }
}
impl Drop for LaneBodyJob {
    fn drop(&mut self) {
        if self.armed {
            self.guard.close_admission_for_restart();
        }
    }
}
impl Drop for LaneBodyCompletion {
    fn drop(&mut self) {
        if self.armed {
            self.guard.close_admission_for_restart();
        }
    }
}

impl LaneBodyJob {
    /// Run on an existing bounded worker, with no caller-held State/MV lease.
    /// Every ordinary result returns the physical store, including errors/waits.
    pub(crate) fn run(mut self, state: &State) -> LaneBodyCompletion {
        let result = self.execute(state);
        self.armed = false;
        LaneBodyCompletion {
            identity: Arc::clone(&self.identity),
            store: self.store.take(),
            result: Some(result),
            guard: Arc::clone(&self.guard),
            armed: true,
        }
    }
    fn execute(&mut self, state: &State) -> std::result::Result<WorkResult, String> {
        let Some(observed) = state.verified_lane_consensus_contexts()? else {
            return Ok(WorkResult::Gate(LaneCurrentGate::ObservationChanged));
        };
        let gate = LaneInstance::gate_for(&self.lane, state, &observed);
        if gate != LaneCurrentGate::Current {
            return Ok(WorkResult::Gate(gate));
        }
        let source = match &self.source {
            Some(source) => source.clone(),
            None => match state.first_lane_admitted_input(&observed, &self.lane)? {
                FirstLaneAdmittedInputReadV1::Ready(source) => source,
                FirstLaneAdmittedInputReadV1::CanonicalBodyRecoveryRequired(source) => {
                    return Ok(WorkResult::Recovery(source));
                }
                FirstLaneAdmittedInputReadV1::ObservationChanged => {
                    return Ok(WorkResult::Gate(LaneCurrentGate::ObservationChanged));
                }
                FirstLaneAdmittedInputReadV1::InstanceNotCurrent => {
                    return Ok(WorkResult::Gate(LaneCurrentGate::InstanceClosed));
                }
            },
        };
        let body = match state.prepare_lane_input_body(&observed, &self.lane, &source)? {
            LaneInputBodyPreparationV1::Ready(body) => Arc::new(body),
            LaneInputBodyPreparationV1::BlockedByEarlierInputs(deps) => {
                return Ok(WorkResult::Earlier(source, deps));
            }
            LaneInputBodyPreparationV1::ObservationChanged => {
                return Ok(WorkResult::Gate(LaneCurrentGate::ObservationChanged));
            }
            LaneInputBodyPreparationV1::InstanceNotCurrent => {
                return Ok(WorkResult::Gate(LaneCurrentGate::InstanceClosed));
            }
        };
        let origin = self
            .target
            .map_or(self.purpose.tag().view(), |value| value.origin_view);
        let manifest = *encode_lane_input(&self.lane, &body, origin)?.manifest();
        if self.target.is_some_and(|target| target != manifest.value)
            || self.manifest.is_some_and(|expected| expected != manifest)
        {
            let reason = "signed native proposal/value differs from exact admitted input and all-route codeword".to_owned();
            if matches!(self.purpose, Purpose::Ingress { .. }) {
                return Ok(WorkResult::RejectedProposal(reason));
            }
            return Err(reason);
        }
        let prepared = Prepared { body, manifest };
        let store = self.store.as_mut().expect("worker owns actual store");
        let result = match &self.purpose {
            Purpose::Ingress { .. } | Purpose::Issued(reducer::Effect::FetchBody { .. }) => {
                Materialized::Prepared(prepared)
            }
            Purpose::Issued(reducer::Effect::StoreBody { .. }) => {
                let read = store
                    .persist(&prepared.body, &manifest)
                    .map_err(|error| error.to_string())?;
                Materialized::Stored(prepared, read)
            }
            Purpose::Issued(reducer::Effect::ValidateBody { .. }) => {
                let read = store
                    .read_for_manifest(&manifest)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "previously stored native body disappeared".to_owned())?;
                store
                    .validate_receipt(read.receipt())
                    .map_err(|error| error.to_string())?;
                verify_lane_input_manifest(
                    &self.lane,
                    &prepared.body,
                    &manifest,
                    read.canonical_bytes(),
                )?;
                Materialized::Validated(prepared, Arc::new(read))
            }
            Purpose::Local { .. } | Purpose::Issued(reducer::Effect::Sign { .. }) => {
                let read = store
                    .persist(&prepared.body, &manifest)
                    .map_err(|error| error.to_string())?;
                store
                    .validate_receipt(read.receipt())
                    .map_err(|error| error.to_string())?;
                verify_lane_input_manifest(
                    &self.lane,
                    &prepared.body,
                    &manifest,
                    read.canonical_bytes(),
                )?;
                Materialized::Validated(prepared, Arc::new(read))
            }
            _ => return Err("worker received a non-body effect".into()),
        };
        // A stale observation can retain immutable bytes/receipts, but the owner
        // must repeat current all-route eligibility before any tagged completion.
        Ok(WorkResult::Body(result))
    }
}

impl BodyCustody {
    pub(super) fn worker_in_flight(&self) -> bool {
        self.ticket.is_some()
    }
    pub(super) fn retained_job_count(&self) -> usize {
        usize::from(self.ticket.is_some()) + usize::from(self.completed.is_some())
    }
    pub(super) fn in_flight_matches(&self, effect: &reducer::Effect) -> bool {
        self.ticket
            .as_ref()
            .and_then(|ticket| ticket.purpose.effect())
            == Some(effect)
            || self
                .completed
                .as_ref()
                .and_then(|(purpose, _)| purpose.effect())
                == Some(effect)
    }
    pub(super) fn original_proposal(
        &self,
        message: &reducer::ConsensusMessageV2,
    ) -> Option<&LaneProposalBodyV1> {
        let reducer::ConsensusMessageV2::Proposal(signed) = message else {
            return None;
        };
        let proposal = signed.proposal();
        self.proposals
            .get(&(proposal.round().view(), proposal.manifest().subject()))
    }
    pub(super) fn add_witnesses(&self, witnesses: &mut Witnesses) -> Result<()> {
        for value in self.values.values() {
            witnesses.retain_value(*value)?;
        }
        for manifest in self.manifests.values() {
            witnesses.retain_value(manifest.value)?;
            witnesses
                .manifests
                .insert(subject(manifest.value)?, *manifest);
        }
        Ok(())
    }
    pub(super) fn observe(&mut self, message: &LaneMessageV1) -> Result<()> {
        match message {
            LaneMessageV1::Proposal(proposal) => {
                let value = proposal.body.manifest.value;
                let subject = subject(value)?;
                self.values.insert(subject, value);
                self.manifests.insert(subject, proposal.body.manifest);
                self.proposals
                    .entry((proposal.body.round.voting_view, subject))
                    .or_insert_with(|| proposal.body.clone());
            }
            LaneMessageV1::Vote(vote) => {
                self.values
                    .insert(subject(vote.statement.value)?, vote.statement.value);
            }
            LaneMessageV1::QuorumCertificate(qc) => {
                self.values
                    .insert(subject(qc.statement.value)?, qc.statement.value);
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn prune(&mut self, reducer: &reducer::Reducer, held: &[super::HeldEffect]) {
        let mut live = reducer
            .retained_body_references()
            .map(|(_, subject)| subject)
            .collect::<BTreeSet<_>>();
        live.extend(
            reducer
                .vote_pool_snapshots()
                .into_iter()
                .map(|pool| pool.subject),
        );
        for held in held {
            match &held.effect {
                reducer::Effect::FetchBody { subject, .. }
                | reducer::Effect::StoreBody { subject, .. }
                | reducer::Effect::ValidateBody { subject, .. }
                | reducer::Effect::Apply { subject, .. } => {
                    live.insert(*subject);
                }
                reducer::Effect::ReportInvalidCertifiedBody { subject, .. } => {
                    live.insert(*subject);
                }
                reducer::Effect::ReportEquivocation { evidence, .. } => match evidence {
                    reducer::EquivocationEvidence::Proposal { first, second } => {
                        live.insert(first.proposal().manifest().subject());
                        live.insert(second.proposal().manifest().subject());
                    }
                    reducer::EquivocationEvidence::Vote { first, second } => {
                        live.insert(first.vote().subject());
                        live.insert(second.vote().subject());
                    }
                    reducer::EquivocationEvidence::Timeout { .. } => {}
                },
                reducer::Effect::Sign { message, .. } => {
                    if let Some(subject) = sign_subject(message) {
                        live.insert(subject);
                    }
                }
                _ => {}
            }
        }
        if let Some(ticket) = &self.ticket {
            if let Some(effect) = ticket.purpose.effect() {
                if let Some(subject) = effect_subject(effect) {
                    live.insert(subject);
                }
            }
        }
        if let Some((purpose, _)) = &self.completed {
            if let Some(subject) = purpose.effect().and_then(effect_subject) {
                live.insert(subject);
            }
        }
        self.values.retain(|subject, _| live.contains(subject));
        self.manifests.retain(|subject, _| live.contains(subject));
        self.validated.retain(|subject, _| live.contains(subject));
        self.proposals
            .retain(|(_, subject), _| live.contains(subject));
    }
}

pub(super) fn subject(value: LaneValueRefV1) -> Result<reducer::Subject> {
    Ok(reducer::Subject::new(
        value.subject_hash().map_err(bad)?.into(),
    ))
}
fn sign_subject(message: &reducer::SignableMessage) -> Option<reducer::Subject> {
    match message {
        reducer::SignableMessage::Proposal(proposal) => Some(proposal.manifest().subject()),
        reducer::SignableMessage::Vote(vote) => Some(vote.subject()),
        reducer::SignableMessage::TimeoutVote(_) => None,
    }
}
fn effect_subject(effect: &reducer::Effect) -> Option<reducer::Subject> {
    match effect {
        reducer::Effect::FetchBody { subject, .. }
        | reducer::Effect::StoreBody { subject, .. }
        | reducer::Effect::ValidateBody { subject, .. }
        | reducer::Effect::Apply { subject, .. } => Some(*subject),
        reducer::Effect::Sign { message, .. } => sign_subject(message),
        _ => None,
    }
}

impl LaneInstance {
    /// Original first-carrier proof requiring the existing global body owner.
    /// It remains held until an exact authenticated response settles it.
    pub(crate) fn source_recovery_requirement(
        &self,
    ) -> Option<&AuthenticatedLaneAdmittedInputSourceV1> {
        self.body.recovery.as_ref()
    }

    /// Settle the retained source via existing request/response authentication.
    /// Current lane/route eligibility is separately rechecked before any effect.
    pub(crate) fn complete_source_recovery(
        &mut self,
        request: &AuthenticatedCertifiedBodyRequest,
        response: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<()> {
        self.check_open()?;
        let source = self
            .body
            .recovery
            .as_ref()
            .ok_or_else(|| bad("no exact first-carrier recovery owner"))?
            .complete_from_authenticated_response(request, response)
            .map_err(bad)?;
        self.body.source = Some(source);
        self.body.recovery = None;
        Ok(())
    }

    /// Exact immutable dependency names from the last worker wait. These do not
    /// decide eligibility; every later job/effect rechecks current State.
    pub(crate) fn blocked_route_dependencies(&self) -> &[LaneInputDependencyV1] {
        &self.body.dependencies
    }

    /// Borrow a still-owned native proposal that has not entered core yet.
    pub(crate) fn deferred_proposal(&self) -> Option<&LaneProposalV1> {
        self.body.ingress.as_ref()
    }

    pub(super) fn defer_proposal(&mut self, proposal: &LaneProposalV1) -> LaneInputOutcome {
        if self
            .body
            .ingress
            .as_ref()
            .is_some_and(|existing| existing != proposal)
        {
            return LaneInputOutcome::Backpressured;
        }
        self.body.ingress = Some(proposal.clone());
        LaneInputOutcome::BodyPreparationQueued
    }

    /// Transfer one exact body operation and the real physical handle to a worker.
    /// Existing control service remains independent while this job is in flight.
    pub(crate) fn take_body_job(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
    ) -> Result<LaneBodyLaunch> {
        self.check_open()?;
        let gate = self.current_gate(state, observed);
        if gate != LaneCurrentGate::Current {
            return Ok(LaneBodyLaunch::Wait(LaneBodyWait::CurrentSet(gate)));
        }
        if self.body.completed.is_some() {
            return Ok(LaneBodyLaunch::Wait(LaneBodyWait::ControlCompletion));
        }
        if self.body.ticket.is_some() {
            return Ok(LaneBodyLaunch::Wait(LaneBodyWait::WorkerInFlight));
        }
        if self.body.recovery.is_some() {
            return Ok(LaneBodyLaunch::Wait(LaneBodyWait::FirstCarrierRecovery));
        }
        if self.completion.is_some() || self.clock.tag != self.tag() || !self.reserve_ingress() {
            return Ok(LaneBodyLaunch::Wait(LaneBodyWait::NoWork));
        }
        let mut remove = None;
        let purpose = if let Some(index) = self.held.iter().position(|held| {
            matches!(held.effect, reducer::Effect::FetchBody { .. } | reducer::Effect::StoreBody { .. } | reducer::Effect::ValidateBody { .. })
            || matches!(&held.effect, reducer::Effect::Sign { message, .. } if sign_subject(message).is_some_and(|subject| !self.body.validated.contains_key(&subject)))
        }) {
            remove = Some(index);
            Purpose::Issued(self.held[index].effect.clone())
        } else if let Some(proposal) = &self.body.ingress {
            Purpose::Ingress { tag: self.tag(), proposal: proposal.clone() }
        } else {
            let round = reducer::Round::new(self.tag().height(), self.tag().view());
            if self.reducer.local_validator() != Some(self.reducer.context().leader(self.tag().view()))
                || self.reducer.durable_state().decision().is_some()
                || self.reducer.durable_state().timeout_intent(round).is_some()
                || self.reducer.durable_state().proposal_intent(round).is_some()
                || self.held.iter().any(|held| matches!(held.effect, reducer::Effect::Persist { .. } | reducer::Effect::Sign { .. })) {
                return Ok(LaneBodyLaunch::Wait(LaneBodyWait::NoWork));
            }
            Purpose::Local { tag: self.tag() }
        };
        let witnesses = self.witnesses(None)?;
        let target = match &purpose {
            Purpose::Ingress { proposal, .. } => Some(proposal.body.manifest.value),
            Purpose::Issued(effect) => Some(
                *LaneNativeWitnesses::value(
                    &witnesses,
                    effect_subject(effect)
                        .ok_or_else(|| bad("body effect has no exact subject"))?,
                )
                .ok_or_else(|| bad("body effect lost its native value witness"))?,
            ),
            Purpose::Local { .. } => self
                .reducer
                .durable_state()
                .last_timeout()
                .and_then(|tc| tc.highest_prepare())
                .or_else(|| self.reducer.durable_state().locked())
                .map(|qc| {
                    LaneNativeWitnesses::value(&witnesses, qc.subject())
                        .copied()
                        .ok_or_else(|| bad("protected local proposal lost native value"))
                })
                .transpose()?,
        };
        let manifest = match &purpose {
            Purpose::Ingress { proposal, .. } => Some(proposal.body.manifest),
            _ => target
                .and_then(|value| subject(value).ok())
                .and_then(|subject| witnesses.manifest(subject).copied()),
        };
        let store = self
            .body_store
            .take()
            .ok_or_else(|| bad("body physical handle has no matching worker owner"))?;
        if let Some(index) = remove {
            self.held.remove(index);
        }
        let identity = Arc::new(());
        self.body.ticket = Some(Ticket {
            identity: Arc::clone(&identity),
            purpose: purpose.clone(),
        });
        self.body.dependencies.clear();
        Ok(LaneBodyLaunch::Job(LaneBodyJob {
            identity,
            purpose,
            lane: self.verified.clone(),
            source: self.body.source.clone(),
            target,
            manifest,
            store: Some(store),
            guard: Arc::clone(&self.output_guard),
            armed: true,
        }))
    }

    /// Restore physical custody first; only a fresh exact gate can consume a tagged result.
    /// A foreign result is returned intact so its actual owner can settle it.
    pub(crate) fn finish_body_job(
        &mut self,
        mut completion: LaneBodyCompletion,
        state: &State,
        observed: &VerifiedLaneContexts,
    ) -> std::result::Result<LaneBodyProgress, (LaneInstanceError, LaneBodyCompletion)> {
        if self
            .body
            .ticket
            .as_ref()
            .is_none_or(|ticket| !Arc::ptr_eq(&ticket.identity, &completion.identity))
            || self.body_store.is_some()
        {
            return Err((
                bad("body completion does not match the exact physical job owner"),
                completion,
            ));
        }
        let ticket = self.body.ticket.take().expect("matched ticket");
        self.body_store = completion.store.take();
        completion.armed = false;
        let result = completion.result.take().expect("single worker result");
        let outcome = match result {
            Ok(result) => {
                self.body.completed = Some((ticket.purpose, result));
                self.service_body_completion(state, observed)
            }
            Err(error) => Err(bad(error)),
        };
        match outcome {
            Ok(result) => Ok(result),
            Err(error) => {
                self.failed = true;
                self.output_guard.close_admission_for_restart();
                Err((error, completion))
            }
        }
    }

    fn restore_purpose(&mut self, purpose: Purpose) {
        match purpose {
            Purpose::Issued(effect) => {
                if !self.held.iter().any(|held| held.effect == effect) {
                    self.held.push(super::HeldEffect {
                        effect,
                        native: None,
                        packet: None,
                    });
                }
            }
            Purpose::Ingress { proposal, .. } => self.body.ingress = Some(proposal),
            Purpose::Local { .. } => {} // No core operation has been issued yet.
        }
    }

    /// Settle actual completed work after outstanding control custody is serviced.
    /// The physical handle has already returned; no worker retry or extra disk I/O.
    pub(crate) fn service_body_completion(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
    ) -> Result<LaneBodyProgress> {
        self.check_open()?;
        if self.completion.is_some() || self.clock.tag != self.tag() || !self.reserve_ingress() {
            return Ok(LaneBodyProgress::Waiting(LaneBodyWait::ControlCompletion));
        }
        let Some((purpose, result)) = self.body.completed.take() else {
            return Ok(LaneBodyProgress::Waiting(LaneBodyWait::NoWork));
        };
        let result = self.accept_body_result(purpose, result, state, observed);
        if result.is_err() {
            self.failed = true;
            self.output_guard.close_admission_for_restart();
        }
        result
    }

    fn accept_body_result(
        &mut self,
        purpose: Purpose,
        result: WorkResult,
        state: &State,
        observed: &VerifiedLaneContexts,
    ) -> Result<LaneBodyProgress> {
        let gate = self.current_gate(state, observed);
        if purpose.tag() != self.tag() || gate == LaneCurrentGate::InstanceClosed {
            let (effect, proposal) = match purpose {
                Purpose::Issued(effect) => (Some(effect), None),
                Purpose::Ingress { proposal, .. } => {
                    self.body.ingress = None;
                    (None, Some(proposal))
                }
                Purpose::Local { .. } => (None, None),
            };
            return Ok(LaneBodyProgress::Retired { effect, proposal });
        }
        match result {
            WorkResult::Recovery(source) => {
                self.body.recovery = Some(source);
                self.restore_purpose(purpose);
                Ok(LaneBodyProgress::Waiting(
                    LaneBodyWait::FirstCarrierRecovery,
                ))
            }
            WorkResult::Earlier(source, deps) => {
                self.body.source = Some(source);
                self.body.dependencies = deps.clone();
                self.restore_purpose(purpose);
                Ok(LaneBodyProgress::Waiting(LaneBodyWait::EarlierHeads(deps)))
            }
            WorkResult::Gate(gate) => {
                self.restore_purpose(purpose);
                Ok(LaneBodyProgress::Waiting(LaneBodyWait::CurrentSet(gate)))
            }
            WorkResult::RejectedProposal(reason) => {
                let Purpose::Ingress { proposal, .. } = purpose else {
                    return Err(bad("invalid unsigned worker rejection"));
                };
                self.body.ingress = None;
                Ok(LaneBodyProgress::RejectedProposal { proposal, reason })
            }
            WorkResult::Body(materialized) => {
                let prepared = match &materialized {
                    Materialized::Prepared(prepared)
                    | Materialized::Stored(prepared, _)
                    | Materialized::Validated(prepared, _) => prepared,
                };
                if self
                    .body
                    .prepared
                    .as_ref()
                    .is_some_and(|body| body.canonical_bytes() != prepared.body.canonical_bytes())
                {
                    return Err(bad(
                        "one lane instance cannot replace its immutable all-route input",
                    ));
                }
                if let Materialized::Stored(_, read) = &materialized {
                    if read.receipt().manifest() != &prepared.manifest
                        || read.canonical_bytes() != prepared.body.canonical_bytes()
                    {
                        return Err(bad("stored completion lost its exact durable receipt"));
                    }
                }
                // Compute deterministic input eligibility and native authentication
                // before taking the State publication lease.
                let eligibility = state
                    .prepare_lane_input_body(observed, &self.verified, prepared.body.source())
                    .map_err(bad)?;
                let wait = match eligibility {
                    LaneInputBodyPreparationV1::Ready(body) => {
                        if body.canonical_bytes() != prepared.body.canonical_bytes() {
                            return Err(bad(
                                "current all-route body changed inside an open immutable instance",
                            ));
                        }
                        None
                    }
                    LaneInputBodyPreparationV1::BlockedByEarlierInputs(deps) => {
                        Some(LaneBodyWait::EarlierHeads(deps))
                    }
                    LaneInputBodyPreparationV1::InstanceNotCurrent => {
                        Some(LaneBodyWait::CurrentSet(LaneCurrentGate::InstanceClosed))
                    }
                    LaneInputBodyPreparationV1::ObservationChanged => Some(
                        LaneBodyWait::CurrentSet(LaneCurrentGate::ObservationChanged),
                    ),
                };
                if let Some(wait) = wait {
                    self.body.completed = Some((purpose, WorkResult::Body(materialized)));
                    return Ok(LaneBodyProgress::Waiting(wait));
                }
                let event = match &purpose {
                    Purpose::Local { tag } => Some(reducer::Event::LocalProposalReady {
                        tag: *tag,
                        manifest: core_manifest(prepared.manifest)?,
                    }),
                    Purpose::Ingress { proposal, tag } => Some(
                        LaneAuthenticator::new(&self.verified)
                            .event(&LaneMessageV1::Proposal(proposal.clone()), *tag)
                            .map_err(bad)?,
                    ),
                    Purpose::Issued(reducer::Effect::FetchBody {
                        tag,
                        round,
                        subject,
                        ..
                    }) => Some(reducer::Event::BodyAvailable {
                        tag: *tag,
                        round: *round,
                        subject: *subject,
                    }),
                    Purpose::Issued(reducer::Effect::StoreBody {
                        tag,
                        round,
                        subject,
                        ..
                    }) => Some(reducer::Event::BodyStored {
                        tag: *tag,
                        round: *round,
                        subject: *subject,
                    }),
                    Purpose::Issued(reducer::Effect::ValidateBody {
                        tag,
                        round,
                        subject,
                        ..
                    }) => Some(reducer::Event::ValidationCompleted {
                        tag: *tag,
                        round: *round,
                        subject: *subject,
                        valid: true,
                    }),
                    Purpose::Issued(reducer::Effect::Sign { .. }) => None,
                    _ => return Err(bad("body completion has non-body purpose")),
                };
                let _lease = state.consensus_publication_lease();
                let gate = self.current_gate(state, observed);
                if gate != LaneCurrentGate::Current {
                    self.body.completed = Some((purpose, WorkResult::Body(materialized)));
                    return Ok(LaneBodyProgress::Waiting(LaneBodyWait::CurrentSet(gate)));
                }
                let guard = Arc::clone(&self.output_guard);
                let operation = guard
                    .begin_fail_stop_operation()
                    .ok_or_else(|| bad("body completion output is closed"))?;
                self.body.source = Some(prepared.body.source().clone());
                let subject = subject(prepared.manifest.value)?;
                self.body.prepared = Some(Arc::clone(&prepared.body));
                self.body.manifests.insert(subject, prepared.manifest);
                self.body.values.insert(subject, prepared.manifest.value);
                // The tagged completion consumes an actual fsynced/validated result.
                // Retain validation custody through later signing and Set B fallback.
                if let Materialized::Validated(_, read) = &materialized {
                    self.body.validated.insert(subject, Arc::clone(read));
                }
                let Some(event) = event else {
                    self.restore_purpose(purpose);
                    operation.complete();
                    return Ok(LaneBodyProgress::SignReady);
                };
                let input = if let Purpose::Ingress { proposal, .. } = &purpose {
                    Some(LaneMessageV1::Proposal(proposal.clone()))
                } else {
                    None
                };
                let receipt = self.step(event, input.as_ref())?;
                if receipt.disposition
                    == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
                {
                    self.body.completed = Some((purpose, WorkResult::Body(materialized)));
                } else if matches!(purpose, Purpose::Ingress { .. }) {
                    self.body.ingress = None;
                }
                operation.complete();
                Ok(LaneBodyProgress::Stepped(receipt))
            }
        }
    }

    /// Fresh eligibility for a body-dependent signature/output, with no disk I/O.
    /// The retained physical validation result is an actual receipt, not a bool.
    pub(super) fn body_gate(
        &self,
        state: &State,
        observed: &VerifiedLaneContexts,
        subject: reducer::Subject,
    ) -> Result<Option<LaneBodyWait>> {
        let Some(read) = self.body.validated.get(&subject) else {
            return Ok(Some(LaneBodyWait::NoWork));
        };
        let Some(body) = &self.body.prepared else {
            return Err(bad("validated physical receipt lost its immutable input"));
        };
        if read.canonical_bytes() != body.canonical_bytes() {
            return Err(bad("held receipt differs from immutable input"));
        }
        Ok(
            match state
                .prepare_lane_input_body(observed, &self.verified, body.source())
                .map_err(bad)?
            {
                LaneInputBodyPreparationV1::Ready(current) => {
                    if current.canonical_bytes() != body.canonical_bytes() {
                        return Err(bad(
                            "current all-route body changed inside an open immutable instance",
                        ));
                    }
                    None
                }
                LaneInputBodyPreparationV1::BlockedByEarlierInputs(deps) => {
                    Some(LaneBodyWait::EarlierHeads(deps))
                }
                LaneInputBodyPreparationV1::InstanceNotCurrent => {
                    Some(LaneBodyWait::CurrentSet(LaneCurrentGate::InstanceClosed))
                }
                _ => Some(LaneBodyWait::CurrentSet(
                    LaneCurrentGate::ObservationChanged,
                )),
            },
        )
    }

    pub(super) fn service_body_sign(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
    ) -> Result<Option<LaneService>> {
        let result = self.service_body_sign_inner(state, observed);
        if result.is_err() {
            self.failed = true;
            self.output_guard.close_admission_for_restart();
        }
        result
    }

    fn service_body_sign_inner(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
    ) -> Result<Option<LaneService>> {
        let Some(index) = self.held.iter().position(|held| {
            matches!(&held.effect,
            reducer::Effect::Sign {message,..} if sign_subject(message).is_some())
        }) else {
            return Ok(None);
        };
        if !self.reserve_step() {
            return Ok(Some(LaneService::NeedsBodyAdapter));
        }
        let reducer::Effect::Sign { tag, message } = &self.held[index].effect else {
            unreachable!()
        };
        let tag = *tag;
        if let Some(wait) =
            self.body_gate(state, observed, sign_subject(message).expect("body sign"))?
        {
            return Ok(Some(match wait {
                LaneBodyWait::NoWork => LaneService::NeedsBodyAdapter,
                other => LaneService::BodyWaiting(other),
            }));
        }
        let auth = LaneAuthenticator::new(&self.verified);
        let preimage = self
            .native_records
            .iter()
            .rev()
            .find_map(|record| auth.native_signing_preimage(message, record).ok())
            .ok_or_else(|| bad("body signing lost its exact fsynced native intent"))?;
        let _lease = state.consensus_publication_lease();
        let gate = self.current_gate(state, observed);
        if gate != LaneCurrentGate::Current {
            return Ok(Some(LaneService::Gate(gate)));
        }
        let guard = Arc::clone(&self.output_guard);
        let operation = guard
            .begin_fail_stop_operation()
            .ok_or_else(|| bad("consensus output is closed"))?;
        let signature = Signature::try_new(self.key.private_key(), &preimage).map_err(bad)?;
        self.completion = Some(reducer::Event::Signed {
            tag,
            signature: reducer::OpaqueSignature::new(signature.payload().to_vec()),
        });
        self.held.remove(index);
        operation.complete();
        Ok(Some(LaneService::SignedAwaitingAck))
    }

    /// Durable native CommitQC is immediately visible even if input recovery is
    /// pending. The exact Apply/source obligation remains owned by this instance.
    pub(crate) fn durable_decision_certificate(
        &self,
    ) -> Option<&iroha_data_model::block::lane_consensus::LaneQcV1> {
        self.native_records
            .iter()
            .rev()
            .find_map(|record| match &record.record {
                LaneWalRecordV1::Decision(qc) => Some(qc),
                _ => None,
            })
    }

    /// Borrow exact durable native Decisions promptly for the read-only group join.
    /// Apply remains owned here until the eventual exact global application contract.
    pub(crate) fn native_decision(&self) -> Result<Option<LaneDecisionV1>> {
        let Some(qc) = self
            .native_records
            .iter()
            .rev()
            .find_map(|record| match &record.record {
                LaneWalRecordV1::Decision(qc) => Some(qc),
                _ => None,
            })
        else {
            return Ok(None);
        };
        let subject = subject(qc.statement.value)?;
        let Some(manifest) = self.body.manifests.get(&subject).copied().or_else(|| {
            self.body
                .validated
                .get(&subject)
                .map(|read| *read.receipt().manifest())
        }) else {
            return Ok(None);
        };
        Ok(Some(LaneDecisionV1 {
            manifest,
            commit_qc: qc.clone(),
        }))
    }
}

fn core_manifest(manifest: LaneManifestV1) -> Result<reducer::PayloadManifest> {
    Ok(reducer::PayloadManifest::new(
        subject(manifest.value)?,
        reducer::Digest::new(manifest.value.payload_hash.into()),
        reducer::Digest::new(manifest.chunk_root.into()),
        manifest.byte_len,
        manifest.chunk_count,
    ))
}

/// Only body-dependent signatures require local durable input custody. Already
/// authenticated QCs/TCs remain relayable while recovery or all-route work waits.
pub(super) fn broadcast_subject(message: &reducer::ConsensusMessageV2) -> Option<reducer::Subject> {
    match message {
        reducer::ConsensusMessageV2::Proposal(proposal) => {
            Some(proposal.proposal().manifest().subject())
        }
        reducer::ConsensusMessageV2::Vote(vote) => Some(vote.vote().subject()),
        _ => None,
    }
}
