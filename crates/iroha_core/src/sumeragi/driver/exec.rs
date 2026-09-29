//! Scheduling of execution, apply and payload building (`specs/sumeragi.md` §12.2, §12.3 O3,
//! O4) over one executor that runs one operation at a time:
//!
//! - every `Execute` is answered exactly once — `Valid`, `Invalid`, `Failed`, or `Cancelled`
//!   when a later `DiscardExecution` of its height left it out (a running job finishes first
//!   and is then answered `Cancelled`), or when its height was applied meanwhile;
//! - the most recent `Execute` runs first; one whose parent post-state the executor does not
//!   hold is *parked* (never `Failed`) until the parent is applied or executed;
//! - `CommitBlock`s apply strictly in height order in three steps — prepare (the cached
//!   post-state or a re-execution; an execution of the block in flight finishes first and is
//!   reused, an `Execute` of it still queued is answered from the apply), durable append to the
//!   block store, commit — and `BlockApplied` carries the applied header; a local commitment
//!   that differs from the certified one is reported as `ApplyDiverged` and apply stops; local
//!   retryable refusals retain the original owner and back off; consuming failures halt;
//! - once prepared, and while a failed step backs off, a commit runs alone: no other executor
//!   call comes between its prepare and its commit (the executor may hold a single live
//!   overlay); a failed commit is retried after a fresh prepare, without a second append or
//!   resetting its failure backoff;
//! - `BuildPayload` for height `h` runs only after `h − 1` is applied (the builder filters the
//!   applied transactions), and `PayloadReady{req}` follows at most once an `EMPTY` answer;
//! - the queues other than the `Execute`s are bounded: discards of one height merge (keeping
//!   what both keep), and rejections are deduplicated and capped (the oldest go first: the
//!   quarantine is best effort).

use std::{collections::VecDeque, sync::Arc};

use iroha_sumeragi::{
    api::{ApplicationControlContext, ControlWitnessContext, Event, ExecOutcome, HaltReason},
    message::{ApplicationControl, Block, Qc},
    types::{AppliedConfig, ControlWitness, Hash32, MAX_COMMITTEE_SIZE, Millis, PublicKey},
};

use super::{persist::Backoff, traits::PublicationError};

/// Rejections kept while the executor is busy (the oldest are dropped beyond).
const MAX_REJECTS: usize = 64;

/// A committed block waiting to be applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    /// The block.
    pub block: Block,
    /// Its `CommitQC` (carries the block hash and the certified result).
    pub qc: Qc,
}

/// An operation for the executor thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecOp {
    /// Execute a block on its parent's post-state.
    Execute {
        /// The block.
        block: Arc<Block>,
        /// Its hash.
        block_hash: Hash32,
    },
    /// Drop the post-states at `height` other than `keep`.
    Discard {
        /// Height.
        height: u64,
        /// Kept blocks.
        keep: Vec<Hash32>,
    },
    /// Obtain the post-state of the next committed block.
    Prepare(Arc<Commit>),
    /// Durably append the next committed block to the block store.
    Append(Arc<Commit>),
    /// Make the prepared post-state the applied state.
    Commit(Arc<Commit>),
    /// Build the independent control witness for one exact fresh proposal.
    BuildControlWitness {
        /// Fresh core request id.
        req: u64,
        /// Exact view and parent source.
        context: ControlWitnessContext,
    },
    /// Drive one all-validator application producer for an applied parent.
    DriveApplicationControl(ApplicationControlContext),
    /// Reduce one bounded authenticated peer partial.
    ReceiveApplicationControl {
        /// P2P authenticated sender.
        from: PublicKey,
        /// Exact context and bounded application bytes.
        message: ApplicationControl,
    },
    /// Build a payload.
    Build {
        /// Request id.
        req: u64,
        /// Height.
        height: u64,
        /// View.
        view: u64,
        /// Size limit.
        max_bytes: u32,
        /// Execution budget hint.
        exec_budget_ms: u32,
    },
    /// Quarantine the transactions of an `Invalid` block.
    Reject {
        /// Height.
        height: u64,
        /// View.
        view: u64,
        /// Block hash.
        block_hash: Hash32,
    },
}

