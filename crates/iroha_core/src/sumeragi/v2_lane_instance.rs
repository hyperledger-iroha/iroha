//! Process-lived native instance control custody around the sole shared reducer.
//!
//! This bounded slice owns pre-payload clocks, move-owned persistence/body jobs,
//! frozen-key signing and move-only output handoff. It intentionally has no runner
//! construction site yet: atomic retirement of the old lane signer, native P2P
//! routing, worker admission and group application must precede production activation.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc,
        mpsc::{SyncSender, TrySendError},
    },
    time::{Duration, Instant},
};

use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::block::lane_consensus::{
    LANE_MESSAGE_VERSION_V1, LaneManifestV1, LaneMessageEnvelopeV1, LaneMessageV1, LaneQcV1,
    LaneTimeoutVoteV1, LaneValueRefV1,
};
use iroha_model_base::peer::PeerId;

use super::{
    output_guard::ConsensusOutputGuard,
    v2_core as reducer,
    v2_lane_body_store::LaneBodyStore,
    v2_lane_wal::LaneSafetyWal,
    v2_lane_wire::{LaneAuthenticator, LaneNativeWitnesses, LaneWalEnvelopeV1, LaneWalRecordV1},
    v2_runtime::round_timeout_for_view,
};
use crate::state::{State, VerifiedLaneContext, VerifiedLaneContexts};

#[path = "v2_lane_instance_body.rs"]
mod body;
#[path = "v2_lane_instance_opening.rs"]
mod opening;
#[path = "v2_lane_instance_persistence.rs"]
mod persistence;
#[path = "v2_lane_process.rs"]
mod process;
pub(crate) use body::{
    LaneApplySettlement, LaneBodyCompletion, LaneBodyJob, LaneBodyLaunch, LaneBodyProgress,
    LaneBodyWait,
};
pub(crate) use opening::{
    LaneOpening, LaneOpeningAdoption, LaneOpeningCompletion, LaneOpeningDrain, LaneOpeningDrained,
    LaneOpeningJob,
};
#[cfg(test)]
pub(crate) use persistence::LanePersistenceWait;
pub(crate) use persistence::{
    LanePersistenceCompletion, LanePersistenceJob, LanePersistenceLaunch,
};
pub(crate) use process::{
    LaneClosedInstance, LanePhysicalPool, LanePhysicalShutdown, LaneProcessLimits,
    LaneProcessOwner, LaneProcessProgress, LaneSourceRecoveryTarget, LaneWorkerClass,
};
#[cfg(test)]
pub(crate) use process::{LanePhysicalCompletion, LaneProcessOccupancy};

/// A control-boundary contradiction or permanent physical failure.
#[derive(Debug, thiserror::Error)]
#[error("native lane instance: {0}")]
pub(crate) struct LaneInstanceError(String);
fn bad(error: impl ToString) -> LaneInstanceError {
    LaneInstanceError(error.to_string())
}
type Result<T> = std::result::Result<T, LaneInstanceError>;

/// Exact current-state gate; an observation is never a permanent signing lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LaneCurrentGate {
    Current,
    ObservationChanged,
    InstanceClosed,
}

/// Non-owning reducer disposition. Actual retired obligations remain charged to
/// the original instance until its explicit retirement handoff is consumed.
#[derive(Debug)]
pub(crate) struct LaneStepReceipt {
    pub(crate) disposition: reducer::StepDisposition,
}

/// Exact unlaunched effect retired by a shared-reducer transition.
#[derive(Debug)]
pub(crate) struct LaneRetiredEffect {
    /// Original issued obligation, never represented as a successful completion.
    pub(crate) effect: reducer::Effect,
    /// Already materialized native bytes, when present, follow the retirement.
    pub(crate) packet: Option<LaneOutbound>,
}

/// Original retired custody. This is a consuming physical handoff, not an Apply,
/// Ready, persistence, transport-delivery or terminal-recovery acknowledgement.
#[must_use]
pub(crate) struct LaneRetirement {
    state_owner: crate::state::NativeLaneStateOwner,
    verified: Arc<VerifiedLaneContext>,
    kind: RetirementKind,
}

enum RetirementKind {
    Effect(LaneRetiredEffect),
    Body(body::RetiredBody),
}

// Private proof-bound permission used only after all fallible checks. It keeps
// the real PublishedCarrier borrow live through original custody consumption.
struct PublishedTerminalRetirement<'proof, 'published> {
    _published: &'proof crate::state::PublishedNativeApply<'published>,
}

impl LaneRetirement {
    /// Borrow the immutable context allocation retained from accepted opening.
    #[cfg(test)]
    pub(crate) fn context_for_test(&self) -> &VerifiedLaneContext {
        &self.verified
    }

