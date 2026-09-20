//! Inactive process-lived native owner assembly and bounded physical workers.
//! The table contains actual instance/job/result custody, never reducer facts.
//! TODO: wire fair native ingress, transport acknowledgements, first-source
//! recovery and the sole economic Apply consumer, then retire old signers atomically.

use super::*;
use crate::kura::Kura;
use iroha_data_model::block::consensus_v2::HeightContextId;
use std::{
    num::NonZeroUsize,
    sync::{
        Mutex,
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::{self, JoinHandle},
};

/// Explicit validated construction limits, not a runtime configuration toggle.
#[derive(Clone, Copy)]
pub(crate) struct LaneProcessLimits {
    pub(crate) instances: NonZeroUsize,
    pub(crate) workers_per_class: NonZeroUsize,
    pub(crate) queued_per_class: NonZeroUsize,
    pub(crate) completed: NonZeroUsize,
    pub(crate) effect_limit: usize,
    pub(crate) base_timeout: Duration,
    pub(crate) retransmit: Duration,
}
impl LaneProcessLimits {
    fn validate(&self, now: Instant) -> Result<()> {
        if self.base_timeout.is_zero()
            || self.retransmit.is_zero()
            || self.effect_limit < 3 * reducer::MAX_EFFECTS_PER_STEP
        {
            return Err(bad("invalid native process clock/effect limits"));
        }
        LaneInstance::preflight_clock(now, self.base_timeout, self.retransmit)
    }
}

/// Separate physical classes prevent slow opening/body work monopolizing WAL workers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LaneWorkerClass {
    Opening,
    Wal,
    Body,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Opening,
    Wal,
    Body,
    OpeningDrain,
    ClosedDrain,
}
impl Kind {
    fn class(self) -> LaneWorkerClass {
        match self {
            Self::Opening | Self::OpeningDrain => LaneWorkerClass::Opening,
            Self::Wal => LaneWorkerClass::Wal,
            Self::Body | Self::ClosedDrain => LaneWorkerClass::Body,
        }
    }
}
struct IssuedWork {
    instance: HeightContextId,
    kind: Kind,
}
enum Job {
    Opening(LaneOpeningJob),
    Wal(LanePersistenceJob),
    Body(LaneBodyJob),
    OpeningDrain(LaneOpeningDrain),
    ClosedDrain(LaneClosedInstance),
}
enum Completed {
    Opening(LaneOpeningCompletion),
    Wal(LanePersistenceCompletion),
    Body(LaneBodyCompletion),
    OpeningDrained(LaneOpeningDrained),
    ClosedDrained(LaneClosedInstance),
}
struct PhysicalWork {
    issued: Arc<IssuedWork>,
    job: Job,
    #[cfg(test)]
    before: Option<Box<dyn FnOnce() + Send>>,
    #[cfg(test)]
    after: Option<Box<dyn FnOnce() + Send>>,
}
impl PhysicalWork {
    fn run(self, state: &State, guard: Arc<ConsensusOutputGuard>) -> LanePhysicalCompletion {
        #[cfg(test)]
        if let Some(before) = self.before {
            before();
        }
        let result = match self.job {
            Job::Opening(job) => Completed::Opening(job.run()),
            Job::Wal(job) => Completed::Wal(job.run()),
            Job::Body(job) => Completed::Body(job.run(state)),
            Job::OpeningDrain(job) => Completed::OpeningDrained(job.run()),
            Job::ClosedDrain(mut closed) => {
                // These destructors/physical owner releases never run on control.
                drop(closed.owner.wal.take());
                drop(closed.owner.body_store.take());
                Completed::ClosedDrained(closed)
            }
        };
        #[cfg(test)]
        if let Some(after) = self.after {
            after();
        }
        LanePhysicalCompletion {
            issued: self.issued,
            result: Some(result),
            guard,
            armed: true,
        }
    }
}
/// Routed private result. A foreign recipient returns this entire value intact.
#[must_use]
pub(crate) struct LanePhysicalCompletion {
    issued: Arc<IssuedWork>,
    result: Option<Completed>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl Drop for LanePhysicalCompletion {
    fn drop(&mut self) {
        if self.armed {
            self.guard.close_admission_for_restart();
        }
    }
}
impl std::fmt::Debug for LanePhysicalCompletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanePhysicalCompletion")
            .field("instance", &self.issued.instance)
            .field("kind", &self.issued.kind)
            .finish_non_exhaustive()
    }
}