/// The executor thread's answer to the operation in flight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecDone {
    /// `Execute`: the outcome, or `None` if the parent post-state is not held.
    Executed(Option<ExecOutcome>),
    /// `Discard` done.
    Discarded,
    /// `Prepare`: the local commitment (`None`: not `Valid`), or a local failure.
    Prepared(Result<Option<Hash32>, PublicationError>),
    /// `Append`: whether the block is durable in the block store.
    Appended(bool),
    /// `Commit`: the original atomic epoch/configuration output, or a local failure.
    Committed(Result<Box<AppliedConfig>, PublicationError>),
    /// Independent exact control response; no empty fallback on local failure.
    ControlWitnessBuilt(Result<(ControlWitness, bool), PublicationError>),
    /// At most one source-bound own partial from the sole producer.
    ApplicationControlDriven(Result<Option<ApplicationControl>, PublicationError>),
    /// The application accepted/rejected one peer partial.
    ApplicationControlReceived(Result<(), PublicationError>),
    /// `Build`: the payload and its attestation flag.
    Built {
        /// Payload.
        payload: Vec<u8>,
        /// Commit-attestation flag (§3.7 A1).
        attest: bool,
    },
    /// `Reject` done.
    Rejected,
}

#[derive(Clone, Debug)]
struct Job {
    req: u64,
    block_hash: Hash32,
    block: Arc<Block>,
    cancelled: bool,
}

impl Job {
    fn height(&self) -> u64 {
        self.block.header.height
    }
}

#[derive(Debug)]
enum Running {
    Execute(Job),
    Discard,
    Prepare,
    Append,
    Commit,
    Build(u64),
    BuildControl(ControlBuild),
    DriveControl(ApplicationControlContext),
    ReceiveControl(u64),
    Reject,
}

/// Where the head of the commit queue is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Fresh,
    Prepared,
    Appended,
}

#[derive(Clone, Copy, Debug)]
struct BuildRequest {
    req: u64,
    height: u64,
    view: u64,
    max_bytes: u32,
    exec_budget_ms: u32,
}

#[derive(Clone, Copy, Debug)]
struct ControlBuild {
    req: u64,
    context: ControlWitnessContext,
    retry_at: Millis,
    failures: u32,
}

/// The execution scheduler of one instance.
#[derive(Debug)]
pub struct ExecSched {
    /// Queued `Execute`s; the last is the most recent.
    jobs: Vec<Job>,
    parked: Vec<Job>,
    /// Queued jobs of the block being applied, answered from its apply.
    merged: Vec<Job>,
    running: Option<Running>,
    commits: VecDeque<Arc<Commit>>,
    stage: Stage,
    /// The head commit is durable in the block store (a re-prepare skips the append).
    appended: bool,
    build: Option<BuildRequest>,
    control_build: Option<ControlBuild>,
    control_round: Option<(u64, u64)>,
    control_drive: Option<ApplicationControlContext>,
    /// At most one pending partial per authenticated committee sender, with a hard protocol cap.
    control_inbox: VecDeque<(PublicKey, ApplicationControl)>,
    /// Alternate one control operation with ordinary work so partial ingress cannot starve it.
    prefer_control: bool,
    /// Rotate drive, ingress and due builds within the control class under saturation.
    control_cursor: u8,
    rejects: VecDeque<(u64, u64, Hash32)>,
    discards: VecDeque<(u64, Vec<Hash32>)>,
    applied: u64,
    halted: Option<HaltReason>,
    failures: u32,
    retry_at: Option<Millis>,
    backoff: Backoff,
    pending_ready: Option<u64>,
    /// An includable transaction arrived while a build ran: its queue snapshot may predate it.
    arrived_during_build: bool,
    events: Vec<Event>,
}

impl ExecSched {
    /// A scheduler whose executor has applied `applied`.
    pub fn new(applied: u64, backoff: Backoff) -> Self {
        Self {
            jobs: Vec::new(),
            parked: Vec::new(),
            merged: Vec::new(),
            running: None,
            commits: VecDeque::new(),
            stage: Stage::Fresh,
            appended: false,
            build: None,
            control_build: None,
            control_round: None,
            control_drive: None,
            control_inbox: VecDeque::new(),
            prefer_control: true,
            control_cursor: 0,
            rejects: VecDeque::new(),
            discards: VecDeque::new(),
            applied,
            halted: None,
            failures: 0,
            retry_at: None,
            backoff,
            pending_ready: None,
            arrived_during_build: false,
            events: Vec::new(),
        }
    }

