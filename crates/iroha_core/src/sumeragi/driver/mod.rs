//! The node driver of the Sumeragi consensus core (`specs/sumeragi.md` §12, §13.5).
//!
//! One driver runs per consensus instance. Its [`Kernel`] owns the sans-IO
//! [`iroha_sumeragi::Core`] (constructed on the event-loop thread, since it is `!Send`), the
//! bounded ingress ([`ingress`]), the persist-before-effect barrier ([`barrier`]), the ordered
//! persistence queue ([`persist`]), the execution, apply and build scheduler ([`exec`]) and the
//! serving scheduler ([`serve`]). It performs no I/O: it turns the core's actions into
//! operations ([`Op`]) for three worker threads — persistence, executor, serving — and the
//! transport, and their completions back into core events. [`Driver::spawn`] runs the kernel on
//! its own thread with the workers over the backends of [`traits`]; the §13.5 conformance runs
//! the *same* kernel inside the `iroha_sumeragi` simulator, whose world performs the operations
//! on fake devices.
//!
//! Guarantees of §12.3: O1 (the core's actions take effect in order, exempt ones at once and
//! gated ones through the barrier), O2 ([`barrier`], with the durable watermark of the ordered
//! [`persist`] queue), O3/O4 ([`exec`]), O5 (a due `Tick` first, then local events, then
//! messages by class; nothing on the loop thread blocks on I/O, and released effects leave in
//! batches of [`MAX_OPS_PER_POLL`] with a due `Tick` handled in between), O6/O8 ([`ingress`];
//! held effects and serving are bounded too, [`barrier`], [`serve`]), O7 (the node's own keys
//! are never delivered to or addressed by it), O9 (every instance has its own threads, queues,
//! barrier and backends), O10 (the transport limit is checked at start against the chain
//! parameters, every frame is decoded within it, and a committed configuration that outgrows it
//! is reported, [`FrameLimitExceeded`]).
//!
//! Every backend call on a worker thread is guarded: a panic is a failed write, a failed apply
//! step or a missing entry, retried like an I/O error (§12.5). A thread that stops anyway, or a
//! worker that cannot be reached, stops the instance: the observer is told
//! ([`Observer::stopped`]) and the handle reports it (`ready()` false, [`DriverHandle::stopped`]).
//!
//! Production backends: the P2P `Net` and ingress router (`sumeragi::net`), the file record and
//! body stores (`sumeragi::records`, `sumeragi::bodies`) and the BLS crypto and signer
//! (`sumeragi::crypto`). TODO(WP5): the Kura block store, the State executor and builder,
//! `Init` from replay, and the node wiring.

pub mod barrier;
pub mod exec;
pub mod ingress;
pub mod persist;
pub mod serve;
pub mod traits;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, VecDeque},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

use iroha_sumeragi::{
    Core,
    api::{
        Action, CommittedTip, ConfigError, CoreStatus, Event, HaltReason, Init, LocalFault,
        LocalParams,
    },
    crypto::{Attestation, AttestationVerifier, Attestor, Crypto, Signer},
    message::{Evidence, TrafficClass, WireMessage},
    pacemaker::FRAME_OVERHEAD,
    safety::RecordState,
    types::{AggregateSignature, Hash32, HeightConfig, Millis, PublicKey, Signature},
};
use parking_lot::Mutex;

use self::{
    barrier::{Barrier, HeldLimits},
    exec::{ExecDone, ExecOp, ExecSched},
    ingress::{Ingress, IngressLimits},
    persist::{Backoff, PersistQueue, Write},
    serve::{ServeLimits, ServeRequest, ServeSched, Served},
    traits::{BlockStore, BodyStore, Clock, Executor, Net, Observer, RecordStore},
};

/// Longest idle wait of the event loop before it re-reads the clock.
const MAX_IDLE_WAIT_MS: Millis = 1_000;

/// Released effects handed out per [`Kernel::poll`]: a burst (a long-pending record finally
/// durable) leaves in batches, and a due `Tick` is handled between them (O5).
pub const MAX_OPS_PER_POLL: usize = 64;

/// A thread of a driver instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Worker {
    /// The event loop (the core's thread).
    Loop,
    /// The persistence thread.
    Persist,
    /// The executor thread.
    Exec,
    /// The serve thread.
    Serve,
}

/// O10 at run time: a committed configuration needs larger frames than the transport accepts,
/// so its larger proposals and bodies will not arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameLimitExceeded {
    /// Height the configuration applies from.
    pub height: u64,
    /// `max_block_bytes + 64 KiB`.
    pub needed: u64,
    /// The transport's frame limit.
    pub limit: u64,
}

/// A report for the [`Observer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Report {
    /// Signed misbehaviour (released by the O2 barrier).
    Evidence(Box<Evidence>),
    /// A local fault.
    Fault(LocalFault),
    /// The instance halted (only serving continues).
    Halt(HaltReason),
    /// A committed configuration outgrows the transport (O10).
    FrameLimit(FrameLimitExceeded),
}

/// An operation of the kernel for a worker or the transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Send `msg` to every peer of `to` (encoded once).
    Send {
        /// Recipients, in order.
        to: Vec<PublicKey>,
        /// The message.
        msg: WireMessage,
    },
    /// The next serving request for the serve thread.
    Serve(ServeRequest),
    /// The next durable write for the persistence thread.
    Persist {
        /// Sequence number.
        seq: u64,
        /// The write.
        write: Write,
    },
    /// The next operation for the executor thread.
    Exec(ExecOp),
    /// A report for the observer.
    Report(Report),
}

