//! The node's executor for the Sumeragi driver: executes blocks on the committed [`State`],
//! applies committed blocks and builds payloads (`specs/sumeragi.md` §4, §6.10, §12.2,
//! §12.3 O3/O4).
//!
//! State allows one writer at a time and an executed overlay borrows the state, so all work
//! runs on one dedicated thread that owns the state and keeps at most **one live overlay**
//! (the most recent execution). [`StateExecutor`] is the driver-facing handle: it forwards
//! each call to that thread and waits for the answer.
//!
//! - `execute` runs only on the applied tip (the core requests an execution once its parent's
//!   `CommitBlock` was emitted, and the driver applies in order); otherwise it answers `None`
//!   and the driver parks the request (never `Failed` for that).
//! - A local condition (storage, admission, a panic) is `Failed` and retried; everything else
//!   the validator rejects is `Invalid` — a deterministic verdict every honest node reaches.
//! - `prepare` reuses the live overlay when it is the committed block and its result is the
//!   certified one, otherwise executes again (a missing cache entry is never a divergence), and
//!   stages the executed block for the block store; `commit` then publishes the overlay into the
//!   state after the block store holds the block, and returns the configuration of `h + 2`.

use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::Duration,
};

use iroha_data_model::{
    account::AccountId,
    block::{BlockHeader as IrohaHeader, CommitCertificate, SignedBlock},
    events::EventBox,
    parameter::system::ConsensusMode,
    transaction::{TransactionAdmissionIntent, TransactionEntrypoint},
};
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    api::ExecOutcome,
    message::{Block, Qc},
    types::{Hash32, HeightConfig},
};

use super::{
    block_store::{StagedBlock, Staging, commit_certificate},
    commitment::{ExecutionResultCommitment, execution_result},
    driver::traits::Executor,
    lanes,
    network_topology::Topology,
    payload::{self, Assembly},
    schedule,
};
use crate::{
    EventsSender,
    block::{BlockValidationError, ValidBlock},
    queue::Queue,
    state::{State, StateBlock, StateReadOnly, WorldReadOnly},
};

/// Payload bytes kept free for the block's non-transaction fields when selecting.
const PAYLOAD_OVERHEAD: usize = 64 * 1024;
/// Transactions of a rejected own payload tested one by one for the quarantine.
const MAX_QUARANTINE_TESTS: usize = 256;

/// What the executor thread needs.
#[derive(Clone)]
pub struct ExecutorContext {
    /// The node's state.
    pub state: Arc<State>,
    /// The transaction queue (peeked by the builder, cleaned after apply); `None` until the
    /// node attaches it ([`StateExecutor::attach_queue`]), e.g. during startup replay.
    pub queue: Option<Arc<Queue>>,
    /// Hand-off of executed blocks to the block store.
    pub staging: Staging,
    /// Pipeline and state events of applied blocks.
    pub events: EventsSender,
    /// The genesis account.
    pub genesis_account: AccountId,
    /// The chain's consensus mode.
    pub consensus_mode: ConsensusMode,
    /// The applied tip: its height and core block hash.
    pub applied: (u64, Hash32),
    /// The driver's cryptography: the keys of each newly scheduled committee are admitted
    /// (their proofs of possession verified) as blocks commit.
    pub crypto: Option<Arc<super::crypto::BlsCrypto>>,
    /// The applied tip published to the node's lane instances (`specs/sumeragi_lanes.md` §3.2).
    pub applied_watch: Arc<crate::sumeragi::lanes::global::AppliedWatch>,
    /// The node's committed lane blocks, which merged global blocks execute (§4.3).
    pub lane_blocks: Arc<dyn crate::sumeragi::lanes::merge::LaneBlockSource>,
}

/// Configured SoraFS archives captured from the exact committed State before apply completes.
///
/// The executor is their single capture producer. A failed capture keeps the committed decision
/// pending, so the driver cannot acknowledge it or advance to another height before recovery.
#[derive(Clone, Default)]
pub struct FinalizedArchives {
    /// Finalized provider assignments and completions.
    pub provider_ingest:
        Option<Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>>,
    /// Finalized provider reputation state.
    pub reputation: Option<Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>>,
}