    /// Consume only a separately transferred closure-body result after genuine
    /// publication of its original instance. Refusal returns the same armed token.
    /// This does not acknowledge an effect, release another owner or delete disk data.
    pub(crate) fn retire_published(
        self,
        published: &crate::state::PublishedNativeApply<'_>,
    ) -> std::result::Result<(), (Self, LaneInstanceError)> {
        if !self.requires_recovery() {
            return Err((
                self,
                bad("terminal publication requires original closure-body custody"),
            ));
        }
        if let Err(error) =
            published.authorizes_terminal(&self.state_owner, &self.verified, std::iter::empty())
        {
            return Err((self, bad(error)));
        }
        let authorized = PublishedTerminalRetirement {
            _published: published,
        };
        // The kind was checked above without changing the token. Only this
        // original body is consumed; the original closed instance remains armed.
        if let RetirementKind::Body(body) = self.kind {
            body.retire_published(&authorized);
        }
        Ok(())
    }
    /// Exact original immutable instance; no current-height reconstruction.
    pub(crate) fn instance(&self) -> iroha_data_model::block::consensus_v2::HeightContextId {
        self.verified.instance_id()
    }
    /// The original opening State family follows physical custody.
    pub(crate) fn belongs_to(&self, state: &State) -> bool {
        self.state_owner.matches_state(state)
    }
    /// Unfinished closure custody keeps the original output guard armed even
    /// after transfer. Dropping it requires restart, never semantic success.
    pub(crate) fn requires_recovery(&self) -> bool {
        matches!(&self.kind, RetirementKind::Body(retired) if retired.requires_recovery())
    }
    /// Original issued effect, if this retirement came from an issued operation.
    pub(crate) fn effect(&self) -> Option<&reducer::Effect> {
        match &self.kind {
            RetirementKind::Effect(retired) => Some(&retired.effect),
            RetirementKind::Body(retired) => retired.effect(),
        }
    }
    /// Original materialized packet, including its exact bytes and destinations.
    pub(crate) fn packet(&self) -> Option<&LaneOutbound> {
        match &self.kind {
            RetirementKind::Effect(retired) => retired.packet.as_ref(),
            RetirementKind::Body(_) => None,
        }
    }
    /// Original signed ingress proposal, if present.
    pub(crate) fn proposal(
        &self,
    ) -> Option<&iroha_data_model::block::lane_consensus::LaneProposalV1> {
        match &self.kind {
            RetirementKind::Body(retired) => retired.proposal(),
            RetirementKind::Effect(_) => None,
        }
    }
    /// Retained deterministic proposal diagnostic, never a synthetic completion.
    pub(crate) fn rejection(&self) -> Option<&str> {
        match &self.kind {
            RetirementKind::Body(retired) => retired.rejection(),
            RetirementKind::Effect(_) => None,
        }
    }
    /// Original canonical body allocation returned by the real physical worker.
    pub(crate) fn body_bytes(&self) -> Option<&[u8]> {
        match &self.kind {
            RetirementKind::Body(retired) => retired.body_bytes(),
            RetirementKind::Effect(_) => None,
        }
    }
}

/// Borrowed input remains with the caller unless the reducer accepts/classifies it.
#[derive(Debug)]
pub(crate) enum LaneInputOutcome {
    /// Neither owned clock is due; no reducer transition occurred.
    NotDue,
    Stepped(LaneStepReceipt),
    Gate(LaneCurrentGate),
    /// A physical completion or full effect reservation must be serviced first.
    Backpressured,
    /// Exact signed proposal retained pending deterministic input checking.
    BodyPreparationQueued,
}

/// One actual move-owning transport packet, with exact frozen destinations.
/// Channel acceptance transfers custody, not delivery or consensus authority.
#[derive(Debug)]
pub(crate) struct LaneOutbound {
    pub(crate) envelope: LaneMessageEnvelopeV1,
    pub(crate) canonical_bytes: Vec<u8>,
    pub(crate) destinations: Vec<PeerId>,
}

/// One service boundary; disk and signing completions remain owned until the next step.
#[derive(Debug)]
pub(crate) enum LaneService {
    Idle,
    Gate(LaneCurrentGate),
    /// Returned only after a real persistence worker returns its private receipt.
    PersistedAwaitingAck,
    /// Exact unlaunched Persist needs bounded worker admission.
    NeedsPersistenceWorker,
    /// One worker/completion owns the physical WAL; other instances remain runnable.
    PersistenceInFlight,
    SignedAwaitingAck,
    Completion(LaneStepReceipt),
    EnteredView(reducer::EventTag),
    Sent,
    OutboxFull,
    /// The exact effect is still present in `held_effects`; no completion exists.
    NeedsBodyAdapter,
    /// The exact productive effect remains held behind a named source/route gate.
    #[cfg_attr(
        test,
        expect(
            dead_code,
            reason = "TODO: consume retained native lane progress through the production driver"
        )
    )]
    BodyWaiting(LaneBodyWait),
}

struct HeldEffect {
    effect: reducer::Effect,
    native: Option<LaneWalEnvelopeV1>,
    packet: Option<LaneOutbound>,
}

/// The clock is an owned tagged deadline, never a second view/lock authority.
struct LaneClock {
    tag: reducer::EventTag,
    timeout: Option<Instant>,
    retransmit: Instant,
}