/// A worker's completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Completion {
    /// Write `seq` became durable, or failed and comes back to be retried.
    Persisted {
        /// Sequence number.
        seq: u64,
        /// Its result.
        result: Result<(), Write>,
    },
    /// The executor answered its operation.
    Exec(ExecDone),
    /// The serve thread finished its request.
    Served(Served),
}

/// The queues of an instance: diagnostics, and what the §13.5 conformance bounds (O-MEM).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Backlog {
    /// Effects held behind a pending record.
    pub held: usize,
    /// Block payload bytes of the held effects.
    pub held_bytes: u64,
    /// Held effects dropped by the bounds so far.
    pub held_dropped: u64,
    /// Released effects not handed out yet.
    pub released: usize,
    /// Writes queued or in flight.
    pub writes: usize,
    /// Safety records queued (not in flight).
    pub records: usize,
    /// Payload bytes of the queued bodies, per height.
    pub bodies: BTreeMap<u64, u64>,
    /// Executor operations queued other than `Execute`s.
    pub exec_ops: usize,
    /// `Execute` requests not answered yet.
    pub executes: usize,
    /// Serving requests pending.
    pub serve: usize,
    /// Serving requests dropped by the bounds so far.
    pub serve_dropped: u64,
    /// Messages queued in the ingress.
    pub ingress: usize,
    /// Messages dropped by the ingress bounds so far.
    pub ingress_dropped: u64,
}

/// What the kernel of an instance starts from.
pub struct KernelStart {
    /// Local parameters.
    pub local: LocalParams,
    /// Startup input (§7.4).
    pub init: Init,
    /// Configured signing keys.
    pub signers: Vec<Box<dyn Signer>>,
    /// The core's crypto.
    pub crypto: Box<dyn Crypto>,
    /// The kernel's hashing (block hashes of stored and executed bodies).
    pub hasher: Box<dyn Crypto>,
    /// The commit-attestation extension (§3.7).
    pub attestation: Attestation,
    /// Local time of the start.
    pub now: Millis,
    /// The ingress queues (shared with the handle that fills them).
    pub ingress: Arc<Mutex<Ingress>>,
    /// Backoff, bounds and the transport limit (the ingress bounds are the queues').
    pub config: DriverConfig,
}

/// Queue a network message into the ingress, unless it is the node's own (O7) or for another
/// instance. Returns whether it was queued.
pub fn admit_message(
    ingress: &Mutex<Ingress>,
    own: &[PublicKey],
    instance: &Hash32,
    from: PublicKey,
    msg: WireMessage,
    class: TrafficClass,
) -> bool {
    if own.contains(&from) || msg.instance() != instance {
        return false;
    }
    ingress.lock().push(from, msg, class);
    true
}

/// O10: whether the configuration of `height` needs larger frames than the transport accepts.
pub fn frame_limit_exceeded(
    frame_limit: u64,
    height: u64,
    config: &HeightConfig,
) -> Option<FrameLimitExceeded> {
    let needed = u64::from(config.params.max_block_bytes) + u64::from(FRAME_OVERHEAD);
    (needed > frame_limit).then_some(FrameLimitExceeded {
        height,
        needed,
        limit: frame_limit,
    })
}

/// The single-threaded, I/O-free heart of a driver instance.
pub struct Kernel {
    core: Core,
    hasher: Box<dyn Crypto>,
    instance: Hash32,
    own: Vec<PublicKey>,
    ingress: Arc<Mutex<Ingress>>,
    local: VecDeque<Event>,
    barrier: Barrier,
    persist: PersistQueue,
    exec: ExecSched,
    serve: ServeSched,
    out: VecDeque<Op>,
    frame_limit: u64,
    /// Local time of the latest call that told it.
    now: Millis,
}

impl Kernel {
    /// Start the core (§12.1) and route its start-up actions; returns the kernel and a copy of
    /// those actions (for observers).
    ///
    /// # Errors
    /// The core refused the configuration or the startup input.
    pub fn start(start: KernelStart) -> Result<(Self, Vec<Action>), ConfigError> {
        let instance = start.init.instance;
        let own = start
            .init
            .records
            .iter()
            .map(|(k, _, _)| k.clone())
            .collect();
        let applied = start.init.tip.height;
        let config = start.config;
        let (core, actions) = Core::new(
            start.local,
            start.init,
            start.signers,
            start.crypto,
            start.attestation,
            start.now,
        )?;
        let mut kernel = Self {
            core,
            hasher: start.hasher,
            instance,
            own,
            ingress: start.ingress,
            local: VecDeque::new(),
            barrier: Barrier::new(config.held),
            persist: PersistQueue::new(config.backoff),
            exec: ExecSched::new(applied, config.backoff),
            serve: ServeSched::new(config.serve),
            out: VecDeque::new(),
            frame_limit: config.frame_limit,
            now: start.now,
        };
        kernel.route(actions.clone());
        Ok((kernel, actions))
    }

    /// The core (read-only).
    pub fn core(&self) -> &Core {
        &self.core
    }