impl FinalizedArchives {
    fn capture(&self, view: &impl StateReadOnly) -> Result<(), String> {
        if let Some(archive) = &self.provider_ingest {
            archive
                .capture_certified_view(view, view.kura())
                .map_err(|error| format!("provider-ingest archive capture failed: {error}"))?;
        }
        if let Some(archive) = &self.reputation {
            archive
                .capture_certified_view(view, view.kura())
                .map_err(|error| format!("reputation archive capture failed: {error}"))?;
        }
        Ok(())
    }
}

/// One State publication awaiting durable archive capture and its remaining notifications.
struct PendingCommit {
    header: iroha_sumeragi::message::BlockHeader,
    qc: Qc,
    state_hash: iroha_crypto::HashOf<IrohaHeader>,
    next: HeightConfig,
    hashes: Vec<iroha_crypto::HashOf<TransactionEntrypoint>>,
    events: Vec<EventBox>,
}

impl PendingCommit {
    fn matches(&self, block: &Block, qc: &Qc) -> bool {
        self.header == block.header && self.qc == *qc
    }
}

enum Request {
    Execute(Block, Hash32, mpsc::SyncSender<Option<ExecOutcome>>),
    Discard(u64, Vec<Hash32>),
    Prepare(Block, Qc, mpsc::SyncSender<Result<Option<Hash32>, String>>),
    Commit(Block, Qc, mpsc::SyncSender<Result<HeightConfig, String>>),
    Build(u64, u64, u32, mpsc::SyncSender<(Vec<u8>, bool)>),
    Reject(u64, u64, Hash32),
    AttachQueue(Arc<Queue>),
    AttachBeacon(Arc<super::beacon::BeaconService>),
    AttachFinalizedArchives(FinalizedArchives, mpsc::SyncSender<Result<(), String>>),
}

/// The driver-facing handle of the executor thread.
pub struct StateExecutor {
    requests: mpsc::Sender<Request>,
    _thread: JoinHandle<()>,
}

impl StateExecutor {
    /// Spawn the executor thread.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn spawn(context: ExecutorContext) -> std::io::Result<Self> {
        let (requests, rx) = mpsc::channel();
        let thread = super::threads::sumeragi_thread_builder("sumeragi-state-exec")
            .spawn(move || run(&context, &rx))?;
        Ok(Self {
            requests,
            _thread: thread,
        })
    }

    fn call<T>(&self, request: impl FnOnce(mpsc::SyncSender<T>) -> Request) -> Option<T> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.requests.send(request(tx)).ok()?;
        rx.recv().ok()
    }
}

impl StateExecutor {
    /// Attach the transaction queue: the builder reads it and applied blocks clean it.
    pub fn attach_queue(&self, queue: Arc<Queue>) {
        let _ = self.requests.send(Request::AttachQueue(queue));
    }

    /// Attach the current pulse producer after replay, before the consensus driver starts.
    pub fn attach_beacon(&self, beacon: Arc<super::beacon::BeaconService>) {
        let _ = self.requests.send(Request::AttachBeacon(beacon));
    }

    /// Attach the configured archives once, after replay and before starting the driver.
    /// This synchronously captures the reconciled tip before the executor acknowledges binding.
    ///
    /// # Errors
    /// The executor is unavailable, already bound or executing, or the exact tip cannot be captured.
    pub fn attach_finalized_archives(&self, archives: FinalizedArchives) -> Result<(), String> {
        self.call(|reply| Request::AttachFinalizedArchives(archives, reply))
            .unwrap_or_else(|| Err("executor thread stopped".into()))
    }

    /// Re-apply a block Kura already holds (startup replay): execute it on the applied tip
    /// and require the certified result.
    ///
    /// # Errors
    /// The block does not re-execute to its certified result, or a local failure.
    pub fn replay(&mut self, block: &Block, commit_qc: &Qc) -> Result<(), String> {
        match self.prepare(block, commit_qc)? {
            Some(result) if result == commit_qc.result => {}
            Some(_) => return Err("replayed block diverges from its certified result".into()),
            None => return Err("replayed block no longer executes".into()),
        }
        self.commit(block, commit_qc).map(|_| ())
    }
}

impl Executor for StateExecutor {
    fn execute(&mut self, block: &Block, block_hash: &Hash32) -> Option<ExecOutcome> {
        self.call(|reply| Request::Execute(block.clone(), *block_hash, reply))
            .unwrap_or_else(|| Some(ExecOutcome::Failed("executor thread stopped".into())))
    }