    /// Highest applied height.
    pub fn applied(&self) -> u64 {
        self.applied
    }

    fn answer(&mut self, job: &Job, outcome: ExecOutcome) {
        self.events.push(Event::Executed {
            block_hash: job.block_hash,
            req: job.req,
            outcome,
        });
    }

    /// `Execute{block, req}`: queued (most recent first); answered `Cancelled` at once if its
    /// height is already applied.
    pub fn execute(&mut self, req: u64, block_hash: Hash32, block: Block) {
        if self.halted.is_some() {
            self.events.push(Event::Executed {
                req,
                block_hash,
                outcome: ExecOutcome::Cancelled,
            });
            return;
        }
        let job = Job {
            req,
            block_hash,
            block: Arc::new(block),
            cancelled: false,
        };
        if job.height() <= self.applied {
            self.answer(&job, ExecOutcome::Cancelled);
        } else {
            self.jobs.push(job);
        }
    }

    /// `DiscardExecution{height, keep}`: waiting jobs left out are answered `Cancelled` now, a
    /// running one when it finishes; the executor drops the post-states.
    pub fn discard(&mut self, height: u64, keep: Vec<Hash32>) {
        if self.halted.is_some() {
            return;
        }
        let out = |job: &Job| job.height() == height && !keep.contains(&job.block_hash);
        let mut cancelled = Vec::new();
        for list in [&mut self.jobs, &mut self.parked] {
            list.retain(|job| {
                if out(job) {
                    cancelled.push(job.clone());
                    false
                } else {
                    true
                }
            });
        }
        for job in &cancelled {
            self.answer(job, ExecOutcome::Cancelled);
        }
        if let Some(Running::Execute(job)) = &mut self.running
            && out(job)
        {
            job.cancelled = true;
        }
        // Discards run before any execution, so two of one height in a row keep what both keep.
        match self.discards.iter_mut().find(|(h, _)| *h == height) {
            Some((_, kept)) => kept.retain(|bh| keep.contains(bh)),
            None => self.discards.push_back((height, keep)),
        }
    }

    /// `CommitBlock` (after the O2 barrier; the core emits them in height order).
    pub fn commit(&mut self, block: Block, qc: Qc) {
        if self.halted.is_some() {
            return;
        }
        self.commits.push_back(Arc::new(Commit { block, qc }));
    }

    /// `BuildPayload`: supersedes an unanswered older request.
    pub fn build(&mut self, req: u64, height: u64, view: u64, max_bytes: u32, exec_budget_ms: u32) {
        if self.halted.is_some() {
            return;
        }
        self.build = Some(BuildRequest {
            req,
            height,
            view,
            max_bytes,
            exec_budget_ms,
        });
    }

    /// Supersede only with a new exact fresh-proposal request. A refusal retains this source.
    pub fn build_control(&mut self, req: u64, context: ControlWitnessContext) {
        if self.halted.is_some() || context.height <= self.applied {
            return;
        }
        self.control_round = Some((context.height, context.view));
        self.control_build = Some(ControlBuild {
            req,
            context,
            retry_at: 0,
            failures: 0,
        });
    }

    /// Cancel retry work that no longer belongs to the core's live signing round. Partial
    /// production is view independent but cannot outlive its current height or a local halt.
    pub fn retain_control_round(&mut self, round: Option<(u64, u64)>) {
        self.control_round = round;
        if self
            .control_build
            .is_some_and(|build| Some((build.context.height, build.context.view)) != round)
        {
            self.control_build = None;
        }
        if self
            .control_drive
            .is_some_and(|context| round.is_none_or(|(height, _)| height != context.height))
        {
            self.control_drive = None;
        }
        self.control_inbox.retain(|(_, message)| {
            round.is_some_and(|(height, _)| height == message.context.height)
        });
    }

    /// Bounded periodic drive requests coalesce; the application owns retransmission state.
    pub fn drive_control(&mut self, context: ApplicationControlContext) {
        if self.halted.is_some() || context.height <= self.applied {
            return;
        }
        self.control_drive = Some(context);
        self.control_inbox
            .retain(|(_, message)| message.context == context);
    }