/// Fixed bounded queues. Threads only perform physical jobs; no protocol scheduler.
/// Results may block a worker, never the process control loop or a publication lease.
pub(crate) struct LanePhysicalPool {
    state: Arc<State>,
    senders: BTreeMap<LaneWorkerClass, SyncSender<PhysicalWork>>,
    results: Option<Receiver<LanePhysicalCompletion>>,
    workers: Vec<JoinHandle<()>>,
    guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl Drop for LanePhysicalPool {
    fn drop(&mut self) {
        if self.armed {
            self.guard.close_admission_for_restart();
        }
    }
}
/// Explicit join owner to be waited outside the consensus control loop.
#[must_use]
pub(crate) struct LanePhysicalShutdown {
    workers: Vec<JoinHandle<()>>,
}
impl LanePhysicalShutdown {
    /// Join only after moving shutdown to a blocking service/shutdown owner.
    pub(crate) fn join(self) -> Result<()> {
        let mut failed = false;
        for worker in self.workers {
            failed |= worker.join().is_err();
        }
        if failed {
            Err(bad("native physical worker panicked"))
        } else {
            Ok(())
        }
    }
}
impl LanePhysicalPool {
    /// Construct once per process. No WAL/body open or fsync occurs here.
    pub(crate) fn new(
        state: Arc<State>,
        guard: Arc<ConsensusOutputGuard>,
        limits: LaneProcessLimits,
    ) -> Result<Self> {
        limits.validate(Instant::now())?;
        let (output, results) = mpsc::sync_channel(limits.completed.get());
        let mut senders = BTreeMap::new();
        let mut workers = Vec::new();
        for class in [
            LaneWorkerClass::Opening,
            LaneWorkerClass::Wal,
            LaneWorkerClass::Body,
        ] {
            let (send, receive) = mpsc::sync_channel::<PhysicalWork>(limits.queued_per_class.get());
            let receive = Arc::new(Mutex::new(receive));
            for _ in 0..limits.workers_per_class.get() {
                let (receive, output, state, worker_guard) = (
                    Arc::clone(&receive),
                    output.clone(),
                    Arc::clone(&state),
                    Arc::clone(&guard),
                );
                let worker = thread::Builder::new()
                    .name(format!("native-lane-{class:?}"))
                    .spawn(move || {
                        // Receive mutex is released before any filesystem work.
                        loop {
                            let received = match receive.lock() {
                                Ok(receiver) => receiver.recv(),
                                Err(_) => {
                                    worker_guard.close_admission_for_restart();
                                    return;
                                }
                            };
                            let Ok(work) = received else {
                                return;
                            };
                            let completed = work.run(&state, Arc::clone(&worker_guard));
                            if output.send(completed).is_err() {
                                worker_guard.close_admission_for_restart();
                                return;
                            }
                        }
                    });
                match worker {
                    Ok(worker) => workers.push(worker),
                    Err(error) => {
                        guard.close_admission_for_restart();
                        return Err(bad(error));
                    }
                }
            }
            senders.insert(class, send);
        }
        Ok(Self {
            state,
            senders,
            results: Some(results),
            workers,
            guard,
            armed: true,
        })
    }
    /// At most one result is moved; empty is a physical wait with owned workers.
    pub(crate) fn try_completion(&self) -> Result<Option<LanePhysicalCompletion>> {
        let receiver = self
            .results
            .as_ref()
            .ok_or_else(|| bad("native physical pool is shut down"))?;
        match receiver.try_recv() {
            Ok(value) => Ok(Some(value)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.guard.close_admission_for_restart();
                Err(bad("native physical results disconnected"))
            }
        }
    }
    /// Close process output, stop physical admission and return joins. Dropped queued/results retain
    /// their armed fail-stop semantics. No blocking join runs on this caller.
    pub(crate) fn shutdown(mut self) -> LanePhysicalShutdown {
        self.guard.close_admission_for_restart();
        self.senders.clear();
        drop(self.results.take());
        self.armed = false;
        LanePhysicalShutdown {
            workers: std::mem::take(&mut self.workers),
        }
    }
}

struct Pending {
    issued: Arc<IssuedWork>,
    queued: Option<PhysicalWork>,
}
enum Owner {
    Opening {
        ticket: Option<LaneOpening>,
        completed: Option<LaneOpeningCompletion>,
    },
    Active(Box<LaneInstance>),
    Closing(Box<LaneInstance>),
    DrainingOpening,
    DrainingClosed,
    Closed(LaneClosedInstance),
}
struct Entry {
    owner: Owner,
    work: BTreeMap<Kind, Pending>,
}
impl Entry {
    fn queue(&mut self, instance: HeightContextId, kind: Kind, job: Job) {
        let issued = Arc::new(IssuedWork { instance, kind });
        let work = PhysicalWork {
            issued: Arc::clone(&issued),
            job,
            #[cfg(test)]
            before: None,
            #[cfg(test)]
            after: None,
        };
        self.work.insert(
            kind,
            Pending {
                issued,
                queued: Some(work),
            },
        );
    }
    fn instance(&self) -> Option<&LaneInstance> {
        match &self.owner {
            Owner::Active(owner) | Owner::Closing(owner) => Some(owner),
            Owner::Closed(closed) => Some(&closed.owner),
            _ => None,
        }
    }
}
/// Closed nonproductive custody. Physical handles are already drained; all held
/// source, ingress, native records, Decision/Apply and untransferred output remain.
/// Taking this value releases table capacity, not any downstream obligation.
#[must_use]
pub(crate) struct LaneClosedInstance {
    owner: Box<LaneInstance>,
    // Written only by the consuming proof-bound terminal operation, after all
    // fallible checks and original retirement consumption have succeeded.
    published_terminal: bool,
}
impl LaneClosedInstance {
    /// Consume this original drained owner after actual global publication.
    /// Held Apply must first pass the separate shared-reducer settlement path.
    /// Every refusal returns the same owner and leaves its output fence armed.
    pub(crate) fn retire_published(
        mut self,
        published: &crate::state::PublishedNativeApply<'_>,
    ) -> std::result::Result<(), (Self, LaneInstanceError)> {
        let authorized = match self.owner.authorize_terminal_retirement(published) {
            Ok(authorized) => authorized,
            Err(error) => return Err((self, error)),
        };
        self.owner.consume_published_retirements(&authorized);
        self.published_terminal = true;
        Ok(())
    }
    /// Exact returned-but-unacknowledged control event retained through closure.
    pub(crate) fn unacknowledged_control(&self) -> Option<&reducer::Event> {
        self.owner.completion.as_ref()
    }
    /// Move one exact retirement from this same closed owner, without reopening.
    pub(crate) fn take_retirement(&mut self) -> Option<super::LaneRetirement> {
        self.owner.take_retirement()
    }
    /// Settle the original Apply after transfer, without reopening this signer.
    /// The caller still owns every other retained output and recovery obligation.
    pub(crate) fn settle_published_apply(
        &mut self,
        published: &crate::state::PublishedNativeApply<'_>,
    ) -> Result<super::LaneApplySettlement> {
        self.owner.settle_published_apply(published)
    }