/// Single process-lived control owner for one authenticated immutable lane instance.
///
/// TODO: the production owner table must enforce one instance/key construction,
/// retain it across global rollover, and drain it after authenticated closure.
/// No global-height adapter or old signer constructs this type today.
pub(crate) struct LaneInstance {
    state_owner: crate::state::NativeLaneStateOwner,
    verified: Arc<VerifiedLaneContext>,
    reducer: reducer::Reducer,
    wal: Option<LaneSafetyWal>,
    persistence: Option<Arc<persistence::IssuedPersistence>>,
    /// Own the sole physical body entry from opening, without granting Ready.
    body_store: Option<LaneBodyStore>,
    body: body::BodyCustody,
    native_records: Vec<LaneWalEnvelopeV1>,
    /// Native evidence for the reducer's exact partial timeout pools. This map
    /// is pruned from reducer snapshots; it makes no quorum or view decision.
    timeout_witnesses: BTreeMap<(u64, u32), LaneTimeoutVoteV1>,
    key: KeyPair,
    output_guard: Arc<ConsensusOutputGuard>,
    clock: LaneClock,
    base_timeout: Duration,
    retransmit_interval: Duration,
    held: Vec<HeldEffect>,
    /// Original obligations stay in the same per-instance descriptor budget.
    retired: VecDeque<RetirementKind>,
    /// Exact cleanup ticket retains its original descriptor through worker return.
    retirement_cleanup: Option<Arc<()>>,
    completion: Option<reducer::Event>,
    effect_limit: usize,
    failed: bool,
}

impl LaneInstance {
    fn deadline(now: Instant, interval: Duration) -> Result<Instant> {
        now.checked_add(interval)
            .ok_or_else(|| bad("native lane clock exceeds Instant range"))
    }
    fn preflight_clock(now: Instant, base: Duration, retransmit: Duration) -> Result<()> {
        Self::deadline(now, round_timeout_for_view(base, u64::MAX))?;
        Self::deadline(now, retransmit)?;
        Ok(())
    }

    fn gate_for(
        verified: &VerifiedLaneContext,
        state: &State,
        observed: &VerifiedLaneContexts,
    ) -> LaneCurrentGate {
        if !observed.is_current(state) {
            return LaneCurrentGate::ObservationChanged;
        }
        if observed.contexts().iter().any(|current| {
            current.instance_id() == verified.instance_id() && current.frozen() == verified.frozen()
        }) {
            LaneCurrentGate::Current
        } else {
            LaneCurrentGate::InstanceClosed
        }
    }
    fn current_gate(&self, state: &State, observed: &VerifiedLaneContexts) -> LaneCurrentGate {
        if !self.state_owner.matches_state(state) {
            return LaneCurrentGate::ObservationChanged;
        }
        Self::gate_for(&self.verified, state, observed)
    }

    /// Borrow the immutable context allocation retained from accepted opening.
    #[cfg(test)]
    pub(crate) fn context_for_test(&self) -> &VerifiedLaneContext {
        &self.verified
    }

    /// Borrow the actual opening State identity without constructing a replacement.
    #[cfg(test)]
    pub(crate) fn state_owner_for_test(&self) -> &crate::state::NativeLaneStateOwner {
        &self.state_owner
    }
    fn check_open(&self) -> Result<()> {
        if self.failed {
            Err(bad("instance requires physical recovery"))
        } else {
            Ok(())
        }
    }
    fn reserve_effects(&self, additional: usize) -> bool {
        self.held
            .len()
            .checked_add(self.retired.len())
            .and_then(|count| count.checked_add(usize::from(self.retirement_cleanup.is_some())))
            .and_then(|count| count.checked_add(self.body.retained_job_count()))
            .and_then(|count| count.checked_add(usize::from(self.persistence.is_some())))
            .and_then(|count| count.checked_add(additional))
            .is_some_and(|count| count <= self.effect_limit)
    }

    fn reserve_step(&self) -> bool {
        self.reserve_effects(reducer::MAX_EFFECTS_PER_STEP)
    }

    fn reserve_ingress(&self) -> bool {
        self.reserve_effects(2 * reducer::MAX_EFFECTS_PER_STEP)
    }

    /// Leave no free descriptor slots around the original actual obligations.
    /// This changes only the fixture's capacity, never its reducer or effects.
    #[cfg(test)]
    pub(crate) fn restrict_effect_capacity_to_retained_for_test(&mut self) {
        self.effect_limit = self
            .held
            .len()
            .checked_add(self.retired.len())
            .and_then(|count| count.checked_add(usize::from(self.retirement_cleanup.is_some())))
            .and_then(|count| count.checked_add(self.body.retained_job_count()))
            .and_then(|count| count.checked_add(usize::from(self.persistence.is_some())))
            .expect("fixture retained descriptors fit usize");
        assert!(self.reserve_effects(0));
        assert!(!self.reserve_step());
    }