    fn discard(&mut self, height: u64, keep: &[Hash32]) {
        let _ = self.requests.send(Request::Discard(height, keep.to_vec()));
    }

    fn prepare(&mut self, block: &Block, commit_qc: &Qc) -> Result<Option<Hash32>, String> {
        self.call(|reply| Request::Prepare(block.clone(), commit_qc.clone(), reply))
            .unwrap_or_else(|| Err("executor thread stopped".into()))
    }

    fn commit(&mut self, block: &Block, commit_qc: &Qc) -> Result<HeightConfig, String> {
        self.call(|reply| Request::Commit(block.clone(), commit_qc.clone(), reply))
            .unwrap_or_else(|| Err("executor thread stopped".into()))
    }

    fn build(
        &mut self,
        height: u64,
        view: u64,
        max_bytes: u32,
        _exec_budget_ms: u32,
    ) -> (Vec<u8>, bool) {
        self.call(|reply| Request::Build(height, view, max_bytes, reply))
            .unwrap_or_default()
    }

    fn reject(&mut self, height: u64, view: u64, block_hash: &Hash32) {
        let _ = self
            .requests
            .send(Request::Reject(height, view, *block_hash));
    }
}

/// The executed overlay of one block.
struct Live<'s> {
    block_hash: Hash32,
    height: u64,
    valid: ValidBlock,
    overlay: Box<StateBlock<'s>>,
    witness: iroha_data_model::block::consensus::ExecWitness,
    preimage: Vec<u8>,
    result: Hash32,
    next: HeightConfig,
    committee: Vec<PeerId>,
    events: Vec<EventBox>,
}

struct Worker<'s> {
    context: &'s ExecutorContext,
    state: &'s State,
    applied: (u64, Hash32),
    live: Option<Live<'s>>,
    /// Verdicts of executions whose overlay is gone, by block hash (bounded per height).
    results: BTreeMap<Hash32, (u64, ExecOutcome)>,
    /// The transactions of the last payload this node built, for the quarantine.
    last_built: Option<(u64, u64, Vec<iroha_crypto::HashOf<TransactionEntrypoint>>)>,
    queue: Option<Arc<Queue>>,
    beacon: Option<Arc<super::beacon::BeaconService>>,
    archives: Option<FinalizedArchives>,
    pending_commit: Option<PendingCommit>,
}

fn run(context: &ExecutorContext, requests: &mpsc::Receiver<Request>) {
    let state = Arc::clone(&context.state);
    let mut worker = Worker {
        context,
        state: &state,
        applied: context.applied,
        live: None,
        results: BTreeMap::new(),
        last_built: None,
        queue: context.queue.clone(),
        beacon: None,
        archives: None,
        pending_commit: None,
    };
    while let Ok(request) = requests.recv() {
        worker.serve(request);
    }
}