    /// Keep one partial per authenticated sender; source/sender checks precede this call in
    /// the core and are repeated by the application before reducing any cryptographic state.
    pub fn receive_control(&mut self, from: PublicKey, message: ApplicationControl) {
        if self.halted.is_some() || message.context.height <= self.applied {
            return;
        }
        if let Some((_, pending)) = self.control_inbox.iter_mut().find(|(key, _)| key == &from) {
            *pending = message;
        } else if self.control_inbox.len() < MAX_COMMITTEE_SIZE {
            self.control_inbox.push_back((from, message));
        }
    }

    fn next_control(&mut self, now: Millis) -> Option<ExecOp> {
        self.control_round?;
        let next = self.applied.checked_add(1)?;
        self.control_inbox
            .retain(|(_, message)| message.context.height >= next);
        if self
            .control_drive
            .is_some_and(|context| context.height < next)
        {
            self.control_drive = None;
        }
        if self
            .control_build
            .is_some_and(|build| build.context.height < next)
        {
            self.control_build = None;
        }
        for offset in 0..3 {
            let kind = (self.control_cursor + offset) % 3;
            let op = match kind {
                0 => self
                    .control_drive
                    .filter(|context| context.height == next)
                    .map(|context| {
                        self.control_drive = None;
                        self.running = Some(Running::DriveControl(context));
                        ExecOp::DriveApplicationControl(context)
                    }),
                1 => self
                    .control_inbox
                    .iter()
                    .position(|(_, message)| message.context.height == next)
                    .and_then(|index| self.control_inbox.remove(index))
                    .map(|(from, message)| {
                        self.running = Some(Running::ReceiveControl(message.context.height));
                        ExecOp::ReceiveApplicationControl { from, message }
                    }),
                _ => self
                    .control_build
                    .filter(|build| build.context.height == next && now >= build.retry_at)
                    .map(|build| {
                        self.control_build = None;
                        self.running = Some(Running::BuildControl(build));
                        ExecOp::BuildControlWitness {
                            req: build.req,
                            context: build.context,
                        }
                    }),
            };
            if op.is_some() {
                self.control_cursor = (kind + 1) % 3;
                return op;
            }
        }
        None
    }

    /// `PayloadRejected` (once per block; the oldest go beyond [`MAX_REJECTS`]).
    pub fn reject(&mut self, height: u64, view: u64, block_hash: Hash32) {
        if self.halted.is_some() {
            return;
        }
        if self.rejects.iter().any(|(_, _, bh)| *bh == block_hash) {
            return;
        }
        if self.rejects.len() >= MAX_REJECTS {
            self.rejects.pop_front();
        }
        self.rejects.push_back((height, view, block_hash));
    }

    /// An includable transaction arrived: `PayloadReady{req}` if the latest build was `EMPTY`
    /// (at most once per request). During a build the arrival is remembered: an `EMPTY` answer
    /// may have read the queue before it, and is followed by `PayloadReady` at once (E55).
    pub fn transactions_available(&mut self) {
        if self.halted.is_some() {
            return;
        }
        if let Some(req) = self.pending_ready.take() {
            self.events.push(Event::PayloadReady { req });
        } else if matches!(self.running, Some(Running::Build(_))) {
            self.arrived_during_build = true;
        }
    }