    /// Borrow retained retirement occupancy without releasing its reservation.
    pub(crate) fn retirement_count(&self) -> usize {
        self.retired.len() + usize::from(self.retirement_cleanup.is_some())
    }

    /// Transfer exactly one original retirement to the downstream owner. Taking
    /// custody frees its instance descriptor; it does not acknowledge any effect.
    pub(crate) fn take_retirement(&mut self) -> Option<LaneRetirement> {
        self.retired.pop_front().map(|kind| LaneRetirement {
            state_owner: self.state_owner.clone(),
            verified: Arc::clone(&self.verified),
            kind,
        })
    }

    // Only the original producer selects semantically retired custody. A closure
    // result remains armed for the separate PublishedNativeApply consumer.
    fn take_retirement_cleanup(
        &mut self,
        state: &State,
        guard: &Arc<ConsensusOutputGuard>,
    ) -> Result<Option<(Arc<()>, LaneRetirement)>> {
        if !self.state_owner.matches_state(state) || !Arc::ptr_eq(&self.output_guard, guard) {
            return Err(bad("foreign Native retirement cleanup owner"));
        }
        if self.retirement_cleanup.is_some() {
            return Ok(None);
        }
        let Some(index) = self.retired.iter().position(|retired| match retired {
            RetirementKind::Effect(retired) => matches!(
                retired.effect,
                reducer::Effect::Broadcast(_)
                    | reducer::Effect::Sign { .. }
                    | reducer::Effect::FetchBody { .. }
                    | reducer::Effect::StoreBody { .. }
                    | reducer::Effect::ValidateBody { .. }
            ),
            RetirementKind::Body(body) => !body.requires_recovery(),
        }) else {
            return Ok(None);
        };
        let ticket = Arc::new(());
        let kind = self
            .retired
            .remove(index)
            .expect("selected original retirement");
        self.retirement_cleanup = Some(Arc::clone(&ticket));
        Ok(Some((
            ticket,
            LaneRetirement {
                state_owner: self.state_owner.clone(),
                verified: Arc::clone(&self.verified),
                kind,
            },
        )))
    }

    fn finish_retirement_cleanup(&mut self, ticket: &Arc<()>) -> Result<()> {
        if !self
            .retirement_cleanup
            .as_ref()
            .is_some_and(|issued| Arc::ptr_eq(issued, ticket))
        {
            return Err(bad("foreign Native retirement cleanup completion"));
        }
        self.retirement_cleanup.take();
        Ok(())
    }

    fn authorize_terminal_retirement<'proof, 'published>(
        &self,
        published: &'proof crate::state::PublishedNativeApply<'published>,
    ) -> Result<PublishedTerminalRetirement<'proof, 'published>> {
        self.check_open()?;
        if self.wal.is_some()
            || self.body_store.is_some()
            || self.persistence.is_some()
            || self.retirement_cleanup.is_some()
            || self.body.worker_in_flight()
        {
            return Err(bad(
                "terminal retirement requires the original completed physical drain",
            ));
        }
        if self
            .held
            .iter()
            .any(|held| matches!(held.effect, reducer::Effect::Apply { .. }))
            || self.retired.iter().any(|retired| {
                matches!(
                    retired,
                    RetirementKind::Effect(LaneRetiredEffect {
                        effect: reducer::Effect::Apply { .. },
                        ..
                    })
                )
            })
        {
            return Err(bad(
                "original Apply requires genuine publication settlement before retirement",
            ));
        }
        published
            .authorizes_terminal(
                &self.state_owner,
                &self.verified,
                self.native_records
                    .iter()
                    .chain(self.held.iter().filter_map(|held| held.native.as_ref()))
                    .filter_map(|record| match &record.record {
                        LaneWalRecordV1::Decision(qc) => Some(qc),
                        _ => None,
                    }),
            )
            .map_err(bad)?;
        Ok(PublishedTerminalRetirement {
            _published: published,
        })
    }

    // All authentication precedes this infallible consumption. No reducer event
    // or persistence acknowledgement is synthesized; disk records stay intact.
    fn consume_published_retirements(&mut self, authorized: &PublishedTerminalRetirement<'_, '_>) {
        for retired in self.retired.drain(..) {
            if let RetirementKind::Body(body) = retired {
                body.retire_published(authorized);
            }
        }
    }

    /// Borrow the physical owner for a future checked body adapter. This grants
    /// no reducer completion, signing permission or current all-route lease.
    #[cfg(test)]
    pub(crate) fn body_store(&mut self) -> &mut LaneBodyStore {
        self.body_store
            .as_mut()
            .expect("no worker owns the physical store")
    }

    /// Current reducer tag; no copied mutable view or generation is exposed.
    pub(crate) fn tag(&self) -> reducer::EventTag {
        self.reducer.current_tag()
    }
    /// Deadline survives replacement of the observed global carrier.
    pub(crate) fn timeout_deadline(&self) -> Option<Instant> {
        self.clock.timeout
    }
    /// Exact unlaunched effects, including unsupported body work.
    pub(crate) fn held_effects(&self) -> impl Iterator<Item = &reducer::Effect> {
        self.held.iter().map(|held| &held.effect)
    }
    #[cfg(test)]
    pub(crate) fn body_state_for_test(
        &self,
        round: reducer::Round,
        subject: reducer::Subject,
    ) -> reducer::BodyState {
        self.reducer.body_state(round, subject)
    }