    /// The execution scheduler (read-only).
    pub fn exec(&self) -> &ExecSched {
        &self.exec
    }

    /// Queue a network message (O6–O8); the node's own messages are dropped (O7).
    pub fn receive(&mut self, from: PublicKey, msg: WireMessage, class: TrafficClass) {
        admit_message(&self.ingress, &self.own, &self.instance, from, msg, class);
    }

    /// Queue a local event (never dropped, O6).
    pub fn deliver(&mut self, event: Event) {
        self.local.push_back(event);
    }

    /// An includable transaction arrived (the builder's `PayloadReady` source).
    pub fn transactions_available(&mut self) {
        self.exec.transactions_available();
        self.collect();
    }

    /// Whether an input is queued.
    pub fn has_input(&self) -> bool {
        !self.local.is_empty() || !self.ingress.lock().is_empty()
    }

    /// The next input at local time `now` (O5): the due `Tick`, then local events, then
    /// messages by class.
    pub fn next_input(&mut self, now: Millis) -> Option<Event> {
        self.now = now;
        if self.core.next_wakeup() <= now {
            return Some(Event::Tick);
        }
        if let Some(event) = self.local.pop_front() {
            return Some(event);
        }
        let (from, msg) = self.ingress.lock().pop()?;
        Some(Event::Message { from, msg })
    }

    /// Hand one input to the core and route its actions (O1).
    pub fn handle(&mut self, now: Millis, event: Event) {
        self.now = now;
        let actions = self.core.handle(now, event);
        self.route(actions);
    }

    /// [`Kernel::handle`] for hosts that also observe the core's actions (the simulator's
    /// oracles): returns a copy of them.
    pub fn handle_observed(&mut self, now: Millis, event: Event) -> Vec<Action> {
        self.now = now;
        let actions = self.core.handle(now, event);
        self.route(actions.clone());
        actions
    }

    /// Route the core's actions in order: writes to the persistence queue (a record raises the
    /// barrier; a newer record of a key supersedes its queued one), executor work to the
    /// scheduler, the rest through the barrier (O1, O2).
    pub fn route(&mut self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::PersistSafety(record) => {
                    let (seq, superseded) = self.persist.push_record(record);
                    if let Some(old) = superseded {
                        self.barrier.superseded(old, seq);
                    }
                    self.barrier.persisting(seq);
                }
                Action::StoreBody { block } => {
                    // An applied height lives in the block store.
                    if block.header.height > self.exec.applied() {
                        self.persist.push(Write::Body(Box::new(block)));
                    }
                }
                Action::Execute { block, req } => {
                    let block_hash = block.hash(&*self.hasher);
                    self.exec.execute(req, block_hash, block);
                }
                Action::DiscardExecution { height, keep } => self.exec.discard(height, keep),
                Action::BuildPayload {
                    req,
                    height,
                    view,
                    max_bytes,
                    exec_budget_ms,
                } => self
                    .exec
                    .build(req, height, view, max_bytes, exec_budget_ms),
                Action::PayloadRejected {
                    height,
                    view,
                    block_hash,
                } => self.exec.reject(height, view, block_hash),
                Action::LocalFault(fault) => self.out.push_back(Op::Report(Report::Fault(fault))),
                gated => {
                    if let Some(effect) = self.barrier.admit(gated) {
                        self.effect(effect);
                    }
                }
            }
        }
        self.collect();
    }

    /// Carry out an effect the barrier let through.
    fn effect(&mut self, action: Action) {
        match action {
            Action::Send { to, msg } => self.send(vec![to], msg),
            Action::Broadcast { to, msg } => self.send(to, msg),
            Action::CommitBlock { block, commit_qc } => self.exec.commit(block, commit_qc),
            Action::ReportEvidence(evidence) => {
                self.out.push_back(Op::Report(Report::Evidence(evidence)));
            }
            Action::Halt(reason) => self.out.push_back(Op::Report(Report::Halt(reason))),
            other => match ServeRequest::from_action(other) {
                Ok(request) => {
                    self.serve.push(request, self.now);
                }
                Err(other) => iroha_logger::error!(?other, "sumeragi action not routable"),
            },
        }
    }

    fn send(&mut self, to: Vec<PublicKey>, msg: WireMessage) {
        let to: Vec<PublicKey> = to.into_iter().filter(|k| !self.own.contains(k)).collect();
        if !to.is_empty() {
            self.out.push_back(Op::Send { to, msg });
        }
    }

    /// Take the scheduler's answers for the core; a committed configuration that outgrows the
    /// transport limit is reported (O10).
    fn collect(&mut self) {
        for event in self.exec.take_events() {
            if let Event::BlockApplied {
                height,
                config_after_next,
                ..
            } = &event
                && let Some(exceeded) = frame_limit_exceeded(
                    self.frame_limit,
                    height.saturating_add(2),
                    config_after_next,
                )
            {
                iroha_logger::error!(?exceeded, "sumeragi configuration outgrows the transport");
                self.out.push_back(Op::Report(Report::FrameLimit(exceeded)));
            }
            self.local.push_back(event);
        }
    }

    /// A worker's completion at local time `now`.
    pub fn complete(&mut self, now: Millis, completion: Completion) {
        self.now = now;
        match completion {
            Completion::Persisted { seq, result } => match result {
                Ok(()) => {
                    let durable = self.persist.done(seq);
                    for effect in self.barrier.release(durable) {
                        self.effect(effect);
                    }
                }
                Err(write) => {
                    if let Some((old, newer)) = self.persist.failed(seq, write, now) {
                        self.barrier.superseded(old, newer);
                    }
                }
            },
            Completion::Exec(done) => {
                if let Some(height) = self.exec.done(now, done) {
                    self.persist.push(Write::Prune(height));
                }
            }
            Completion::Served(served) => {
                self.serve.done(now, served.bytes);
                if let Some(event) = served.event {
                    self.local.push_back(event);
                }
            }
        }
        self.collect();
    }

    /// The operations to start now: at most [`MAX_OPS_PER_POLL`] pending sends and reports,
    /// then at most one write, one executor operation and one serving request (each worker
    /// runs one at a time).
    pub fn poll(&mut self, now: Millis) -> Vec<Op> {
        self.now = now;
        let batch = self.out.len().min(MAX_OPS_PER_POLL);
        let mut ops: Vec<Op> = self.out.drain(..batch).collect();
        if let Some((seq, write)) = self.persist.next(now) {
            ops.push(Op::Persist { seq, write });
        }
        if let Some(op) = self.exec.next(now) {
            ops.push(Op::Exec(op));
        }
        if let Some(request) = self.serve.next(now) {
            ops.push(Op::Serve(request));
        }
        ops
    }

    /// Whether sends or reports wait for the next [`Kernel::poll`].
    pub fn has_output(&self) -> bool {
        !self.out.is_empty()
    }

    /// The next local time the kernel needs to run: the core's deadline or a retry.
    pub fn next_wakeup(&self) -> Millis {
        self.core
            .next_wakeup()
            .min(self.persist.wakeup())
            .min(self.exec.wakeup())
    }

    /// The effects held behind the O2 barrier, in order.
    pub fn held(&self) -> Vec<Action> {
        self.barrier.held().cloned().collect()
    }

    /// Messages dropped by the ingress bounds so far (O6).
    pub fn ingress_drops(&self) -> u64 {
        self.ingress.lock().dropped()
    }

    /// The queues of the instance.
    pub fn backlog(&self) -> Backlog {
        let (held, held_bytes) = self.barrier.size();
        let (ingress, ingress_dropped) = {
            let ingress = self.ingress.lock();
            (ingress.len(), ingress.dropped())
        };
        Backlog {
            held,
            held_bytes,
            held_dropped: self.barrier.dropped(),
            released: self.out.len(),
            writes: self.persist.len(),
            records: self.persist.queued_records(),
            bodies: self.persist.queued_bodies(),
            exec_ops: self.exec.queued_ops(),
            executes: self.exec.outstanding(),
            serve: self.serve.len(),
            serve_dropped: self.serve.dropped(),
            ingress,
            ingress_dropped,
        }
    }
}