impl Worker<'_> {
    fn serve(&mut self, request: Request) {
        match request {
            Request::Execute(block, block_hash, reply) => {
                let _ = reply.send(self.execute(&block, block_hash));
            }
            Request::Discard(height, keep) => self.discard(height, &keep),
            Request::Prepare(block, qc, reply) => {
                let _ = reply.send(self.prepare(&block, &qc));
            }
            Request::Commit(block, qc, reply) => {
                let _ = reply.send(self.commit(&block, &qc));
            }
            Request::Build(height, view, max_bytes, reply) => {
                let _ = reply.send(self.build(height, view, max_bytes));
            }
            Request::Reject(height, view, block_hash) => self.reject(height, view, block_hash),
            Request::AttachQueue(queue) => self.queue = Some(queue),
            Request::AttachBeacon(beacon) => self.beacon = Some(beacon),
            Request::AttachFinalizedArchives(archives, reply) => {
                let result = if self.archives.is_some()
                    || self.live.is_some()
                    || self.pending_commit.is_some()
                {
                    Err("finalized archives must be bound once before execution starts".into())
                } else {
                    archives
                        .capture(&self.state.view())
                        .map(|()| self.archives = Some(archives))
                };
                let _ = reply.send(result);
            }
        }
    }

    /// Answer an `Execute` (O4): the live overlay or a remembered verdict, `None` while the
    /// parent is not applied, otherwise a fresh execution.
    fn execute(&mut self, block: &Block, block_hash: Hash32) -> Option<ExecOutcome> {
        if self.pending_commit.is_some() {
            return None;
        }
        if let Some(live) = &self.live {
            if live.block_hash == block_hash {
                return Some(ExecOutcome::Valid(live.result));
            }
        }
        if let Some((_, outcome)) = self.results.get(&block_hash) {
            return Some(outcome.clone());
        }
        if !self.parent_applied(block) {
            return None;
        }
        let outcome = self.run_execution(block, block_hash);
        if !matches!(outcome, ExecOutcome::Valid(_) | ExecOutcome::Failed(_)) {
            self.remember(block.header.height, block_hash, outcome.clone());
        }
        Some(outcome)
    }

    fn parent_applied(&self, block: &Block) -> bool {
        block.header.height == self.applied.0.saturating_add(1)
            && block.header.parent_hash == self.applied.1
    }

    fn remember(&mut self, height: u64, block_hash: Hash32, outcome: ExecOutcome) {
        self.results.insert(block_hash, (height, outcome));
        // The core keeps at most a handful of bodies per height (§8.4); stay bounded.
        while self.results.len() > 16 {
            let Some(oldest) = self
                .results
                .iter()
                .min_by_key(|(_, (height, _))| *height)
                .map(|(hash, _)| *hash)
            else {
                break;
            };
            self.results.remove(&oldest);
        }
    }

    /// Execute `block` on the applied tip, keeping the overlay as the live one.
    fn run_execution(&mut self, block: &Block, block_hash: Hash32) -> ExecOutcome {
        // The state admits one overlay: drop the previous one first.
        if let Some(previous) = self.live.take() {
            self.remember(
                previous.height,
                previous.block_hash,
                ExecOutcome::Valid(previous.result),
            );
            // Its overlay is gone; a later `prepare` executes again.
            self.results.remove(&previous.block_hash);
        }
        let height = block.header.height;
        let iroha_block = match payload::decode(&block.payload) {
            Ok(block) => block,
            Err(error) => return invalid(height, &error),
        };
        if self.state.view().latest_block().is_none() {
            return ExecOutcome::Failed("the applied parent block is not available".into());
        }
        let Some(scheduled) = self.scheduled(height) else {
            return ExecOutcome::Failed(format!("no scheduled configuration for height {height}"));
        };
        let cadence = Duration::from_millis(scheduled.params.block_time_ms);
        if !proposal_matches_header(iroha_block.header(), block) {
            return invalid(
                height,
                &"the payload's height or view differs from the header",
            );
        }
        if block.header.attest != attestation_required(&iroha_block) {
            return invalid(
                height,
                &"the attestation flag differs from the payload's rule",
            );
        }
        // Merged lane blocks execute after the block's own transactions (§4.3 of
        // `specs/sumeragi_lanes.md`); the node waits for its lane stores within `E_max`.
        let expansion = match lanes::merge::expand(
            &self.state.view(),
            &iroha_block,
            &*self.context.lane_blocks,
            Duration::from_millis(scheduled.params.exec_budget_ms),
        ) {
            Ok(expansion) => expansion,
            Err(lanes::merge::MergeError::Pending(reason)) => {
                return ExecOutcome::Failed(reason);
            }
            Err(error @ lanes::merge::MergeError::Invalid(_)) => return invalid(height, &error),
        };
        let iroha_block = match expansion.apply(iroha_block) {
            Ok(block) => block,
            Err(error) => return invalid(height, &error),
        };
        let topology = Topology::new(scheduled.committee.clone());
        let validated = catch_unwind(AssertUnwindSafe(|| {
            ValidBlock::validate_sumeragi_block(
                iroha_block,
                &topology,
                &self.context.genesis_account,
                cadence,
                self.context.consensus_mode,
                expansion.step,
                self.state,
            )
        }));
        let Ok(validated) = validated else {
            return ExecOutcome::Failed("block validation panicked".into());
        };
        let mut events = Vec::new();
        let (valid, mut overlay) = match validated.unpack(|event| events.push(event.into())) {
            Ok(executed) => executed,
            Err((_, error)) => return classify(height, &error),
        };
        if let Err(error) = overlay.take_sumeragi_lanes() {
            return invalid(height, &error);
        }
        let next = match overlay
            .take_sumeragi_schedule()
            .and_then(|next| next.height_config())
        {
            Ok(next) => next,
            Err(error) => return invalid(height, &error),
        };
        let Some(witness) = overlay.take_exec_witness() else {
            return ExecOutcome::Failed("the execution witness was not captured".into());
        };
        let (commitment, preimage, result) = match execution_result(&witness, valid.as_ref(), &next)
        {
            Ok(computed) => computed,
            Err(error) => return invalid(height, &error),
        };
        if top_ups_without_flag(&commitment, block.header.attest) {
            return invalid(height, &"executed top-ups without the attestation flag");
        }
        self.live = Some(Live {
            block_hash,
            height,
            valid,
            overlay,
            witness,
            preimage,
            result,
            next,
            committee: scheduled.committee,
            events,
        });
        ExecOutcome::Valid(result)
    }

    /// Admit the keys of the committee scheduled for `height` into the driver's cryptography.
    fn admit_scheduled(&self, height: u64) {
        let Some(crypto) = &self.context.crypto else {
            return;
        };
        let Some(config) = self.scheduled(height) else {
            return;
        };
        let view = self.state.view();
        for (peer, pop) in schedule::committee_pops(view.world(), &config) {
            if let Err(error) = crypto.admit(peer.public_key(), &pop) {
                iroha_logger::warn!(%peer, ?error, "sumeragi: committee key not admitted");
            }
        }
    }

    fn scheduled(&self, height: u64) -> Option<schedule::ScheduledConfig> {
        self.state
            .view()
            .world()
            .consensus_schedule()
            .get(height)
            .cloned()
    }

    fn discard(&mut self, height: u64, keep: &[Hash32]) {
        if self
            .live
            .as_ref()
            .is_some_and(|live| live.height == height && !keep.contains(&live.block_hash))
        {
            self.live = None;
        }
        self.results
            .retain(|hash, (at, _)| *at != height || keep.contains(hash));
    }

    /// The local result of the committed block, staging it for the block store.
    fn prepare(&mut self, block: &Block, qc: &Qc) -> Result<Option<Hash32>, String> {
        if let Some(pending) = &self.pending_commit {
            return if pending.matches(block, qc) {
                Ok(Some(pending.qc.result))
            } else {
                Err("another committed decision is awaiting archive capture".into())
            };
        }
        let block_hash = qc.block_hash;
        let reusable = self
            .live
            .as_ref()
            .is_some_and(|live| live.block_hash == block_hash && live.result == qc.result);
        if !reusable {
            self.live = None;
            self.results.remove(&block_hash);
            if !self.parent_applied(block) {
                return Err("the committed block's parent is not applied".into());
            }
            match self.run_execution(block, block_hash) {
                ExecOutcome::Valid(_) => {}
                ExecOutcome::Invalid => return Ok(None),
                ExecOutcome::Failed(reason) => return Err(reason),
                ExecOutcome::Cancelled => return Err("execution cancelled".into()),
            }
        }
        let live = self
            .live
            .as_ref()
            .ok_or_else(|| "no executed overlay to prepare".to_owned())?;
        self.context.staging.stage(StagedBlock {
            block_hash,
            executed: Arc::new(live.valid.as_ref().clone()),
            result_preimage: live.preimage.clone(),
        });
        Ok(Some(live.result))
    }

    /// Publish the prepared overlay (the block store already holds the block).
    fn commit(&mut self, block: &Block, qc: &Qc) -> Result<HeightConfig, String> {
        if let Some(pending) = &self.pending_commit {
            if !pending.matches(block, qc) {
                return Err("another committed decision is awaiting archive capture".into());
            }
            return self.finish_commit();
        }
        let live = match self.live.take() {
            Some(live) if live.block_hash == qc.block_hash && live.result == qc.result => live,
            other => {
                self.live = other;
                return Err("commit without its prepared overlay".into());
            }
        };
        let Live {
            valid,
            mut overlay,
            witness,
            preimage,
            next,
            committee,
            mut events,
            ..
        } = live;
        let certificate: CommitCertificate =
            commit_certificate(&block.header, qc, preimage).map_err(|error| error.to_string())?;
        let committed = valid
            .commit_unchecked()
            .unpack(|event| events.push(event.into()));
        overlay.authorize_sumeragi_output_publication(&committed, &witness, &certificate)?;
        let state_events = overlay
            .apply_without_execution_with_sumeragi_commit(&committed, &certificate, committee)
            .map_err(|error| error.to_string())?;
        overlay.commit().map_err(|error| error.to_string())?;
        self.pending_commit = Some(PendingCommit {
            header: block.header.clone(),
            qc: qc.clone(),
            state_hash: committed.as_ref().hash(),
            next,
            hashes: committed
                .as_ref()
                .external_entrypoints_slice()
                .iter()
                .map(TransactionEntrypoint::hash)
                .collect(),
            events: events.into_iter().chain(state_events).collect(),
        });
        self.finish_commit()
    }

    /// Retry only capture after State publication. Never reapply a transaction or emit success
    /// before every configured archive durably accepts this exact decision.
    fn finish_commit(&mut self) -> Result<HeightConfig, String> {
        let pending = self
            .pending_commit
            .as_ref()
            .ok_or_else(|| "no committed decision is awaiting completion".to_owned())?;
        {
            let view = self.state.view();
            let height = u64::try_from(view.height()).map_err(|_| "State height exceeds u64")?;
            if height != pending.header.height
                || view.latest_block_hash() != Some(pending.state_hash)
            {
                return Err("State changed while committed archive capture was pending".into());
            }
            if let Some(archives) = &self.archives {
                archives.capture(&view)?;
            }
        }
        let pending = self
            .pending_commit
            .take()
            .ok_or_else(|| "committed completion disappeared".to_owned())?;
        let height = pending.header.height;
        self.applied = (height, pending.qc.block_hash);
        self.context
            .applied_watch
            .publish(height, pending.state_hash);
        self.results.retain(|_, (at, _)| *at > height);
        if let Some(queue) = &self.queue {
            queue.remove_committed_hashes(pending.hashes, None);
        }
        self.admit_scheduled(height.saturating_add(2));
        for event in pending.events {
            let _ = self.context.events.send(event);
        }
        Ok(pending.next)
    }

    fn pulse_for_height(
        &self,
        height: u64,
    ) -> Result<Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>, String>
    {
        if let Some(beacon) = &self.beacon {
            return beacon
                .pulse_for_height(height)
                .map_err(|error| error.to_string());
        }
        // Replay and component executors have no signer service. They may build only heights
        // for which committed state requests no pulse.
        if super::beacon::current_requirement(self.state, height, self.context.consensus_mode)
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("required global beacon producer is not attached".into());
        }
        Ok(None)
    }

    /// Build a payload for `(height, view)` over the applied tip (§6.10).
    fn build(&mut self, height: u64, view: u64, max_bytes: u32) -> (Vec<u8>, bool) {
        if self.pending_commit.is_some() || height != self.applied.0.saturating_add(1) {
            return (Vec::new(), false);
        }
        let Some(parent) = self.state.view().latest_block() else {
            return (Vec::new(), false);
        };
        let Some(scheduled) = self.scheduled(height) else {
            return (Vec::new(), false);
        };
        let Some(queue) = &self.queue else {
            return (Vec::new(), false);
        };
        let max_bytes = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        // Certified lane blocks come first: they reserve their share of the block's capacity
        // (`specs/sumeragi_lanes.md` §4.2).
        let (merges, reserved) =
            lanes::merge::propose(&self.state.view(), &*self.context.lane_blocks, height);
        let mut selected = payload::select(
            self.state,
            queue,
            max_bytes.saturating_sub(PAYLOAD_OVERHEAD),
            reserved,
        );
        // Only real work may activate the pulse signer. A pulse cannot create a block.
        if selected.is_empty() && merges.is_empty() {
            return (Vec::new(), false);
        }
        let pulse = match self.pulse_for_height(height) {
            Ok(pulse) => pulse,
            Err(error) => {
                iroha_logger::debug!(height, %error, "sumeragi: waiting for required global beacon pulse");
                return (Vec::new(), false);
            }
        };
        let assembly = Assembly {
            parent: &parent,
            view,
            cadence: Duration::from_millis(scheduled.params.block_time_ms),
        };
        while !selected.is_empty() || !merges.is_empty() {
            let block = match payload::assemble_with_merges(
                self.state, assembly, &selected, &merges, pulse,
            ) {
                Ok(block) => block,
                Err(error) => {
                    iroha_logger::warn!(height, %error, "sumeragi: payload assembly failed");
                    return (Vec::new(), false);
                }
            };
            match payload::encode(&block) {
                Ok(bytes) if bytes.len() <= max_bytes => {
                    self.last_built = Some((
                        height,
                        view,
                        selected
                            .iter()
                            .map(|(tx, _)| tx.hash_as_entrypoint())
                            .collect(),
                    ));
                    let attest = attestation_required(&block);
                    return (bytes, attest);
                }
                Ok(_) if !selected.is_empty() => {
                    selected.pop();
                }
                Ok(_) => return (Vec::new(), false),
                Err(_) => return (Vec::new(), false),
            }
        }
        (Vec::new(), false)
    }

    /// Our payload for `(height, view)` executed `Invalid`: remove the transactions that make
    /// a block invalid on their own (§4.2), tested one by one on the applied tip.
    fn reject(&mut self, height: u64, view: u64, _block_hash: Hash32) {
        let Some((built_height, built_view, hashes)) = self.last_built.take() else {
            return;
        };
        if (built_height, built_view) != (height, view) || height != self.applied.0 + 1 {
            return;
        }
        let Some(parent) = self.state.view().latest_block() else {
            return;
        };
        let Some(scheduled) = self.scheduled(height) else {
            return;
        };
        let cadence = Duration::from_millis(scheduled.params.block_time_ms);
        let Some(queue) = self.queue.clone() else {
            return;
        };
        // Local pulse availability must never quarantine otherwise valid transactions.
        let Ok(pulse) = self.pulse_for_height(height) else {
            return;
        };
        let queued = payload::select(self.state, &queue, usize::MAX, 0);
        let mut poison = Vec::new();
        for (tx, plan) in queued
            .into_iter()
            .filter(|(tx, _)| hashes.contains(&tx.hash_as_entrypoint()))
            .take(MAX_QUARANTINE_TESTS)
        {
            let hash = tx.hash_as_entrypoint();
            let assembly = Assembly {
                parent: &parent,
                view,
                cadence,
            };
            let Ok(single) =
                payload::assemble_with_pulse(self.state, assembly, &[(tx, plan)], pulse)
            else {
                continue;
            };
            self.live = None;
            let topology = Topology::new(scheduled.committee.clone());
            let verdict = catch_unwind(AssertUnwindSafe(|| {
                ValidBlock::validate_sumeragi_block(
                    single,
                    &topology,
                    &self.context.genesis_account,
                    cadence,
                    self.context.consensus_mode,
                    lanes::merge::LaneStepInput::default(),
                    self.state,
                )
                .unpack(|_| {})
            }));
            if let Ok(Err((_, error))) = verdict {
                if local_failure(&error).is_none() {
                    poison.push(hash);
                }
            }
        }
        if !poison.is_empty() {
            iroha_logger::warn!(
                height,
                view,
                removed = poison.len(),
                "sumeragi: removing transactions that make a block invalid on their own"
            );
            queue.remove_committed_hashes(poison, None);
        }
    }
}