    /// Exact native records whose physical fsync has completed.
    pub(crate) fn native_records(&self) -> &[LaneWalEnvelopeV1] {
        &self.native_records
    }

    /// Authenticate outside the publication lease, then admit under exact current authority.
    /// Proposals retain their original signed body until input preparation completes.
    pub(crate) fn offer(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
        message: &LaneMessageV1,
    ) -> Result<LaneInputOutcome> {
        self.check_open()?;
        // Native signatures/QCs may be expensive; a frozen context permits
        // read-only verification without blocking global State publication.
        let event = LaneAuthenticator::new(&self.verified)
            .event(message, self.tag())
            .map_err(bad)?;
        let _lease = state.consensus_publication_lease();
        let gate = self.current_gate(state, observed);
        if gate != LaneCurrentGate::Current {
            return Ok(LaneInputOutcome::Gate(gate));
        }
        if self.completion.is_some() || !self.reserve_ingress() || self.clock.tag != self.tag() {
            return Ok(LaneInputOutcome::Backpressured);
        }
        if let LaneMessageV1::Proposal(proposal) = message {
            return Ok(self.defer_proposal(proposal));
        }
        // Valid CommitQCs and timeout controls do not wait for source/body work.
        // Prepare/Commit signing separately requires a physical validated input.
        let guard = Arc::clone(&self.output_guard);
        let operation = guard
            .begin_fail_stop_operation()
            .ok_or_else(|| bad("consensus output is closed"))?;
        let receipt = self.step(event, Some(message))?;
        operation.complete();
        if receipt.disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            return Ok(LaneInputOutcome::Backpressured);
        }
        Ok(LaneInputOutcome::Stepped(receipt))
    }

    /// Service an owned due deadline. Busy preserves the original due clock.
    pub(crate) fn poll_clock(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
        now: Instant,
    ) -> Result<LaneInputOutcome> {
        self.check_open()?;
        let _lease = state.consensus_publication_lease();
        let gate = self.current_gate(state, observed);
        if gate != LaneCurrentGate::Current {
            return Ok(LaneInputOutcome::Gate(gate));
        }
        if self.completion.is_some() || !self.reserve_ingress() || self.clock.tag != self.tag() {
            return Ok(LaneInputOutcome::Backpressured);
        }
        let timeout = self.clock.timeout.is_some_and(|deadline| deadline <= now);
        if !timeout && self.clock.retransmit > now {
            return Ok(LaneInputOutcome::NotDue);
        }
        // Reserve the next deadline before offering any reducer input.
        let next_retransmit = if timeout {
            None
        } else {
            Some(Self::deadline(now, self.retransmit_interval)?)
        };
        let event = if timeout {
            reducer::Event::TimeoutElapsed { tag: self.tag() }
        } else {
            reducer::Event::RetransmitElapsed { tag: self.tag() }
        };
        let guard = Arc::clone(&self.output_guard);
        let operation = guard
            .begin_fail_stop_operation()
            .ok_or_else(|| bad("consensus output is closed"))?;
        let receipt = self.step(event, None)?;
        if receipt.disposition != reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            if timeout {
                self.clock.timeout = None;
            } else {
                self.clock.retransmit =
                    next_retransmit.expect("retransmission deadline was preflighted");
            }
        }
        operation.complete();
        Ok(LaneInputOutcome::Stepped(receipt))
    }

    fn step(
        &mut self,
        event: reducer::Event,
        native_input: Option<&LaneMessageV1>,
    ) -> Result<LaneStepReceipt> {
        let result = self.step_inner(event, native_input);
        if result.is_err() {
            // Reducer or native-projection contradictions may follow mutation.
            // Never permit this in-memory instance to resume after such a fault.
            self.failed = true;
            self.output_guard.close_admission_for_restart();
        }
        result
    }

    fn step_inner(
        &mut self,
        event: reducer::Event,
        native_input: Option<&LaneMessageV1>,
    ) -> Result<LaneStepReceipt> {
        // Application completion consumes an existing obligation and the shared
        // reducer emits no effects. Requiring new-work headroom here would make
        // a saturated closed owner wait for the capacity it must itself release.
        let effect_budget = if matches!(&event, reducer::Event::ApplicationCompleted { .. }) {
            0
        } else {
            reducer::MAX_EFFECTS_PER_STEP
        };
        if !self.reserve_effects(effect_budget) {
            return Err(bad("complete effect reservation was not held"));
        }
        let old_tag = self.tag();
        let outcome = self.reducer.step(event).map_err(bad)?;
        if outcome.effects().len() > effect_budget {
            self.failed = true;
            self.output_guard.close_admission_for_restart();
            return Err(bad(
                "shared reducer exceeded its complete effect reservation",
            ));
        }
        let disposition = outcome.disposition();
        if let Some(LaneMessageV1::TimeoutVote(vote)) = native_input {
            let roster = self.verified.reducer_context().roster();
            let admitted = self.reducer.timeout_pool_snapshots().iter().any(|pool| {
                pool.round.view() == vote.body.round.voting_view
                    && pool
                        .signers
                        .contains(&roster[vote.share.signer as usize].id())
            });
            if admitted
                && disposition == reducer::StepDisposition::Applied
                && !outcome
                    .effects()
                    .iter()
                    .any(|effect| matches!(effect, reducer::Effect::ReportEquivocation { .. }))
            {
                self.timeout_witnesses
                    .entry((vote.body.round.voting_view, vote.share.signer))
                    .or_insert_with(|| vote.clone());
            }
        }
        if let Some(input) = native_input {
            self.body.observe(input)?;
        }
        for effect in outcome.into_effects() {
            if matches!(
                &effect,
                reducer::Effect::Broadcast(_)
                    | reducer::Effect::FetchBody { .. }
                    | reducer::Effect::StoreBody { .. }
                    | reducer::Effect::ValidateBody { .. }
                    | reducer::Effect::Apply { .. }
            ) && (self.held.iter().any(|held| held.effect == effect)
                || self.body.in_flight_matches(&effect))
            {
                // These effects are still unlaunched. The same exact owner
                // services repeated requests; no operation was acknowledged.
                continue;
            }
            self.held.push(HeldEffect {
                effect,
                native: None,
                packet: None,
            });
        }
        // First retain every issued effect. If projection contradicts the
        // native evidence, the failed owner still holds the exact obligation.
        let witnesses = self.witnesses(native_input)?;
        let auth = LaneAuthenticator::new(&self.verified);
        for held in &mut self.held {
            if held.native.is_none()
                && let reducer::Effect::Persist { entry, .. } = &held.effect
            {
                held.native = Some(auth.native_wal(entry, &witnesses).map_err(bad)?);
            }
        }
        let tag = self.tag();
        let mut retained = Vec::with_capacity(self.held.len());
        for held in self.held.drain(..) {
            let obsolete = match &held.effect {
                reducer::Effect::Broadcast(message) => !self
                    .reducer
                    .retained_control_messages()
                    .any(|current| current == message),
                reducer::Effect::Sign { tag: issued, .. }
                | reducer::Effect::FetchBody { tag: issued, .. }
                | reducer::Effect::StoreBody { tag: issued, .. }
                | reducer::Effect::ValidateBody { tag: issued, .. } => {
                    old_tag != tag && *issued != tag
                }
                _ => false,
            };
            if obsolete {
                self.retired
                    .push_back(RetirementKind::Effect(LaneRetiredEffect {
                        effect: held.effect,
                        packet: held.packet,
                    }));
            } else {
                retained.push(held);
            }
        }
        self.held = retained;
        let pools = self.reducer.timeout_pool_snapshots();
        let roster = self.verified.reducer_context().roster();
        self.timeout_witnesses.retain(|(view, signer), _| {
            pools.iter().any(|pool| {
                pool.round.view() == *view && pool.signers.contains(&roster[*signer as usize].id())
            })
        });
        self.body.prune(&self.reducer, &self.held);
        Ok(LaneStepReceipt { disposition })
    }

    /// Complete one control action. A Broadcast never sits ahead of persistence,
    /// completion, EnterView or timeout signing. Body signing requires actual
    /// validation custody and fresh all-route eligibility.
    pub(crate) fn service_one(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
        now: Instant,
    ) -> Result<LaneService> {
        self.check_open()?;
        if self.completion.is_some() {
            // A persistence ack can install any certified view. Validate the
            // clock range before taking the physical completion or mutating core.
            Self::preflight_clock(now, self.base_timeout, self.retransmit_interval)?;
            if !self.reserve_step() {
                self.failed = true;
                self.output_guard.close_admission_for_restart();
                return Err(bad("reserved completion lost effect capacity"));
            }
            let event = self.completion.take().expect("checked completion");
            return self
                .step(event, None)
                .map(LaneService::Completion)
                .inspect_err(|_| {
                    self.failed = true;
                    self.output_guard.close_admission_for_restart();
                });
        }
        if let Some(index) = self
            .held
            .iter()
            .position(|held| matches!(held.effect, reducer::Effect::EnterView { .. }))
        {
            let reducer::Effect::EnterView { tag, .. } = &self.held[index].effect else {
                unreachable!()
            };
            let tag = *tag;
            if tag != self.tag() {
                self.failed = true;
                self.output_guard.close_admission_for_restart();
                return Err(bad("unserviced EnterView was superseded"));
            }
            let clock = LaneClock {
                tag,
                timeout: Some(Self::deadline(
                    now,
                    round_timeout_for_view(self.base_timeout, tag.view()),
                )?),
                retransmit: Self::deadline(now, self.retransmit_interval)?,
            };
            // A failed deadline calculation leaves the exact EnterView owned.
            self.held.remove(index);
            self.clock = clock;
            return Ok(LaneService::EnteredView(tag));
        }
        // The scheduler must reserve a bounded worker slot before taking the
        // move-owned job. No append/fsync occurs on this control service path.
        if self.persistence.is_some() {
            return Ok(LaneService::PersistenceInFlight);
        }
        if self
            .held
            .iter()
            .any(|held| matches!(held.effect, reducer::Effect::Persist { .. }))
        {
            return Ok(LaneService::NeedsPersistenceWorker);
        }
        if let Some(index) = self.held.iter().position(|held| {
            matches!(
                &held.effect,
                reducer::Effect::Sign {
                    message: reducer::SignableMessage::TimeoutVote(_),
                    ..
                }
            )
        }) {
            if !self.reserve_step() {
                return Ok(LaneService::NeedsBodyAdapter);
            }
            let _lease = state.consensus_publication_lease();
            let gate = self.current_gate(state, observed);
            if gate != LaneCurrentGate::Current {
                return Ok(LaneService::Gate(gate));
            }
            let reducer::Effect::Sign { tag, message } = &self.held[index].effect else {
                unreachable!()
            };
            let auth = LaneAuthenticator::new(&self.verified);
            let Some(preimage) = self
                .native_records
                .iter()
                .rev()
                .find_map(|native| auth.native_signing_preimage(message, native).ok())
            else {
                self.failed = true;
                self.output_guard.close_admission_for_restart();
                return Err(bad("timeout signing lost its fsynced native intent"));
            };
            let guard = Arc::clone(&self.output_guard);
            let operation = guard
                .begin_fail_stop_operation()
                .ok_or_else(|| bad("consensus output is closed"))?;
            let signature = Signature::try_new(self.key.private_key(), &preimage).map_err(bad)?;
            self.completion = Some(reducer::Event::Signed {
                tag: *tag,
                signature: reducer::OpaqueSignature::new(signature.payload().to_vec()),
            });
            self.held.remove(index);
            operation.complete();
            return Ok(LaneService::SignedAwaitingAck);
        }
        if let Some(result) = self.service_body_sign(state, observed)? {
            return Ok(result);
        }
        if self
            .held
            .iter()
            .any(|held| !matches!(held.effect, reducer::Effect::Broadcast(_)))
        {
            return Ok(LaneService::NeedsBodyAdapter);
        }
        Ok(LaneService::Idle)
    }

    /// Transfer exactly one native packet to a bounded outbox under a fresh lease.
    /// Full/Disconnected retains the exact packet and shared Broadcast effect.
    pub(crate) fn flush_one(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
        sender: &SyncSender<LaneOutbound>,
    ) -> Result<LaneService> {
        self.check_open()?;
        let Some(index) = self
            .held
            .iter()
            .position(|held| {
                matches!(&held.effect, reducer::Effect::Broadcast(message)
                if body::broadcast_subject(message).is_none())
            })
            .or_else(|| {
                self.held
                    .iter()
                    .position(|held| matches!(held.effect, reducer::Effect::Broadcast(_)))
            })
        else {
            return Ok(LaneService::Idle);
        };
        if let reducer::Effect::Broadcast(message) = &self.held[index].effect
            && let Some(subject) = body::broadcast_subject(message)
            && let Some(wait) = self.body_gate(state, observed, subject)?
        {
            return Ok(match wait {
                LaneBodyWait::NoWork => LaneService::NeedsBodyAdapter,
                other => LaneService::BodyWaiting(other),
            });
        }
        if self.held[index].packet.is_none() {
            let reducer::Effect::Broadcast(message) = &self.held[index].effect else {
                unreachable!()
            };
            let witnesses = self.witnesses(None)?;
            let proposal = self.body.original_proposal(message).or_else(|| {
                let reducer::ConsensusMessageV2::Proposal(signed) = message else {
                    return None;
                };
                let proposal = signed.proposal();
                self.native_records
                    .iter()
                    .rev()
                    .find_map(|native| match &native.record {
                        LaneWalRecordV1::ProposalIntent(body)
                            if body.round.voting_view == proposal.round().view()
                                && body::subject(body.manifest.value).ok()
                                    == Some(proposal.manifest().subject()) =>
                        {
                            Some(body)
                        }
                        _ => None,
                    })
            });
            let message = LaneAuthenticator::new(&self.verified)
                .native_broadcast(message, &witnesses, proposal)
                .map_err(bad)?;
            let envelope = LaneMessageEnvelopeV1 {
                version: LANE_MESSAGE_VERSION_V1,
                message,
            };
            let canonical_bytes = norito::encode_canonical(&envelope).map_err(bad)?;
            self.held[index].packet = Some(LaneOutbound {
                envelope,
                canonical_bytes,
                destinations: self.verified.frozen().committee.clone(),
            });
        }
        let _lease = state.consensus_publication_lease();
        let gate = self.current_gate(state, observed);
        if gate != LaneCurrentGate::Current {
            return Ok(LaneService::Gate(gate));
        }
        let guard = Arc::clone(&self.output_guard);
        let operation = guard
            .begin_fail_stop_operation()
            .ok_or_else(|| bad("consensus output is closed"))?;
        let packet = self.held[index]
            .packet
            .take()
            .expect("encoded packet retained until move");
        let outcome = match sender.try_send(packet) {
            Ok(()) => {
                self.held.remove(index);
                LaneService::Sent
            }
            Err(TrySendError::Full(packet) | TrySendError::Disconnected(packet)) => {
                self.held[index].packet = Some(packet);
                LaneService::OutboxFull
            }
        };
        operation.complete();
        Ok(outcome)
    }

    /// Explicitly transfer diagnostic custody; it is not silently dropped.
    pub(crate) fn take_diagnostic(&mut self) -> Option<reducer::Effect> {
        let index = self.held.iter().position(|held| {
            matches!(
                held.effect,
                reducer::Effect::ReportEquivocation { .. }
                    | reducer::Effect::ReportInvalidCertifiedBody { .. }
            )
        })?;
        Some(self.held.remove(index).effect)
    }

    fn witnesses(&self, input: Option<&LaneMessageV1>) -> Result<Witnesses> {
        let mut witnesses = Witnesses::default();
        self.body.add_witnesses(&mut witnesses)?;
        if let Some(issued) = &self.persistence {
            witnesses.record(&issued.native.record)?;
        }
        for native in &self.native_records {
            witnesses.record(&native.record)?;
        }
        for vote in self.timeout_witnesses.values() {
            if let Some(qc) = &vote.body.highest_prepare {
                witnesses.qc(qc)?;
            }
        }
        if let Some(input) = input {
            match input {
                LaneMessageV1::TimeoutVote(vote) => {
                    if let Some(qc) = &vote.body.highest_prepare {
                        witnesses.qc(qc)?;
                    }
                }
                LaneMessageV1::TimeoutCertificate(tc) => {
                    for vote in &tc.votes {
                        if let Some(qc) = &vote.body.highest_prepare {
                            witnesses.qc(qc)?;
                        }
                    }
                }
                LaneMessageV1::Proposal(proposal) => {
                    witnesses.retain_value(proposal.body.manifest.value)?;
                    witnesses.manifests.insert(
                        body::subject(proposal.body.manifest.value)?,
                        proposal.body.manifest,
                    );
                }
                LaneMessageV1::Vote(vote) => witnesses.retain_value(vote.statement.value)?,
                LaneMessageV1::QuorumCertificate(qc) => witnesses.qc(qc)?,
            }
        }
        Ok(witnesses)
    }
}