/// Assemble the core's startup input (§7.4 Restart) from the block store: the tip (the genesis
/// block's hash and result at `genesis_height`), the last `W + 2` committed headers, the
/// records found for every key, the configurations of `t`, `t + 1`, `t + 2` and a fresh nonce.
///
/// # Errors
/// The block store lacks an entry at or below its height (local corruption).
#[allow(clippy::too_many_arguments)] // the startup inputs of §7.4, each from its own source
pub fn assemble_init(
    blocks: &(impl BlockStore + ?Sized),
    instance: Hash32,
    genesis_height: u64,
    genesis: (Hash32, Hash32),
    demotion_window: u64,
    records: Vec<(PublicKey, RecordState, bool)>,
    configs: Vec<(u64, HeightConfig)>,
    nonce: u64,
) -> Result<Init, ConfigError> {
    let t = blocks.height().max(genesis_height);
    let tip = if t == genesis_height {
        CommittedTip {
            height: t,
            block_hash: genesis.0,
            result: genesis.1,
            header: None,
            commit_qc: None,
        }
    } else {
        let entry = blocks
            .entry(t)
            .ok_or(ConfigError::InvalidInit("block store tip missing"))?;
        CommittedTip {
            height: t,
            block_hash: entry.commit_qc.block_hash,
            result: entry.commit_qc.result,
            header: Some(entry.block.header),
            commit_qc: Some(entry.commit_qc),
        }
    };
    let first = t
        .saturating_sub(demotion_window.saturating_add(1))
        .max(genesis_height + 1);
    let mut recent_headers = Vec::new();
    for height in first..=t {
        let entry = blocks
            .entry(height)
            .ok_or(ConfigError::InvalidInit("block store entry missing"))?;
        recent_headers.push(entry.block.header);
    }
    Ok(Init {
        instance,
        records,
        genesis_height,
        demotion_window,
        nonce,
        tip,
        configs,
        recent_headers,
    })
}

/// Why a driver did not start.
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    /// The core refused its configuration or startup input.
    #[error("sumeragi configuration: {0}")]
    Config(ConfigError),
    /// O10: the transport's frame limit is below what the chain parameters need.
    #[error("sumeragi frame limit {limit} is below the {needed} bytes the parameters need")]
    FrameLimit {
        /// Needed.
        needed: u64,
        /// Configured.
        limit: u64,
    },
    /// A thread could not be spawned.
    #[error("sumeragi driver thread: {0}")]
    Thread(#[from] std::io::Error),
}