/// The iroha header of a decoded payload must match the certified core header.
fn proposal_matches_header(header: IrohaHeader, block: &Block) -> bool {
    header.height().get() == block.header.height
        && header.view_change_index() == block.header.origin_view
}

/// Whether a block requires commit attestations (§3.7, KAGEMUSHA mint finality): it carries a
/// KAGEMUSHA V1 top-up, which admission confines to single-instruction transactions.
///
/// TODO(WP5c-kagemusha): mint-finality epoch boundaries also require seals; top-ups become
/// proposable once WP8a moves them from QueuePlanSynced to ordinary admission.
#[must_use]
pub fn attestation_required(block: &SignedBlock) -> bool {
    block.external_entrypoints_slice().iter().any(|entrypoint| {
        let TransactionEntrypoint::External(tx) = entrypoint else {
            return false;
        };
        tx.admission_intent() == TransactionAdmissionIntent::QueuePlanSynced
            && tx
                .instructions()
                .explicit_instructions()
                .any(|instruction| {
                    instruction
                        .as_any()
                        .downcast_ref::<iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1>()
                        .is_some()
                })
    })
}

/// Executed top-ups in a block that does not require attestations: the static rule missed a
/// top-up path, so the block cannot be finalized with mint finality.
fn top_ups_without_flag(commitment: &ExecutionResultCommitment, attest: bool) -> bool {
    commitment.execution.kagemusha_top_up_count > 0 && !attest
}