/// Temporary projection of actual retained evidence, not a scheduling cache.
#[derive(Default)]
struct Witnesses {
    values: BTreeMap<reducer::Subject, LaneValueRefV1>,
    manifests: BTreeMap<reducer::Subject, LaneManifestV1>,
}
impl Witnesses {
    fn retain_value(&mut self, value: LaneValueRefV1) -> Result<()> {
        let subject = reducer::Subject::new(value.subject_hash().map_err(bad)?.into());
        if self
            .values
            .insert(subject, value)
            .is_some_and(|previous| previous != value)
        {
            return Err(bad("same subject has different native values"));
        }
        Ok(())
    }
    fn qc(&mut self, qc: &LaneQcV1) -> Result<()> {
        self.retain_value(qc.statement.value)
    }
    fn record(&mut self, record: &LaneWalRecordV1) -> Result<()> {
        match record {
            LaneWalRecordV1::ProposalIntent(body) => {
                self.retain_value(body.manifest.value)?;
                self.manifests.insert(
                    reducer::Subject::new(body.manifest.value.subject_hash().map_err(bad)?.into()),
                    body.manifest,
                );
            }
            LaneWalRecordV1::PrepareIntent { statement, .. } => {
                self.retain_value(statement.value)?
            }
            LaneWalRecordV1::ObservePrepare(qc) | LaneWalRecordV1::Decision(qc) => self.qc(qc)?,
            LaneWalRecordV1::LockAndCommit {
                prepare, statement, ..
            } => {
                self.qc(prepare)?;
                self.retain_value(statement.value)?;
            }
            LaneWalRecordV1::TimeoutIntent { body, .. } => {
                if let Some(qc) = &body.highest_prepare {
                    self.qc(qc)?;
                }
            }
            LaneWalRecordV1::InstallTimeout(tc) => {
                for vote in &tc.votes {
                    if let Some(qc) = &vote.body.highest_prepare {
                        self.qc(qc)?;
                    }
                }
            }
        }
        Ok(())
    }
}
impl LaneNativeWitnesses for Witnesses {
    fn value(&self, subject: reducer::Subject) -> Option<&LaneValueRefV1> {
        self.values.get(&subject)
    }
    fn manifest(&self, subject: reducer::Subject) -> Option<&LaneManifestV1> {
        self.manifests.get(&subject)
    }
}