/// Configuration of one driver instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriverConfig {
    /// Ingress bounds (O6, O8).
    pub ingress: IngressLimits,
    /// Retry backoff of failed writes and apply steps.
    pub backoff: Backoff,
    /// Bounds of the effects held behind a pending record (O6).
    pub held: HeldLimits,
    /// Per-peer serving limits (§12.2).
    pub serve: ServeLimits,
    /// Largest frame the transport accepts (O10); every frame is decoded within it.
    pub frame_limit: u64,
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            ingress: IngressLimits::default(),
            backoff: Backoff::default(),
            held: HeldLimits::default(),
            serve: ServeLimits::default(),
            frame_limit: 16 * 1024 * 1024 + u64::from(FRAME_OVERHEAD),
        }
    }
}

/// The cryptography of a driver instance, shared by its threads.
pub type SharedCrypto = Arc<dyn Crypto + Send + Sync>;

/// [`Crypto`] through a shared handle (the core and the kernel each hold a box).
struct CryptoRef(SharedCrypto);

impl Crypto for CryptoRef {
    fn hash(&self, bytes: &[u8]) -> Hash32 {
        self.0.hash(bytes)
    }
    fn verify(&self, pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool {
        self.0.verify(pk, msg, sig)
    }
    fn aggregate(&self, sigs: &[Signature]) -> AggregateSignature {
        self.0.aggregate(sigs)
    }
    fn verify_aggregate(&self, pks: &[&PublicKey], msg: &[u8], agg: &AggregateSignature) -> bool {
        self.0.verify_aggregate(pks, msg, agg)
    }
    fn verify_aggregate_multi(
        &self,
        groups: &[(Vec<&PublicKey>, Vec<u8>)],
        agg: &AggregateSignature,
    ) -> bool {
        self.0.verify_aggregate_multi(groups, agg)
    }
}

/// What a driver instance starts with (all of it moves to its threads).
pub struct DriverStart {
    /// Local parameters.
    pub local: LocalParams,
    /// Startup input ([`assemble_init`]).
    pub init: Init,
    /// Configured signing keys.
    pub signers: Vec<Box<dyn Signer + Send>>,
    /// Cryptography.
    pub crypto: SharedCrypto,
    /// The node's commit-attestation authority (§3.7).
    pub attestor: Box<dyn Attestor + Send>,
    /// The commit-attestation verifier.
    pub verifier: Box<dyn AttestationVerifier + Send>,
}

enum Input {
    Done(Completion),
    Wake,
    Transactions,
    /// A worker thread ended.
    Exited(Worker),
    Stop,
}

/// State shared between the event loop and the handles.
struct Shared {
    instance: Hash32,
    own: Vec<PublicKey>,
    ingress: Arc<Mutex<Ingress>>,
    /// Decode limit of every frame: the transport's (O10).
    frame_limit: usize,
    status: Mutex<Option<CoreStatus>>,
    backlog: Mutex<Backlog>,
    wake_pending: AtomicBool,
    /// The event loop runs.
    alive: AtomicBool,
    /// The thread whose end stopped the instance, if one did.
    stopped: Mutex<Option<Worker>>,
}

impl Shared {
    fn publish(&self, kernel: &Kernel) {
        *self.status.lock() = Some(kernel.core().status());
        *self.backlog.lock() = kernel.backlog();
    }
}

/// Run `f`, containing a panic (logged): an observer or transport call on the loop thread.
fn contained(what: &str, f: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(f)).is_err() {
        iroha_logger::error!(what, "sumeragi driver backend panicked");
    }
}

/// Record that the end of `worker` stopped the instance and tell the observer.
fn stop(shared: &Shared, observer: &dyn Observer, worker: Worker) {
    iroha_logger::error!(
        ?worker,
        "sumeragi driver thread stopped; the instance stops"
    );
    shared.stopped.lock().get_or_insert(worker);
    contained("observer", || observer.stopped(worker));
}

/// Tells the event loop that a worker thread ended, however it ended.
struct ExitGuard {
    worker: Worker,
    tx: mpsc::Sender<Input>,
}

impl Drop for ExitGuard {
    fn drop(&mut self) {
        let _ = self.tx.send(Input::Exited(self.worker));
    }
}

/// Marks the event loop stopped when its thread ends; a panic stops the instance.
struct LoopGuard {
    shared: Arc<Shared>,
    observer: Arc<dyn Observer>,
}

impl Drop for LoopGuard {
    fn drop(&mut self) {
        if std::thread::panicking() {
            stop(&self.shared, &*self.observer, Worker::Loop);
        }
        self.shared.alive.store(false, Ordering::Release);
    }
}

/// A cheap, cloneable handle of a running driver instance.
#[derive(Clone)]
pub struct DriverHandle {
    shared: Arc<Shared>,
    inputs: mpsc::Sender<Input>,
}

impl DriverHandle {
    fn wake(&self) {
        if !self.shared.wake_pending.swap(true, Ordering::AcqRel) {
            let _ = self.inputs.send(Input::Wake);
        }
    }

    /// Deliver an encoded frame from the authenticated peer `from` (never blocks; bounded,
    /// O6). The driver alone decodes, within the transport's frame limit (O10), which every
    /// committed configuration is validated against. Returns whether the message was queued.
    pub fn deliver(&self, from: &PublicKey, frame: &[u8]) -> bool {
        match WireMessage::decode(frame, self.shared.frame_limit) {
            Ok(msg) => self.deliver_message(from.clone(), msg),
            Err(_) => false,
        }
    }