/// A deterministically invalid block at `height`, logged with its reason.
fn invalid(height: u64, reason: &dyn std::fmt::Display) -> ExecOutcome {
    iroha_logger::warn!(height, %reason, "sumeragi: block is invalid");
    ExecOutcome::Invalid
}

/// Local conditions are `Failed` (retried); every other rejection is deterministic.
fn classify(height: u64, error: &BlockValidationError) -> ExecOutcome {
    local_failure(error).map_or_else(|| invalid(height, error), ExecOutcome::Failed)
}

/// The local condition behind `error`, if it is one (not a property of the block).
fn local_failure(error: &BlockValidationError) -> Option<String> {
    match error {
        BlockValidationError::BlockHashAdmission(reason) => {
            Some(format!("block-hash admission: {reason}"))
        }
        BlockValidationError::MembershipAdmission(reason) => {
            Some(format!("membership admission: {reason}"))
        }
        BlockValidationError::DaIndexHydration(reason) => {
            Some(format!("DA index hydration: {reason}"))
        }
        BlockValidationError::LocalStorageRecoveryRequired { reason } => {
            Some(format!("local storage recovery: {reason}"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_encoded_zero_transaction_payloads_are_invalid_without_state_work() {
        use iroha_sumeragi::message::BlockHeader;
        use std::{collections::BTreeSet, num::NonZeroU64};

        let state = Arc::new(State::new_for_testing(
            crate::state::World::new(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        ));
        let parent_hash = Hash32([1; 32]);
        let mut executor = StateExecutor::spawn(ExecutorContext {
            state: Arc::clone(&state),
            queue: None,
            staging: Staging::new(),
            events: tokio::sync::broadcast::channel(16).0,
            genesis_account: iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone(),
            consensus_mode: ConsensusMode::Permissioned,
            applied: (1, parent_hash),
            crypto: None,
            applied_watch: Arc::new(crate::sumeragi::lanes::global::AppliedWatch::new(1, None)),
            lane_blocks: Arc::new(crate::sumeragi::lanes::merge::NoLanes),
        })
        .expect("state executor");
        let zero_transaction_wire = iroha_data_model::block::builder::BlockBuilder::new(
            IrohaHeader::new(NonZeroU64::new(2).unwrap(), None, None, 1, 0),
        )
        .build(BTreeSet::new())
        .encode_wire()
        .unwrap();
        for (index, payload) in [Vec::new(), zero_transaction_wire].into_iter().enumerate() {
            let block = Block {
                header: BlockHeader {
                    instance: Hash32([2; 32]),
                    height: 2,
                    origin_view: 0,
                    parent_hash,
                    parent_result: Hash32([3; 32]),
                    payload_hash: Hash32([4; 32]),
                    payload_len: u32::try_from(payload.len()).unwrap(),
                    proposer: 0,
                    skipped_leaders: Vec::new(),
                    attest: false,
                },
                payload,
            };
            let hash = Hash32([u8::try_from(index + 5).unwrap(); 32]);
            assert!(matches!(
                executor.execute(&block, &hash),
                Some(ExecOutcome::Invalid)
            ));
            assert_eq!(
                state.view().height(),
                0,
                "no synthesized block or overlay committed"
            );
        }
    }

    /// Local conditions are retried (`Failed`); a property of the block is `Invalid`.
    #[test]
    fn classification_table() {
        let local = [
            BlockValidationError::DaIndexHydration("cold".into()),
            BlockValidationError::LocalStorageRecoveryRequired {
                reason: "disk".into(),
            },
        ];
        for error in &local {
            assert!(
                matches!(classify(2, error), ExecOutcome::Failed(_)),
                "{error:?} is local"
            );
        }
        let invalid = [
            BlockValidationError::InvalidGenesis(crate::block::InvalidGenesisError::InvalidHeader),
            BlockValidationError::ExecutionContextInvalid("bad".into()),
            BlockValidationError::HasCommittedTransactions,
        ];
        for error in &invalid {
            assert!(
                matches!(classify(2, error), ExecOutcome::Invalid),
                "{error:?} is a property of the block"
            );
        }
    }
}

#[cfg(test)]
mod archive_tests;