    /// Read immutable witnesses/held effects for the future exact retirement consumer.
    pub(crate) fn instance(&self) -> &LaneInstance {
        &self.owner
    }
}
impl Drop for LaneClosedInstance {
    fn drop(&mut self) {
        // No generic discard can acknowledge Apply or terminal custody.
        if !self.published_terminal {
            self.owner.output_guard.close_admission_for_restart();
        }
    }
}
/// Counts actual owned entries/jobs, including closed obligations awaiting retrieval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LaneProcessOccupancy {
    pub(crate) instances: usize,
    pub(crate) queued: usize,
    pub(crate) transferred: usize,
    pub(crate) closed: usize,
    pub(crate) opening_results: usize,
    pub(crate) closing: usize,
    pub(crate) limit: usize,
}
/// Non-owning progress; exact retired custody stays inside the original instance.
pub(crate) enum LaneProcessProgress {
    Idle,
    QueueFull,
    Dispatched,
    OpeningAdopted,
    OpeningRetained,
    OpeningDraining,
    OpeningDrained,
    ClosedDrained,
    Persistence(LaneService),
    Body(LaneBodyProgress),
    Failed(String),
}
/// Sole process-lifetime owner, independent of global height/view rollover.
/// Construction is inactive until old production signers are removed atomically.
pub(crate) struct LaneProcessOwner {
    state: Arc<State>,
    kura: Arc<Kura>,
    guard: Arc<ConsensusOutputGuard>,
    limits: LaneProcessLimits,
    entries: BTreeMap<HeightContextId, Entry>,
    // Physical FIFO rotation only; no consensus readiness/view facts.
    last_dispatch: BTreeMap<LaneWorkerClass, HeightContextId>,
}
impl Drop for LaneProcessOwner {
    fn drop(&mut self) {
        if !self.entries.is_empty() {
            self.guard.close_admission_for_restart();
        }
    }
}
impl LaneProcessOwner {
    /// Reserve bounded custody in memory; this performs no filesystem operation.
    pub(crate) fn new(
        state: Arc<State>,
        guard: Arc<ConsensusOutputGuard>,
        limits: LaneProcessLimits,
    ) -> Result<Self> {
        limits.validate(Instant::now())?;
        Ok(Self {
            kura: state.kura_handle(),
            state,
            guard,
            limits,
            entries: BTreeMap::new(),
            last_dispatch: BTreeMap::new(),
        })
    }
    /// Exact borrowed owner; callers inspect source recovery, Decision/Apply and
    /// held effects without fabricating an acknowledgement or a second scheduler.
    pub(crate) fn instance(&self, id: HeightContextId) -> Option<&LaneInstance> {
        self.entries.get(&id).and_then(Entry::instance)
    }
    /// Only the original active owner accepts productive input. Closed custody
    /// remains inspectable through `instance` without becoming a fresh signer.
    pub(crate) fn is_productive(&self, id: HeightContextId) -> bool {
        self.entries
            .get(&id)
            .is_some_and(|entry| matches!(entry.owner, Owner::Active(_)))
    }
    /// Transfer one actual diagnostic effect to its explicit reporting consumer.
    /// This never acknowledges a Decision, Apply, body or transport effect.
    pub(crate) fn take_diagnostic(&mut self, id: HeightContextId) -> Option<reducer::Effect> {
        match self.entries.get_mut(&id).map(|entry| &mut entry.owner) {
            Some(Owner::Active(owner) | Owner::Closing(owner)) => owner.take_diagnostic(),
            Some(Owner::Closed(closed)) => closed.owner.take_diagnostic(),
            _ => None,
        }
    }
    /// Settle the exact retained owner, including closed nonproductive custody.
    /// This neither reconstructs an instance nor opens a new signing authority.
    pub(crate) fn settle_published_apply(
        &mut self,
        id: HeightContextId,
        published: &crate::state::PublishedNativeApply<'_>,
    ) -> Result<Option<super::LaneApplySettlement>> {
        match self.entries.get_mut(&id).map(|entry| &mut entry.owner) {
            Some(Owner::Active(owner) | Owner::Closing(owner)) => {
                owner.settle_published_apply(published).map(Some)
            }
            Some(Owner::Closed(closed)) => closed.owner.settle_published_apply(published).map(Some),
            _ => Ok(None),
        }
    }