    /// Deliver a decoded message (in-process transports and tests).
    pub fn deliver_message(&self, from: PublicKey, msg: WireMessage) -> bool {
        let class = msg.traffic_class();
        let shared = &self.shared;
        let queued = admit_message(
            &shared.ingress,
            &shared.own,
            &shared.instance,
            from,
            msg,
            class,
        );
        if queued {
            self.wake();
        }
        queued
    }

    /// An includable transaction arrived (`PayloadReady` after an `EMPTY` build).
    pub fn transactions_available(&self) {
        let _ = self.inputs.send(Input::Transactions);
    }

    /// The core's latest diagnostics (`None` before the core started).
    pub fn status(&self) -> Option<CoreStatus> {
        self.shared.status.lock().clone()
    }

    /// Why the instance halted, if it did: the core's reason, or `DriverAnomaly` when a
    /// stopped thread stopped the instance ([`DriverHandle::stopped`]).
    pub fn halted(&self) -> Option<HaltReason> {
        self.status()
            .and_then(|s| s.halted)
            .or_else(|| self.stopped().map(|_| HaltReason::DriverAnomaly))
    }

    /// The thread whose end stopped the instance, if one did.
    pub fn stopped(&self) -> Option<Worker> {
        *self.shared.stopped.lock()
    }

    /// Whether the core started and has not halted, and the instance runs.
    pub fn ready(&self) -> bool {
        self.shared.alive.load(Ordering::Acquire)
            && self.stopped().is_none()
            && self.status().is_some_and(|s| s.halted.is_none())
    }

    /// Messages dropped by the ingress bounds so far.
    pub fn ingress_drops(&self) -> u64 {
        self.shared.ingress.lock().dropped()
    }

    /// The instance's queues, as of the loop's latest step.
    pub fn backlog(&self) -> Backlog {
        self.shared.backlog.lock().clone()
    }
}

/// A running driver instance: its handle and threads.
pub struct RunningDriver {
    handle: DriverHandle,
    threads: Vec<JoinHandle<()>>,
}

impl RunningDriver {
    /// A handle for delivering messages and reading the status.
    pub fn handle(&self) -> DriverHandle {
        self.handle.clone()
    }

    /// Stop the event loop and wait for every thread of the instance.
    pub fn shutdown(self) {
        let _ = self.handle.inputs.send(Input::Stop);
        drop(self.handle);
        for thread in self.threads {
            let _ = thread.join();
        }
    }
}

/// The driver of one instance over its backends (§13.5): the transport `N`, the record store
/// `R`, the body store `B`, the block store `K`, the clock `C` and the executor `E`.
pub struct Driver<N, R, B, K, C, E> {
    /// Transport.
    pub net: Arc<N>,
    /// Safety records.
    pub records: Arc<R>,
    /// Block bodies.
    pub bodies: Arc<B>,
    /// Committed blocks.
    pub blocks: Arc<K>,
    /// Local clock.
    pub clock: Arc<C>,
    /// The application.
    pub executor: E,
    /// Reports.
    pub observer: Arc<dyn Observer>,
}