    /// The next operation for the executor, if it is idle. A commit that is prepared or
    /// appended, or whose failed step backs off, runs alone (its next step once due, nothing
    /// else meanwhile). Otherwise: discards, then the apply of the next committed block, then a
    /// build whose parent is applied, then rejections, then the most recent `Execute`.
    pub fn next(&mut self, now: Millis) -> Option<ExecOp> {
        if self.halted.is_some() || self.running.is_some() {
            return None;
        }
        let committing = self.halted.is_none()
            && !self.commits.is_empty()
            && (self.stage != Stage::Fresh || self.retry_at.is_some());
        if committing {
            let due = self.retry_at.is_none_or(|at| at <= now);
            return if due { self.commit_step() } else { None };
        }
        if let Some((height, keep)) = self.discards.pop_front() {
            self.running = Some(Running::Discard);
            return Some(ExecOp::Discard { height, keep });
        }
        if self.halted.is_none() && !self.commits.is_empty() {
            return self.commit_step();
        }
        if self.prefer_control
            && let Some(op) = self.next_control(now)
        {
            self.prefer_control = false;
            return Some(op);
        }
        if let Some(build) = self.build
            && build.height <= self.applied.saturating_add(1)
        {
            self.build = None;
            self.prefer_control = true;
            self.running = Some(Running::Build(build.req));
            self.arrived_during_build = false;
            return Some(ExecOp::Build {
                req: build.req,
                height: build.height,
                view: build.view,
                max_bytes: build.max_bytes,
                exec_budget_ms: build.exec_budget_ms,
            });
        }
        if let Some((height, view, block_hash)) = self.rejects.pop_front() {
            self.prefer_control = true;
            self.running = Some(Running::Reject);
            return Some(ExecOp::Reject {
                height,
                view,
                block_hash,
            });
        }
        let Some(job) = self.jobs.pop() else {
            self.prefer_control = false;
            return self.next_control(now);
        };
        self.prefer_control = true;
        let op = ExecOp::Execute {
            block: Arc::clone(&job.block),
            block_hash: job.block_hash,
        };
        self.running = Some(Running::Execute(job));
        Some(op)
    }

    /// The next step of the head commit.
    fn commit_step(&mut self) -> Option<ExecOp> {
        let commit = self.commits.front().cloned()?;
        let (running, op) = match self.stage {
            Stage::Fresh => {
                let bh = commit.qc.block_hash;
                for list in [&mut self.jobs, &mut self.parked] {
                    let (same, rest): (Vec<Job>, Vec<Job>) =
                        list.drain(..).partition(|job| job.block_hash == bh);
                    *list = rest;
                    self.merged.extend(same);
                }
                (Running::Prepare, ExecOp::Prepare(commit))
            }
            Stage::Prepared => (Running::Append, ExecOp::Append(commit)),
            Stage::Appended => (Running::Commit, ExecOp::Commit(commit)),
        };
        self.running = Some(running);
        Some(op)
    }

    /// Jobs whose parent is `parent` may run now: back in the queue, behind newer ones.
    fn unpark(&mut self, parent: &Hash32) {
        let (ready, rest): (Vec<Job>, Vec<Job>) = self
            .parked
            .drain(..)
            .partition(|job| job.block.header.parent_hash == *parent);
        self.parked = rest;
        let newer = std::mem::replace(&mut self.jobs, ready);
        self.jobs.extend(newer);
    }

    fn retry(&mut self, now: Millis, what: &str, reason: &str) {
        self.failures = self.failures.saturating_add(1);
        self.retry_at = Some(now.saturating_add(self.backoff.delay(self.failures)));
        iroha_logger::warn!(%reason, step = what, "sumeragi apply failed; retrying");
    }

    /// Why execution scheduling stopped; a halt never schedules a publication retry.
    pub fn halted(&self) -> Option<HaltReason> {
        self.halted
    }

    fn require_recovery(&mut self, height: u64, reason: &str) {
        self.halted = Some(HaltReason::PublicationRecoveryRequired { height });
        self.retry_at = None;
        self.pending_ready = None;
        self.build = None;
        self.control_build = None;
        self.control_round = None;
        self.control_drive = None;
        self.control_inbox.clear();
        self.rejects.clear();
        self.discards.clear();
        // Preserve the committed head for diagnostics; no owner is re-entered after this point.
        self.events
            .push(Event::PublicationRecoveryRequired { height });
        for jobs in [
            std::mem::take(&mut self.jobs),
            std::mem::take(&mut self.parked),
            std::mem::take(&mut self.merged),
        ] {
            for job in jobs {
                self.answer(&job, ExecOutcome::Cancelled);
            }
        }
        iroha_logger::error!(height, %reason, "sumeragi publication requires recovery; stopped scheduling");
    }