    /// Actual owned identities, including opening/closing/drain occurrences.
    pub(crate) fn instance_ids(&self) -> impl Iterator<Item = HeightContextId> + '_ {
        self.entries.keys().copied()
    }
    /// Derive the next timer wake directly from current live instance clocks.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.entries
            .values()
            .filter_map(|entry| match &entry.owner {
                Owner::Active(owner) => Some(owner),
                _ => None,
            })
            .flat_map(|owner| {
                owner
                    .clock
                    .timeout
                    .into_iter()
                    .chain(std::iter::once(owner.clock.retransmit))
            })
            .min()
    }
    pub(crate) fn occupancy(&self) -> LaneProcessOccupancy {
        LaneProcessOccupancy {
            instances: self.entries.len(),
            queued: self
                .entries
                .values()
                .flat_map(|entry| entry.work.values())
                .filter(|work| work.queued.is_some())
                .count(),
            transferred: self
                .entries
                .values()
                .flat_map(|entry| entry.work.values())
                .filter(|work| work.queued.is_none())
                .count(),
            closed: self
                .entries
                .values()
                .filter(|entry| matches!(entry.owner, Owner::Closed(_)))
                .count(),
            opening_results: self
                .entries
                .values()
                .filter(|entry| {
                    matches!(
                        entry.owner,
                        Owner::Opening {
                            completed: Some(_),
                            ..
                        }
                    )
                })
                .count(),
            closing: self
                .entries
                .values()
                .filter(|entry| {
                    matches!(
                        entry.owner,
                        Owner::Closing(_) | Owner::DrainingClosed | Owner::DrainingOpening
                    )
                })
                .count(),
            limit: self.limits.instances.get(),
        }
    }
    /// Reserve one exact instance/key before one opening; duplicates, including
    /// closing/closed entries, are rejected before any physical job is created.
    pub(crate) fn reserve_opening(
        &mut self,
        observed: &VerifiedLaneContexts,
        verified: &VerifiedLaneContext,
        key: KeyPair,
        now: Instant,
    ) -> Result<()> {
        let id = verified.instance_id();
        if self.entries.contains_key(&id) {
            return Err(bad("native instance already reserved"));
        }
        if self.entries.len() >= self.limits.instances.get() {
            return Err(bad("native process instance capacity exhausted"));
        }
        let (ticket, job) = LaneInstance::prepare_opening(
            &self.state,
            observed,
            verified,
            Arc::clone(&self.kura),
            key,
            Arc::clone(&self.guard),
            now,
            self.limits.base_timeout,
            self.limits.retransmit,
            self.limits.effect_limit,
        )?;
        let mut entry = Entry {
            owner: Owner::Opening {
                ticket: Some(ticket),
                completed: None,
            },
            work: BTreeMap::new(),
        };
        entry.queue(id, Kind::Opening, Job::Opening(job));
        self.entries.insert(id, entry);
        Ok(())
    }
    /// Queue exactly one issued append without physical I/O. Body work never
    /// owns this admission slot, so timers/TC may progress while body work waits.
    pub(crate) fn prepare_persistence(
        &mut self,
        id: HeightContextId,
    ) -> Result<LaneProcessProgress> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| bad("unknown native instance"))?;
        if entry.work.contains_key(&Kind::Wal) {
            return Ok(LaneProcessProgress::Idle);
        }
        let Owner::Active(owner) = &mut entry.owner else {
            return Ok(LaneProcessProgress::Idle);
        };
        match owner.take_persistence_job()? {
            LanePersistenceLaunch::Job(job) => {
                entry.queue(id, Kind::Wal, Job::Wal(job));
                Ok(LaneProcessProgress::Idle)
            }
            LanePersistenceLaunch::Wait(_) => Ok(LaneProcessProgress::Idle),
        }
    }
    /// Queue the actual single body-store job; waits remain inside its instance.
    pub(crate) fn prepare_body(
        &mut self,
        id: HeightContextId,
        observed: &VerifiedLaneContexts,
    ) -> Result<LaneProcessProgress> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| bad("unknown native instance"))?;
        if entry.work.contains_key(&Kind::Body) {
            return Ok(LaneProcessProgress::Idle);
        }
        let Owner::Active(owner) = &mut entry.owner else {
            return Ok(LaneProcessProgress::Idle);
        };
        match owner.take_body_job(&self.state, observed)? {
            LaneBodyLaunch::Job(job) => {
                entry.queue(id, Kind::Body, Job::Body(job));
                Ok(LaneProcessProgress::Idle)
            }
            LaneBodyLaunch::Wait(wait) => {
                Ok(LaneProcessProgress::Body(LaneBodyProgress::Waiting(wait)))
            }
        }
    }
    /// Attempt one queued transfer. Queue-full restores the identical moved job.
    pub(crate) fn dispatch_one(
        &mut self,
        pool: &LanePhysicalPool,
        class: LaneWorkerClass,
    ) -> Result<LaneProcessProgress> {
        if !Arc::ptr_eq(&self.state, &pool.state) || !Arc::ptr_eq(&self.guard, &pool.guard) {
            return Err(bad("foreign native physical pool"));
        }
        let last = self.last_dispatch.get(&class).copied();
        let eligible = |entry: &Entry| {
            entry
                .work
                .values()
                .any(|pending| pending.issued.kind.class() == class && pending.queued.is_some())
        };
        let selected = self
            .entries
            .iter()
            .find(|(id, entry)| last.is_none_or(|last| **id > last) && eligible(entry))
            .or_else(|| self.entries.iter().find(|(_, entry)| eligible(entry)))
            .map(|(id, _)| *id);
        let Some(id) = selected else {
            return Ok(LaneProcessProgress::Idle);
        };
        let pending = self
            .entries
            .get_mut(&id)
            .and_then(|entry| {
                entry.work.values_mut().find(|pending| {
                    pending.issued.kind.class() == class && pending.queued.is_some()
                })
            })
            .ok_or_else(|| bad("selected physical job disappeared"))?;
        let work = pending
            .queued
            .take()
            .ok_or_else(|| bad("queued native work lost custody"))?;
        match pool.senders[&class].try_send(work) {
            Ok(()) => {
                self.last_dispatch.insert(class, id);
                Ok(LaneProcessProgress::Dispatched)
            }
            Err(TrySendError::Full(work)) => {
                pending.queued = Some(work);
                Ok(LaneProcessProgress::QueueFull)
            }
            Err(TrySendError::Disconnected(work)) => {
                pending.queued = Some(work);
                self.guard.close_admission_for_restart();
                Err(bad("native physical work queue disconnected"))
            }
        }
    }
    /// Restore physical ownership before any productive acknowledgement. Exact
    /// envelope ticket and inner job ticket both check; foreign results are returned.
    pub(crate) fn accept_completion(
        &mut self,
        mut completed: LanePhysicalCompletion,
        observed: &VerifiedLaneContexts,
    ) -> std::result::Result<LaneProcessProgress, (LaneInstanceError, LanePhysicalCompletion)> {
        let id = completed.issued.instance;
        let kind = completed.issued.kind;
        let Some(entry) = self.entries.get_mut(&id) else {
            return Err((bad("foreign native process completion"), completed));
        };
        if !entry.work.get(&kind).is_some_and(|pending| {
            pending.queued.is_none() && Arc::ptr_eq(&pending.issued, &completed.issued)
        }) {
            return Err((bad("foreign native process job ticket"), completed));
        }
        let Some(result) = completed.result.take() else {
            self.guard.close_admission_for_restart();
            return Err((bad("physical result lost custody"), completed));
        };
        let outcome = match result {
            Completed::Opening(result) => match &mut entry.owner {
                Owner::Opening { completed, .. } if completed.is_none() => {
                    *completed = Some(result);
                    Ok(LaneProcessProgress::OpeningRetained)
                }
                _ => Err((
                    bad("opening result has no reserved owner"),
                    Completed::Opening(result),
                )),
            },
            Completed::Wal(result) => match &mut entry.owner {
                Owner::Active(owner) | Owner::Closing(owner) => owner
                    .finish_persistence_job(result)
                    .map(LaneProcessProgress::Persistence)
                    .map_err(|(error, result)| (error, Completed::Wal(result))),
                _ => Err((
                    bad("WAL result has no exact instance"),
                    Completed::Wal(result),
                )),
            },
            Completed::Body(result) => match &mut entry.owner {
                Owner::Active(owner) | Owner::Closing(owner) => owner
                    .finish_body_job(result, &self.state, observed)
                    .map(LaneProcessProgress::Body)
                    .map_err(|(error, result)| (error, Completed::Body(result))),
                _ => Err((
                    bad("body result has no exact instance"),
                    Completed::Body(result),
                )),
            },
            Completed::OpeningDrained(result) => {
                if matches!(entry.owner, Owner::DrainingOpening) && result.instance_id() == id {
                    Ok(LaneProcessProgress::OpeningDrained)
                } else {
                    Err((
                        bad("opening drain has no exact owner"),
                        Completed::OpeningDrained(result),
                    ))
                }
            }
            Completed::ClosedDrained(closed) => {
                if matches!(entry.owner, Owner::DrainingClosed)
                    && closed.owner.verified.instance_id() == id
                {
                    entry.owner = Owner::Closed(closed);
                    Ok(LaneProcessProgress::ClosedDrained)
                } else {
                    Err((
                        bad("closed drain has no exact owner"),
                        Completed::ClosedDrained(closed),
                    ))
                }
            }
        };
        match outcome {
            Ok(progress) => {
                entry.work.remove(&kind);
                if matches!(progress, LaneProcessProgress::OpeningDrained) {
                    self.entries.remove(&id);
                }
                completed.armed = false;
                Ok(progress)
            }
            Err((error, result)) => {
                self.guard.close_admission_for_restart();
                completed.result = Some(result);
                Err((error, completed))
            }
        }
    }
    /// A stale observation retains ticket and completion. Adopting requires the
    /// existing exact State/Kura/frozen-key lease, never a copied current context.
    pub(crate) fn settle_opening(
        &mut self,
        id: HeightContextId,
        observed: &VerifiedLaneContexts,
    ) -> Result<LaneProcessProgress> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| bad("unknown native instance"))?;
        let Owner::Opening { ticket, completed } = &mut entry.owner else {
            return Ok(LaneProcessProgress::Idle);
        };
        if completed.is_none() {
            return Ok(LaneProcessProgress::OpeningRetained);
        }
        let Some(opening) = ticket.take() else {
            self.guard.close_admission_for_restart();
            return Err(bad("opening ticket lost"));
        };
        let Some(completed) = completed.take() else {
            self.guard.close_admission_for_restart();
            return Err(bad("opening completion lost"));
        };
        match opening.adopt(&self.state, observed, completed) {
            Ok(LaneOpeningAdoption::Opened(owner)) => {
                entry.owner = Owner::Active(owner);
                Ok(LaneProcessProgress::OpeningAdopted)
            }
            Ok(LaneOpeningAdoption::ObservationChanged {
                opening,
                completion,
            }) => {
                entry.owner = Owner::Opening {
                    ticket: Some(opening),
                    completed: Some(completion),
                };
                Ok(LaneProcessProgress::OpeningRetained)
            }
            Ok(LaneOpeningAdoption::Closed(drain)) => {
                entry.owner = Owner::DrainingOpening;
                entry.queue(id, Kind::OpeningDrain, Job::OpeningDrain(drain));
                Ok(LaneProcessProgress::OpeningDraining)
            }
            Ok(LaneOpeningAdoption::Failed { error, drain }) => {
                entry.owner = Owner::DrainingOpening;
                entry.queue(id, Kind::OpeningDrain, Job::OpeningDrain(drain));
                Ok(LaneProcessProgress::Failed(error.to_string()))
            }
            Err((error, opening, completed)) => {
                entry.owner = Owner::Opening {
                    ticket: Some(opening),
                    completed: Some(completed),
                };
                Err(error)
            }
        }
    }
    /// Observe authenticated closure without consuming any source/output/Apply.
    /// Same immutable membership across unrelated global advances leaves owner
    /// identity, frozen key, clocks, native records and physical tickets untouched.
    pub(crate) fn reconcile(&mut self, observed: &VerifiedLaneContexts) -> LaneCurrentGate {
        let _lease = self.state.consensus_publication_lease();
        if !observed.is_current(&self.state) {
            return LaneCurrentGate::ObservationChanged;
        }
        for entry in self.entries.values_mut() {
            let close = match &entry.owner {
                Owner::Active(owner) => {
                    owner.current_gate(&self.state, observed) == LaneCurrentGate::InstanceClosed
                }
                _ => false,
            };
            if close {
                let owner = std::mem::replace(&mut entry.owner, Owner::DrainingClosed);
                if let Owner::Active(owner) = owner {
                    entry.owner = Owner::Closing(owner);
                }
            }
        }
        LaneCurrentGate::Current
    }
    /// Once every admitted physical job has returned, move handles for off-loop
    /// release. Completed-but-unacknowledged effects remain inside the closed token.
    pub(crate) fn prepare_closed_drain(
        &mut self,
        id: HeightContextId,
    ) -> Result<LaneProcessProgress> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| bad("unknown native instance"))?;
        if !entry.work.is_empty() {
            return Ok(LaneProcessProgress::Idle);
        }
        let Owner::Closing(owner) = &entry.owner else {
            return Ok(LaneProcessProgress::Idle);
        };
        if owner.persistence.is_some()
            || owner.body.worker_in_flight()
            || owner.wal.is_none()
            || owner.body_store.is_none()
        {
            self.guard.close_admission_for_restart();
            return Err(bad("closed instance physical owner is missing"));
        }
        let Owner::Closing(owner) = std::mem::replace(&mut entry.owner, Owner::DrainingClosed)
        else {
            self.guard.close_admission_for_restart();
            return Err(bad("closed owner changed during exclusive transfer"));
        };
        entry.queue(
            id,
            Kind::ClosedDrain,
            Job::ClosedDrain(LaneClosedInstance {
                owner,
                published_terminal: false,
            }),
        );
        Ok(LaneProcessProgress::Idle)
    }
    /// Transfer retained closed obligations to an explicit downstream owner. No
    /// transport acknowledgement or ApplicationCompleted is synthesized here.
    pub(crate) fn take_closed(&mut self, id: HeightContextId) -> Option<LaneClosedInstance> {
        if !self
            .entries
            .get(&id)
            .is_some_and(|entry| matches!(entry.owner, Owner::Closed(_)) && entry.work.is_empty())
        {
            return None;
        }
        match self.entries.remove(&id)?.owner {
            Owner::Closed(closed) => Some(closed),
            _ => None,
        }
    }
    /// Consume the same retained retirement regardless of productive/closed state.
    /// Physical draining keeps the original owner in flight until it returns.
    pub(crate) fn take_retirement(&mut self, id: HeightContextId) -> Option<super::LaneRetirement> {
        match &mut self.entries.get_mut(&id)?.owner {
            Owner::Active(owner) | Owner::Closing(owner) => owner.take_retirement(),
            Owner::Closed(closed) => closed.take_retirement(),
            _ => None,
        }
    }
    fn active(&mut self, id: HeightContextId) -> Result<&mut LaneInstance> {
        match self.entries.get_mut(&id).map(|entry| &mut entry.owner) {
            Some(Owner::Active(owner)) => Ok(owner),
            _ => Err(bad("native instance is not productively open")),
        }
    }
    /// Caller retains borrowed ingress on Backpressured; this table makes no
    /// transport dequeue/acknowledgement claim. Original proposals stay in instance.
    pub(crate) fn offer(
        &mut self,
        id: HeightContextId,
        observed: &VerifiedLaneContexts,
        message: &LaneMessageV1,
    ) -> Result<LaneInputOutcome> {
        let state = Arc::clone(&self.state);
        self.active(id)?.offer(&state, observed, message)
    }
    /// Advance one existing tagged clock; physical work/output cannot block this.
    pub(crate) fn poll_clock(
        &mut self,
        id: HeightContextId,
        observed: &VerifiedLaneContexts,
        now: Instant,
    ) -> Result<LaneInputOutcome> {
        let state = Arc::clone(&self.state);
        self.active(id)?.poll_clock(&state, observed, now)
    }
    /// One same-reducer control action; retired custody remains capacity-accounted.
    pub(crate) fn service_one(
        &mut self,
        id: HeightContextId,
        observed: &VerifiedLaneContexts,
        now: Instant,
    ) -> Result<LaneService> {
        let state = Arc::clone(&self.state);
        self.active(id)?.service_one(&state, observed, now)
    }
    /// Settle already-returned body work without another physical operation.
    pub(crate) fn service_body_completion(
        &mut self,
        id: HeightContextId,
        observed: &VerifiedLaneContexts,
    ) -> Result<LaneBodyProgress> {
        let state = Arc::clone(&self.state);
        self.active(id)?.service_body_completion(&state, observed)
    }
    /// Channel acceptance transfers this exact packet beyond table ownership.
    /// Its eventual delivery/retirement remains the transport consumer's obligation.
    pub(crate) fn flush_one(
        &mut self,
        id: HeightContextId,
        observed: &VerifiedLaneContexts,
        sender: &SyncSender<LaneOutbound>,
    ) -> Result<LaneService> {
        let state = Arc::clone(&self.state);
        self.active(id)?.flush_one(&state, observed, sender)
    }
    /// Existing authenticated global source recovery, never a native fetch protocol.
    pub(crate) fn complete_source_recovery(
        &mut self,
        id: HeightContextId,
        request: &crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyRequest,
        response: &crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyResponse,
    ) -> Result<()> {
        self.active(id)?.complete_source_recovery(request, response)
    }
    /// Whether an original physical job is still queued, used by a one-shot test hold.
    #[cfg(test)]
    pub(crate) fn has_queued_job_for_test(
        &self,
        id: HeightContextId,
        class: LaneWorkerClass,
    ) -> bool {
        self.entries.get(&id).is_some_and(|entry| {
            entry
                .work
                .values()
                .any(|pending| pending.issued.kind.class() == class && pending.queued.is_some())
        })
    }

    /// Borrow a real queued opening/body job's retained immutable context.
    #[cfg(test)]
    pub(crate) fn queued_context_for_test(
        &self,
        id: HeightContextId,
        class: LaneWorkerClass,
    ) -> Option<&Arc<VerifiedLaneContext>> {
        self.entries.get(&id)?.work.values().find_map(|pending| {
            if pending.issued.kind.class() != class {
                return None;
            }
            match &pending.queued.as_ref()?.job {
                Job::Opening(job) => Some(job.context_for_test()),
                Job::Body(job) => Some(job.context_for_test()),
                _ => None,
            }
        })
    }

    /// Exhaust only this fixture instance's descriptor headroom, retaining all owners.
    #[cfg(test)]
    pub(crate) fn restrict_effect_capacity_to_retained_for_test(&mut self, id: HeightContextId) {
        let owner = self.active(id).expect("exact active fixture owner");
        assert!(owner.completion.is_none() && owner.persistence.is_none());
        owner.restrict_effect_capacity_to_retained_for_test();
    }

    /// Hold an actual completed operation before its private result is delivered.
    #[cfg(test)]
    pub(crate) fn hold_next_completion_for_test(
        &mut self,
        id: HeightContextId,
        class: LaneWorkerClass,
        after: impl FnOnce() + Send + 'static,
    ) -> Result<()> {
        let work = self
            .entries
            .get_mut(&id)
            .and_then(|entry| {
                entry.work.values_mut().find(|pending| {
                    pending.issued.kind.class() == class && pending.queued.is_some()
                })
            })
            .and_then(|pending| pending.queued.as_mut())
            .ok_or_else(|| bad("no queued physical owner to hold its completion"))?;
        work.after = Some(Box::new(after));
        Ok(())
    }

    /// Test control holds a real queued job before its physical operation.
    #[cfg(test)]
    pub(crate) fn hold_next_job_for_test(
        &mut self,
        id: HeightContextId,
        class: LaneWorkerClass,
        before: impl FnOnce() + Send + 'static,
    ) -> Result<()> {
        let work = self
            .entries
            .get_mut(&id)
            .and_then(|entry| {
                entry.work.values_mut().find(|pending| {
                    pending.issued.kind.class() == class && pending.queued.is_some()
                })
            })
            .and_then(|pending| pending.queued.as_mut())
            .ok_or_else(|| bad("no queued physical owner to hold"))?;
        work.before = Some(Box::new(before));
        Ok(())
    }
}