impl<N, R, B, K, C, E> Driver<N, R, B, K, C, E>
where
    N: Net + 'static,
    R: RecordStore + 'static,
    B: BodyStore + 'static,
    K: BlockStore + 'static,
    C: Clock + 'static,
    E: Executor + 'static,
{
    /// A driver over these backends.
    pub fn new(
        net: Arc<N>,
        records: Arc<R>,
        bodies: Arc<B>,
        blocks: Arc<K>,
        clock: Arc<C>,
        executor: E,
        observer: Arc<dyn Observer>,
    ) -> Self {
        Self {
            net,
            records,
            bodies,
            blocks,
            clock,
            executor,
            observer,
        }
    }

    /// Start the instance: checks the frame limit (O10), spawns the persistence, executor and
    /// serve threads and the event loop, which constructs the core (§12.1).
    ///
    /// # Errors
    /// An invalid configuration, a frame limit below the parameters' needs, or a thread that
    /// could not be spawned.
    #[allow(clippy::too_many_lines)] // one block per thread
    pub fn spawn(
        self,
        config: DriverConfig,
        start: DriverStart,
    ) -> Result<RunningDriver, DriverError> {
        check_frame_limit(&config, &start)?;
        let instance = start.init.instance;
        let own: Vec<PublicKey> = start
            .init
            .records
            .iter()
            .map(|(k, _, _)| k.clone())
            .collect();
        let ingress = Arc::new(Mutex::new(Ingress::new(config.ingress)));
        let shared = Arc::new(Shared {
            instance,
            own,
            ingress: Arc::clone(&ingress),
            frame_limit: usize::try_from(config.frame_limit).unwrap_or(usize::MAX),
            status: Mutex::new(None),
            backlog: Mutex::new(Backlog::default()),
            wake_pending: AtomicBool::new(false),
            alive: AtomicBool::new(true),
            stopped: Mutex::new(None),
        });
        let (inputs, rx) = mpsc::channel();
        let mut threads = Vec::new();
        let (persist_tx, persist_rx) = mpsc::channel::<(u64, Write)>();
        {
            let (records, bodies, crypto) = (
                self.records,
                Arc::clone(&self.bodies),
                Arc::clone(&start.crypto),
            );
            let tx = inputs.clone();
            threads.push(
                super::sumeragi_thread_builder("sumeragi-persist").spawn(move || {
                    let _exit = ExitGuard {
                        worker: Worker::Persist,
                        tx: tx.clone(),
                    };
                    for (seq, write) in persist_rx {
                        let result = persist::perform(&*records, &*bodies, &*crypto, write);
                        if tx
                            .send(Input::Done(Completion::Persisted { seq, result }))
                            .is_err()
                        {
                            break;
                        }
                    }
                })?,
            );
        }
        let (exec_tx, exec_rx) = mpsc::channel::<ExecOp>();
        {
            let (mut executor, blocks) = (self.executor, Arc::clone(&self.blocks));
            let tx = inputs.clone();
            threads.push(
                super::sumeragi_thread_builder("sumeragi-exec").spawn(move || {
                    let _exit = ExitGuard {
                        worker: Worker::Exec,
                        tx: tx.clone(),
                    };
                    for op in exec_rx {
                        let done = run_exec(&mut executor, &*blocks, op);
                        if tx.send(Input::Done(Completion::Exec(done))).is_err() {
                            break;
                        }
                    }
                })?,
            );
        }
        let (serve_tx, serve_rx) = mpsc::channel::<ServeRequest>();
        {
            let (bodies, blocks, net) = (self.bodies, self.blocks, Arc::clone(&self.net));
            let tx = inputs.clone();
            threads.push(
                super::sumeragi_thread_builder("sumeragi-serve").spawn(move || {
                    let _exit = ExitGuard {
                        worker: Worker::Serve,
                        tx: tx.clone(),
                    };
                    for request in serve_rx {
                        let served = catch_unwind(AssertUnwindSafe(|| {
                            serve::serve(request, instance, &*bodies, &*blocks, &*net)
                        }))
                        .unwrap_or_else(|_| {
                            iroha_logger::error!("sumeragi serving panicked");
                            Served::default()
                        });
                        if tx.send(Input::Done(Completion::Served(served))).is_err() {
                            break;
                        }
                    }
                })?,
            );
        }
        let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), ConfigError>>(1);
        {
            let (clock, net, observer) = (self.clock, self.net, self.observer);
            let shared = Arc::clone(&shared);
            threads.push(
                super::sumeragi_thread_builder("sumeragi-loop").spawn(move || {
                    let _guard = LoopGuard {
                        shared: Arc::clone(&shared),
                        observer: Arc::clone(&observer),
                    };
                    let workers = Workers {
                        net,
                        observer,
                        persist: persist_tx,
                        exec: exec_tx,
                        serve: serve_tx,
                    };
                    let kernel = Kernel::start(KernelStart {
                        local: start.local,
                        init: start.init,
                        signers: start
                            .signers
                            .into_iter()
                            .map(|s| -> Box<dyn Signer> { s })
                            .collect(),
                        crypto: Box::new(CryptoRef(Arc::clone(&start.crypto))),
                        hasher: Box::new(CryptoRef(start.crypto)),
                        attestation: Attestation::new(start.attestor, start.verifier),
                        now: clock.now(),
                        ingress,
                        config,
                    });
                    match kernel {
                        Ok((kernel, _)) => {
                            shared.publish(&kernel);
                            let _ = ready_tx.send(Ok(()));
                            if let Err(worker) = run_loop(kernel, &rx, &shared, &*clock, &workers) {
                                stop(&shared, &*workers.observer, worker);
                            }
                        }
                        Err(error) => {
                            let _ = ready_tx.send(Err(error));
                        }
                    }
                })?,
            );
        }
        let handle = DriverHandle { shared, inputs };
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(RunningDriver { handle, threads }),
            Ok(Err(error)) => {
                RunningDriver { handle, threads }.shutdown();
                Err(DriverError::Config(error))
            }
            Err(_) => {
                RunningDriver { handle, threads }.shutdown();
                Err(DriverError::Config(ConfigError::InvalidInit(
                    "event loop died",
                )))
            }
        }
    }
}

/// O10 at start: the transport limit covers the blocks of every startup configuration and the
/// sync byte cap.
fn check_frame_limit(config: &DriverConfig, start: &DriverStart) -> Result<(), DriverError> {
    for (height, height_config) in &start.init.configs {
        if let Some(exceeded) = frame_limit_exceeded(config.frame_limit, *height, height_config) {
            return Err(DriverError::FrameLimit {
                needed: exceeded.needed,
                limit: exceeded.limit,
            });
        }
    }
    let bulk = u64::from(start.local.sync_max_bytes) + u64::from(FRAME_OVERHEAD);
    if bulk > config.frame_limit {
        return Err(DriverError::FrameLimit {
            needed: bulk,
            limit: config.frame_limit,
        });
    }
    Ok(())
}