    /// The executor answered the operation in flight. Returns the height newly applied, if
    /// any (its stored bodies can be pruned).
    #[allow(clippy::too_many_lines)] // one arm per operation
    pub fn done(&mut self, now: Millis, done: ExecDone) -> Option<u64> {
        let running = self.running.take()?;
        match (running, done) {
            (Running::Execute(job), ExecDone::Executed(outcome)) => match outcome {
                _ if job.cancelled => self.answer(&job, ExecOutcome::Cancelled),
                None if job.height() <= self.applied => {
                    self.answer(&job, ExecOutcome::Cancelled);
                }
                None => self.parked.push(job),
                Some(outcome) => {
                    let valid = matches!(outcome, ExecOutcome::Valid(_));
                    self.answer(&job, outcome);
                    if valid {
                        self.unpark(&job.block_hash);
                    }
                }
            },
            (Running::Prepare, ExecDone::Prepared(result)) => {
                let commit = self.commits.front().cloned()?;
                match result {
                    Ok(Some(local)) if local == commit.qc.result => {
                        // Re-prepare after append is part of the same failed commit attempt.
                        // In particular, archive capture may still be pending after State
                        // publication: obtaining that retained result is not recovery.
                        if !self.appended {
                            self.failures = 0;
                        }
                        self.retry_at = None;
                        self.stage = if self.appended {
                            Stage::Appended
                        } else {
                            Stage::Prepared
                        };
                        for job in std::mem::take(&mut self.merged) {
                            self.answer(&job, ExecOutcome::Valid(local));
                        }
                    }
                    Ok(local) => {
                        // O3: the local state or executor disagrees with a certified result.
                        self.halted = Some(HaltReason::ApplyDiverged {
                            height: commit.block.header.height,
                        });
                        let outcome = local.map_or(ExecOutcome::Invalid, ExecOutcome::Valid);
                        for job in std::mem::take(&mut self.merged) {
                            self.answer(&job, outcome.clone());
                        }
                        self.events.push(Event::ApplyDiverged {
                            height: commit.block.header.height,
                            block_hash: commit.qc.block_hash,
                            local_result: local.unwrap_or(Hash32::ZERO),
                        });
                    }
                    Err(PublicationError::Retryable(reason)) => self.retry(now, "prepare", &reason),
                    Err(PublicationError::RecoveryRequired(reason)) => {
                        self.require_recovery(commit.block.header.height, &reason)
                    }
                }
            }
            (Running::Append, ExecDone::Appended(ok)) => {
                if ok {
                    self.failures = 0;
                    self.retry_at = None;
                    self.appended = true;
                    self.stage = Stage::Appended;
                } else {
                    self.retry(now, "append", "block store write failed");
                }
            }
            (Running::Commit, ExecDone::Committed(result)) => match result {
                Ok(config) => {
                    let commit = self.commits.pop_front()?;
                    self.failures = 0;
                    self.retry_at = None;
                    self.stage = Stage::Fresh;
                    self.appended = false;
                    let height = commit.block.header.height;
                    self.applied = height;
                    self.events.push(Event::BlockApplied {
                        height,
                        block_hash: commit.qc.block_hash,
                        header: Box::new(commit.block.header.clone()),
                        config: *config,
                    });
                    // Requests of applied heights are moot: answered, never left waiting.
                    let applied = self.applied;
                    for list in [&mut self.jobs, &mut self.parked] {
                        let (stale, rest): (Vec<Job>, Vec<Job>) =
                            list.drain(..).partition(|job| job.height() <= applied);
                        *list = rest;
                        for job in stale {
                            let outcome = if job.block_hash == commit.qc.block_hash {
                                ExecOutcome::Valid(commit.qc.result)
                            } else {
                                ExecOutcome::Cancelled
                            };
                            self.events.push(Event::Executed {
                                block_hash: job.block_hash,
                                req: job.req,
                                outcome,
                            });
                        }
                    }
                    self.unpark(&commit.qc.block_hash);
                    return Some(height);
                }
                Err(PublicationError::Retryable(reason)) => {
                    // Re-prepare the retained original owner; the append is kept.
                    self.stage = Stage::Fresh;
                    self.retry(now, "commit", &reason);
                }
                Err(PublicationError::RecoveryRequired(reason)) => {
                    if let Some(commit) = self.commits.front() {
                        self.require_recovery(commit.block.header.height, &reason);
                    }
                }
            },
            (Running::BuildControl(mut build), ExecDone::ControlWitnessBuilt(result)) => {
                match result {
                    Ok(_)
                        if self.control_round
                            != Some((build.context.height, build.context.view)) => {}
                    Ok((witness, attest)) => self.events.push(Event::ControlWitnessBuilt {
                        req: build.req,
                        context: build.context,
                        witness,
                        attest,
                    }),
                    Err(PublicationError::Retryable(_))
                        if self.control_build.is_none()
                            && self.control_round
                                == Some((build.context.height, build.context.view)) =>
                    {
                        build.failures = build.failures.saturating_add(1);
                        build.retry_at = now.saturating_add(self.backoff.delay(build.failures));
                        self.control_build = Some(build);
                    }
                    Err(PublicationError::Retryable(_)) => {} // superseded by a newer exact request
                    Err(PublicationError::RecoveryRequired(reason)) => {
                        self.require_recovery(build.context.height, &reason)
                    }
                }
            }
            (Running::DriveControl(context), ExecDone::ApplicationControlDriven(result)) => {
                match result {
                    Ok(Some(message))
                        if message.context == context
                            && self
                                .control_round
                                .is_some_and(|(height, _)| height == context.height) =>
                    {
                        self.events.push(Event::ApplicationControlBuilt { message })
                    }
                    Ok(_) | Err(PublicationError::Retryable(_)) => {} // next periodic drive retries the sole retained owner
                    Err(PublicationError::RecoveryRequired(reason)) => {
                        self.require_recovery(context.height, &reason)
                    }
                }
            }
            (Running::ReceiveControl(height), ExecDone::ApplicationControlReceived(result)) => {
                match result {
                    Ok(()) => {
                        if let Some(build) = self.control_build.as_mut() {
                            build.retry_at = now;
                        }
                    }
                    Err(PublicationError::Retryable(_)) => {} // authenticated sender periodically retries its same partial
                    Err(PublicationError::RecoveryRequired(reason)) => {
                        self.require_recovery(height, &reason)
                    }
                }
            }
            (Running::Build(req), ExecDone::Built { payload, attest }) => {
                let empty = payload.is_empty();
                let arrived = std::mem::take(&mut self.arrived_during_build);
                self.pending_ready = (empty && !arrived).then_some(req);
                self.events.push(Event::PayloadBuilt {
                    req,
                    payload,
                    attest,
                });
                // A non-empty answer may come after the core timed `req` out to `EMPTY` and
                // began the heartbeat wait: the transactions it carries are includable (E55).
                // The core ignores `PayloadReady` unless it is waiting on `req`.
                if !empty || arrived {
                    self.events.push(Event::PayloadReady { req });
                }
            }
            (Running::Discard, ExecDone::Discarded) | (Running::Reject, ExecDone::Rejected) => {}
            (running, done) => {
                iroha_logger::error!(?running, ?done, "sumeragi executor answer does not match");
                self.running = Some(running);
            }
        }
        None
    }