/// Run one executor operation (the executor thread); a panic of the executor or of the block
/// store becomes a local failure, retried like one.
fn run_exec<E: Executor, K: BlockStore + ?Sized>(
    executor: &mut E,
    blocks: &K,
    op: ExecOp,
) -> ExecDone {
    let failed = |what: &str| format!("executor panicked in {what}");
    match op {
        ExecOp::Execute { block, block_hash } => ExecDone::Executed(
            catch_unwind(AssertUnwindSafe(|| executor.execute(&block, &block_hash)))
                .unwrap_or_else(|_| {
                    Some(iroha_sumeragi::api::ExecOutcome::Failed(failed("execute")))
                }),
        ),
        ExecOp::Discard { height, keep } => {
            let _ = catch_unwind(AssertUnwindSafe(|| executor.discard(height, &keep)));
            ExecDone::Discarded
        }
        ExecOp::Prepare(commit) => ExecDone::Prepared(
            catch_unwind(AssertUnwindSafe(|| {
                executor.prepare(&commit.block, &commit.qc)
            }))
            .unwrap_or_else(|_| Err(failed("prepare"))),
        ),
        ExecOp::Append(commit) => {
            let appended = catch_unwind(AssertUnwindSafe(|| {
                blocks.append(&commit.block, &commit.qc)
            }))
            .unwrap_or_else(|_| Err(std::io::Error::other("block store panicked")));
            ExecDone::Appended(match appended {
                Ok(()) => true,
                Err(error) => {
                    iroha_logger::warn!(%error, "sumeragi block store append failed; retrying");
                    false
                }
            })
        }
        ExecOp::Commit(commit) => ExecDone::Committed(
            catch_unwind(AssertUnwindSafe(|| {
                executor.commit(&commit.block, &commit.qc)
            }))
            .unwrap_or_else(|_| Err(failed("commit"))),
        ),
        ExecOp::Build {
            height,
            view,
            max_bytes,
            exec_budget_ms,
            ..
        } => {
            let (payload, attest) = catch_unwind(AssertUnwindSafe(|| {
                executor.build(height, view, max_bytes, exec_budget_ms)
            }))
            .unwrap_or_default();
            ExecDone::Built { payload, attest }
        }
        ExecOp::Reject {
            height,
            view,
            block_hash,
        } => {
            let _ = catch_unwind(AssertUnwindSafe(|| {
                executor.reject(height, view, &block_hash)
            }));
            ExecDone::Rejected
        }
    }
}

/// Where the event loop sends operations.
struct Workers {
    net: Arc<dyn Net>,
    observer: Arc<dyn Observer>,
    persist: mpsc::Sender<(u64, Write)>,
    exec: mpsc::Sender<ExecOp>,
    serve: mpsc::Sender<ServeRequest>,
}

impl Workers {
    /// Start `ops`; a worker that cannot be reached stops the instance (returned).
    fn dispatch(&self, ops: Vec<Op>) -> Result<(), Worker> {
        for op in ops {
            match op {
                Op::Send { to, msg } => contained("transport", || {
                    if let Some(frame) = serve::frame(&msg) {
                        for peer in &to {
                            self.net.send(peer, &frame);
                        }
                    }
                }),
                Op::Serve(request) => self.serve.send(request).map_err(|_| Worker::Serve)?,
                Op::Persist { seq, write } => self
                    .persist
                    .send((seq, write))
                    .map_err(|_| Worker::Persist)?,
                Op::Exec(op) => self.exec.send(op).map_err(|_| Worker::Exec)?,
                Op::Report(report) => contained("observer", || match &report {
                    Report::Evidence(evidence) => self.observer.evidence(evidence),
                    Report::Fault(fault) => self.observer.fault(fault),
                    Report::Halt(reason) => self.observer.halt(reason),
                    Report::FrameLimit(exceeded) => self.observer.frame_limit(exceeded),
                }),
            }
        }
        Ok(())
    }
}

/// The event loop (O5): absorb completions, start operations, handle the next input by
/// priority (a due `Tick` also between batches of released effects), publish the status, and
/// otherwise wait for an input or the next deadline. Returns on shutdown, or with the worker
/// whose end stops the instance (it ended, or cannot be reached).
fn run_loop(
    mut kernel: Kernel,
    rx: &mpsc::Receiver<Input>,
    shared: &Shared,
    clock: &dyn Clock,
    workers: &Workers,
) -> Result<(), Worker> {
    let absorb = |kernel: &mut Kernel, input: Input| -> Result<bool, Worker> {
        match input {
            Input::Done(completion) => kernel.complete(clock.now(), completion),
            Input::Wake => shared.wake_pending.store(false, Ordering::Release),
            Input::Transactions => kernel.transactions_available(),
            Input::Exited(worker) => return Err(worker),
            Input::Stop => return Ok(false),
        }
        Ok(true)
    };
    loop {
        loop {
            match rx.try_recv() {
                Ok(input) => {
                    if !absorb(&mut kernel, input)? {
                        return Ok(());
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let now = clock.now();
        workers.dispatch(kernel.poll(now))?;
        if let Some(event) = kernel.next_input(now) {
            kernel.handle(now, event);
            workers.dispatch(kernel.poll(clock.now()))?;
            shared.publish(&kernel);
            continue;
        }
        if kernel.has_output() {
            continue;
        }
        shared.publish(&kernel);
        let wait = kernel
            .next_wakeup()
            .saturating_sub(clock.now())
            .min(MAX_IDLE_WAIT_MS);
        match rx.recv_timeout(Duration::from_millis(wait)) {
            Ok(input) => {
                if !absorb(&mut kernel, input)? {
                    return Ok(());
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}