    /// When a failed apply step is due again (`Millis::MAX`: nothing to wait for). Only while
    /// the executor is idle, so the wakeup can always be acted on.
    pub fn wakeup(&self) -> Millis {
        if self.running.is_some() || self.halted.is_some() {
            return Millis::MAX;
        }
        if !self.commits.is_empty() {
            return self.retry_at.unwrap_or(Millis::MAX);
        }
        self.control_build
            .filter(|build| build.context.height == self.applied.saturating_add(1))
            .map_or(Millis::MAX, |build| build.retry_at)
    }

    /// The local events produced so far (answers for the core), in order.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// `Execute` requests not answered yet (queued, parked, merged or running).
    pub fn outstanding(&self) -> usize {
        self.jobs.len()
            + self.parked.len()
            + self.merged.len()
            + usize::from(matches!(self.running, Some(Running::Execute(_))))
    }

    /// Whether an operation is in flight.
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// Executor operations queued other than `Execute`s: commits, discards, rejections and a
    /// build.
    pub fn queued_ops(&self) -> usize {
        self.commits.len()
            + self.discards.len()
            + self.rejects.len()
            + usize::from(self.build.is_some())
            + usize::from(self.control_build.is_some())
            + usize::from(self.control_drive.is_some())
            + self.control_inbox.len()
    }
}
