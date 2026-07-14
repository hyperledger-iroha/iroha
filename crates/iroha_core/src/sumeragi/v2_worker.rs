//! Production service boundary for the single Sumeragi v2 reducer owner.
//!
//! The reducer itself remains serialized on the Sumeragi thread. Potentially
//! blocking signing, body fsync/validation, state application, and certified
//! body serving execute on one ordered I/O worker and return tagged
//! completions. Network effects are sent directly to every frozen voter; no
//! correctness-critical collector or global RBC state exists here.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(test)]
use super::v2_core::Generation;
use super::v2_core::{EquivocationKind, EventTag};
use iroha_crypto::{Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    block::{CertifiedMergeLedgerReference, consensus_v2 as wire, decode_framed_signed_block},
    merge::MergeCommitteeSignature,
    peer::PeerId,
};
use iroha_p2p::{Post, Priority};

use super::{
    message::{BlockMessage, BlockMessageWire},
    output_guard::{ConsensusOutputGuard, ConsensusOutputPermit},
    v2_apply::V2ApplyService,
    v2_body_store::{
        BodyStoreCompletion, BodyValidationCompletion, V2BodyStore, ValidatedBodyReceipt,
    },
    v2_chunks::{EncodedV2Payload, V2ChunkSession, encode_payload},
    v2_effects::{
        ApplyTask, BodyFetchTask, BodyStoreTask, BodyValidationTask, ConsensusSignTask,
        DurableApplyCompletion, EffectExecutorError, EffectExecutorStatus, EffectTransportError,
        EffectWorkId, PostFinalityCleanupOutcome, PostFinalityCleanupTarget, V2EffectExecutor,
        V2EffectServices,
    },
    v2_transport::{AuthenticatedCertifiedBodyRequest, AuthenticatedPayloadChunk},
};
use crate::{
    EventsSender, IrohaNetwork, NetworkMessage, kura::KuraV2CommitReceipt,
    merge_sidecar::CertifiedMergeSidecarMessage,
};

enum V2IoCommand {
    Sign {
        task: ConsensusSignTask,
        restore_outbound_payload: bool,
    },
    Store(BodyStoreTask),
    Validate(BodyValidationTask),
    Apply(ApplyTask),
    Serve(AuthenticatedCertifiedBodyRequest),
    LoadCandidate {
        tag: EventTag,
        subject: wire::BlockSubject,
    },
    Retire(KuraV2CommitReceipt),
    Shutdown,
}

const LOCAL_IO_CONTROL_RESERVE: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2IoAdmissionClass {
    Auxiliary,
    Consensus,
    Control,
}

impl V2IoCommand {
    const fn admission_class(&self) -> V2IoAdmissionClass {
        match self {
            Self::Serve(_) => V2IoAdmissionClass::Auxiliary,
            Self::Sign { .. } | Self::Store(_) | Self::Validate(_) | Self::Apply(_) => {
                V2IoAdmissionClass::Consensus
            }
            Self::LoadCandidate { .. } | Self::Retire(_) | Self::Shutdown => {
                V2IoAdmissionClass::Control
            }
        }
    }

    const fn work_id(&self) -> Option<EffectWorkId> {
        match self {
            Self::Sign { task, .. } => Some(task.id()),
            Self::Store(task) => Some(task.id()),
            Self::Validate(task) => Some(task.id()),
            Self::Apply(task) => Some(task.id()),
            Self::Serve(_) | Self::LoadCandidate { .. } | Self::Retire(_) | Self::Shutdown => None,
        }
    }

    const fn cancellable_kind(&self) -> Option<V2IoCancellableKind> {
        match self {
            Self::Sign { .. } => Some(V2IoCancellableKind::Sign),
            Self::Store(_) => Some(V2IoCancellableKind::Store),
            Self::Validate(_) => Some(V2IoCancellableKind::Validate),
            Self::Apply(_)
            | Self::Serve(_)
            | Self::LoadCandidate { .. }
            | Self::Retire(_)
            | Self::Shutdown => None,
        }
    }

    fn work_descriptor(&self) -> Option<(EffectWorkId, V2IoWorkDescriptor)> {
        match self {
            Self::Sign {
                task,
                restore_outbound_payload,
            } => Some((
                task.id(),
                V2IoWorkDescriptor::Sign {
                    tag: task.tag(),
                    request: task.request().clone(),
                    restore_outbound_payload: *restore_outbound_payload,
                },
            )),
            Self::Store(task) => Some((
                task.id(),
                V2IoWorkDescriptor::Store {
                    tag: task.tag(),
                    manifest_hash: HashOf::new(task.manifest()),
                    canonical_wire_len: task.canonical_wire().len(),
                    canonical_wire_hash: Hash::new(task.canonical_wire()),
                },
            )),
            Self::Validate(task) => Some((
                task.id(),
                V2IoWorkDescriptor::Validate {
                    tag: task.tag(),
                    round: task.round(),
                    subject: task.subject(),
                    durable_receipt: task.durable_receipt().clone(),
                },
            )),
            Self::Apply(task) => Some((
                task.id(),
                V2IoWorkDescriptor::Apply {
                    tag: task.tag(),
                    subject: task.subject(),
                    certificate: task.certificate().clone(),
                    validated_receipt: task.validated_receipt().clone(),
                },
            )),
            Self::Serve(_) | Self::LoadCandidate { .. } | Self::Retire(_) | Self::Shutdown => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum V2IoWorkDescriptor {
    Sign {
        tag: EventTag,
        request: super::v2::SignRequest,
        restore_outbound_payload: bool,
    },
    Store {
        tag: EventTag,
        manifest_hash: HashOf<wire::PayloadManifest>,
        canonical_wire_len: usize,
        canonical_wire_hash: Hash,
    },
    Validate {
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        durable_receipt: super::v2_body_store::DurableBodyReceipt,
    },
    Apply {
        tag: EventTag,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
        validated_receipt: ValidatedBodyReceipt,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2IoCancellableKind {
    Sign,
    Store,
    Validate,
}

impl V2IoWorkDescriptor {
    const fn cancellable_kind(&self) -> Option<V2IoCancellableKind> {
        match self {
            Self::Sign { .. } => Some(V2IoCancellableKind::Sign),
            Self::Store { .. } => Some(V2IoCancellableKind::Store),
            Self::Validate { .. } => Some(V2IoCancellableKind::Validate),
            Self::Apply { .. } => None,
        }
    }
}

/// Hierarchical admission for the single ordered I/O FIFO.
///
/// Admission is based on the total number of queued commands. Remote body
/// service can occupy only the auxiliary prefix, consensus work can also use
/// its reserved suffix, and trusted local control can use the final slot. The
/// worker still consumes one physical FIFO, so admission never reorders work.
struct V2IoAdmission {
    queued: AtomicUsize,
    auxiliary_limit: usize,
    consensus_limit: usize,
    capacity: usize,
}

impl V2IoAdmission {
    fn new(auxiliary_capacity: usize, consensus_capacity: usize) -> Result<Self, String> {
        let consensus_limit = auxiliary_capacity
            .checked_add(consensus_capacity)
            .ok_or_else(|| "Sumeragi v2 I/O queue capacity overflow".to_owned())?;
        let capacity = consensus_limit
            .checked_add(LOCAL_IO_CONTROL_RESERVE)
            .ok_or_else(|| "Sumeragi v2 I/O queue capacity overflow".to_owned())?;
        Ok(Self {
            queued: AtomicUsize::new(0),
            auxiliary_limit: auxiliary_capacity,
            consensus_limit,
            capacity,
        })
    }

    #[cfg(test)]
    fn unbounded_for_tests() -> Arc<Self> {
        Arc::new(Self {
            queued: AtomicUsize::new(0),
            auxiliary_limit: usize::MAX,
            consensus_limit: usize::MAX,
            capacity: usize::MAX,
        })
    }

    const fn capacity(&self) -> usize {
        self.capacity
    }

    const fn limit(&self, class: V2IoAdmissionClass) -> usize {
        match class {
            V2IoAdmissionClass::Auxiliary => self.auxiliary_limit,
            V2IoAdmissionClass::Consensus => self.consensus_limit,
            V2IoAdmissionClass::Control => self.capacity,
        }
    }

    fn has_capacity(&self, class: V2IoAdmissionClass) -> bool {
        self.queued.load(AtomicOrdering::Acquire) < self.limit(class)
    }

    fn try_reserve(&self, class: V2IoAdmissionClass) -> bool {
        let limit = self.limit(class);
        self.queued
            .fetch_update(AtomicOrdering::AcqRel, AtomicOrdering::Acquire, |queued| {
                (queued < limit).then_some(queued + 1)
            })
            .is_ok()
    }

    fn release(&self) {
        let previous = self.queued.fetch_sub(1, AtomicOrdering::AcqRel);
        assert!(
            previous != 0,
            "Sumeragi v2 I/O admission released an unreserved command"
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2IoWorkState {
    Queued,
    Active,
    CompletionPending,
}

#[derive(Debug)]
struct V2IoTrackedWork {
    descriptor: V2IoWorkDescriptor,
    state: V2IoWorkState,
}

struct V2IoCommandQueueState {
    commands: VecDeque<V2IoCommand>,
    work: BTreeMap<EffectWorkId, V2IoTrackedWork>,
    sender_open: bool,
    receiver_open: bool,
}

/// Bounded cancellable FIFO shared by the serialized reducer and I/O worker.
///
/// Work ownership outlives physical queue admission: it remains indexed while
/// active and while its completion waits for serialized delivery. This makes
/// exact retransmission idempotent across every asynchronous race without
/// charging completed work against the hierarchical queue reservations.
struct V2IoCommandQueue {
    capacity: usize,
    admission: Arc<V2IoAdmission>,
    state: Mutex<V2IoCommandQueueState>,
    ready: Condvar,
}

struct V2IoCommandSender {
    queue: Arc<V2IoCommandQueue>,
}

struct V2IoCommandReceiver {
    queue: Arc<V2IoCommandQueue>,
}

enum V2IoTrySendError {
    Full(V2IoCommand),
    Disconnected,
    ConflictingWorkId { work_id: EffectWorkId },
}

impl std::fmt::Debug for V2IoTrySendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(_) => formatter.write_str("Full(..)"),
            Self::Disconnected => formatter.write_str("Disconnected"),
            Self::ConflictingWorkId { work_id } => formatter
                .debug_struct("ConflictingWorkId")
                .field("work_id", work_id)
                .finish(),
        }
    }
}

fn v2_io_command_channel(
    capacity: usize,
    admission: Arc<V2IoAdmission>,
) -> (V2IoCommandSender, V2IoCommandReceiver) {
    let queue = Arc::new(V2IoCommandQueue {
        capacity,
        admission,
        state: Mutex::new(V2IoCommandQueueState {
            commands: VecDeque::with_capacity(capacity.min(1_024)),
            work: BTreeMap::new(),
            sender_open: true,
            receiver_open: true,
        }),
        ready: Condvar::new(),
    });
    (
        V2IoCommandSender {
            queue: Arc::clone(&queue),
        },
        V2IoCommandReceiver { queue },
    )
}

impl V2IoCommandQueue {
    fn lock(&self) -> std::sync::MutexGuard<'_, V2IoCommandQueueState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn try_send_as(
        &self,
        class: V2IoAdmissionClass,
        command: V2IoCommand,
    ) -> Result<(), V2IoTrySendError> {
        let descriptor = command.work_descriptor();
        let mut state = self.lock();
        if !state.receiver_open {
            return Err(V2IoTrySendError::Disconnected);
        }
        if let Some((work_id, descriptor)) = &descriptor
            && let Some(existing) = state.work.get(work_id)
        {
            if existing.descriptor == *descriptor {
                return Ok(());
            }
            return Err(V2IoTrySendError::ConflictingWorkId { work_id: *work_id });
        }
        if state.commands.len() >= self.capacity || !self.admission.try_reserve(class) {
            return Err(V2IoTrySendError::Full(command));
        }
        if let Some((work_id, descriptor)) = descriptor {
            let replaced = state.work.insert(
                work_id,
                V2IoTrackedWork {
                    descriptor,
                    state: V2IoWorkState::Queued,
                },
            );
            debug_assert!(replaced.is_none());
        }
        state.commands.push_back(command);
        drop(state);
        self.ready.notify_one();
        Ok(())
    }

    fn cancel(
        &self,
        work_id: EffectWorkId,
        expected_kind: V2IoCancellableKind,
    ) -> Result<bool, String> {
        let mut state = self.lock();
        let Some(tracked) = state.work.get(&work_id) else {
            return Ok(false);
        };
        if tracked.descriptor.cancellable_kind() != Some(expected_kind) {
            return Err(format!(
                "Sumeragi v2 I/O work {} was reused by a conflicting command",
                work_id.get()
            ));
        }
        if matches!(
            tracked.state,
            V2IoWorkState::Active | V2IoWorkState::CompletionPending
        ) {
            return Ok(false);
        }
        let index = state
            .commands
            .iter()
            .position(|command| command.work_id() == Some(work_id))
            .expect("queued Sumeragi v2 work must have a FIFO owner");
        let removed = state
            .commands
            .remove(index)
            .expect("located Sumeragi v2 work must remain queued");
        debug_assert_eq!(removed.work_id(), Some(work_id));
        debug_assert_eq!(removed.cancellable_kind(), Some(expected_kind));
        state
            .work
            .remove(&work_id)
            .expect("removed Sumeragi v2 work must have an ownership record");
        self.admission.release();
        Ok(true)
    }

    fn recv(&self) -> Result<V2IoCommand, ()> {
        let mut state = self.lock();
        loop {
            if let Some(command) = state.commands.pop_front() {
                self.admission.release();
                if let Some(work_id) = command.work_id() {
                    let tracked = state
                        .work
                        .get_mut(&work_id)
                        .expect("queued Sumeragi v2 command must have an ownership record");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                return Ok(command);
            }
            if !state.sender_open {
                return Err(());
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    #[cfg(test)]
    fn try_recv(&self) -> Result<V2IoCommand, mpsc::TryRecvError> {
        let mut state = self.lock();
        let Some(command) = state.commands.pop_front() else {
            return if state.sender_open {
                Err(mpsc::TryRecvError::Empty)
            } else {
                Err(mpsc::TryRecvError::Disconnected)
            };
        };
        self.admission.release();
        if let Some(work_id) = command.work_id() {
            let tracked = state
                .work
                .get_mut(&work_id)
                .expect("queued Sumeragi v2 command must have an ownership record");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        Ok(command)
    }

    fn complete_work(&self, work_id: EffectWorkId) {
        let mut state = self.lock();
        let tracked = state
            .work
            .get_mut(&work_id)
            .expect("completed Sumeragi v2 work must have an ownership record");
        assert_eq!(tracked.state, V2IoWorkState::Active);
        tracked.state = V2IoWorkState::CompletionPending;
    }

    fn acknowledge_completion(&self, work_id: EffectWorkId) {
        let mut state = self.lock();
        let tracked = state
            .work
            .remove(&work_id)
            .expect("delivered Sumeragi v2 completion must have an ownership record");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
    }

    fn close_sender(&self) {
        let mut state = self.lock();
        state.sender_open = false;
        drop(state);
        self.ready.notify_all();
    }

    fn close_receiver(&self) {
        let mut state = self.lock();
        if !state.receiver_open {
            return;
        }
        state.receiver_open = false;
        let queued = state.commands.len();
        state.commands.clear();
        // A normal Shutdown/Retire exit can close the command receiver while
        // already-sent completions remain buffered. Keep those ownership
        // records until the serialized handle drains and acknowledges them.
        state
            .work
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        for _ in 0..queued {
            self.admission.release();
        }
        drop(state);
        self.ready.notify_all();
    }
}

impl V2IoCommandSender {
    #[cfg(test)]
    fn try_send(&self, command: V2IoCommand) -> Result<(), V2IoTrySendError> {
        self.queue.try_send_as(command.admission_class(), command)
    }

    fn try_send_as(
        &self,
        class: V2IoAdmissionClass,
        command: V2IoCommand,
    ) -> Result<(), V2IoTrySendError> {
        self.queue.try_send_as(class, command)
    }

    fn cancel(
        &self,
        work_id: EffectWorkId,
        expected_kind: V2IoCancellableKind,
    ) -> Result<bool, String> {
        self.queue.cancel(work_id, expected_kind)
    }

    fn acknowledge_completion(&self, work_id: EffectWorkId) {
        self.queue.acknowledge_completion(work_id);
    }
}

impl Drop for V2IoCommandSender {
    fn drop(&mut self) {
        self.queue.close_sender();
    }
}

impl V2IoCommandReceiver {
    fn recv(&self) -> Result<V2IoCommand, ()> {
        self.queue.recv()
    }

    #[cfg(test)]
    fn try_recv(&self) -> Result<V2IoCommand, mpsc::TryRecvError> {
        self.queue.try_recv()
    }

    #[cfg(test)]
    fn try_iter(&self) -> V2IoCommandTryIter<'_> {
        V2IoCommandTryIter { receiver: self }
    }

    fn complete_work(&self, work_id: EffectWorkId) {
        self.queue.complete_work(work_id);
    }
}

impl Drop for V2IoCommandReceiver {
    fn drop(&mut self) {
        self.queue.close_receiver();
    }
}

#[cfg(test)]
struct V2IoCommandTryIter<'a> {
    receiver: &'a V2IoCommandReceiver,
}

#[cfg(test)]
impl Iterator for V2IoCommandTryIter<'_> {
    type Item = V2IoCommand;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.try_recv().ok()
    }
}

enum V2IoCompletion {
    Signature {
        work_id: EffectWorkId,
        signature: Vec<u8>,
        outbound_payload: Option<EncodedV2Payload>,
    },
    Stored(BodyStoreCompletion),
    Validated(BodyValidationCompletion),
    Applied(DurableApplyCompletion),
    ApplyDeferred {
        work_id: EffectWorkId,
        reference: CertifiedMergeLedgerReference,
    },
    CertifiedResponse {
        recipient: PeerId,
        response: wire::CertifiedBodyResponse,
    },
    CertifiedRequestIgnored,
    CandidateLoaded(LoadedCandidateBody),
    Retired,
    RetirementFailed(String),
    RecoveryRequired(String),
    Failed(String),
}

impl V2IoCompletion {
    fn work_id(&self) -> Option<EffectWorkId> {
        match self {
            Self::Signature { work_id, .. } | Self::ApplyDeferred { work_id, .. } => Some(*work_id),
            Self::Stored(completion) => Some(completion.work_id()),
            Self::Validated(completion) => Some(completion.work_id()),
            Self::Applied(completion) => Some(completion.work_id()),
            Self::CertifiedResponse { .. }
            | Self::CertifiedRequestIgnored
            | Self::CandidateLoaded(_)
            | Self::Retired
            | Self::RetirementFailed(_)
            | Self::RecoveryRequired(_)
            | Self::Failed(_) => None,
        }
    }
}

struct V2IoHandle {
    command_tx: V2IoCommandSender,
    completion_rx: mpsc::Receiver<V2IoCompletion>,
    join: Option<thread::JoinHandle<()>>,
    allow_finalized_disconnect: Arc<AtomicBool>,
    admission: Arc<V2IoAdmission>,
}

struct V2IoWorkerFailureGuard {
    output_guard: Arc<ConsensusOutputGuard>,
    allow_finalized_disconnect: Arc<AtomicBool>,
    armed: bool,
}

impl V2IoWorkerFailureGuard {
    fn new(
        output_guard: Arc<ConsensusOutputGuard>,
        allow_finalized_disconnect: Arc<AtomicBool>,
    ) -> Self {
        Self {
            output_guard,
            allow_finalized_disconnect,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for V2IoWorkerFailureGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if thread::panicking() {
            self.output_guard.close_admission_for_restart();
        } else if !self
            .allow_finalized_disconnect
            .load(AtomicOrdering::Acquire)
        {
            self.output_guard.activate_restart_required();
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CleanupWorkerIdentity {
    height: u64,
    context_id: wire::HeightContextId,
    block_hash: HashOf<iroha_data_model::block::BlockHeader>,
}

impl CleanupWorkerIdentity {
    fn from_receipt(receipt: &KuraV2CommitReceipt) -> Self {
        Self {
            height: receipt.height(),
            context_id: receipt.context_id(),
            block_hash: receipt.block_hash(),
        }
    }
}

struct SupervisedCleanupWorker {
    identity: CleanupWorkerIdentity,
    join: thread::JoinHandle<()>,
}

/// Runner-owned reaper for cleanup workers which outlive their configured
/// post-finality response deadline.
///
/// Timed-out workers remain supervised instead of being detached. Finished
/// workers are reaped during subsequent height processing; shutdown joins any
/// remaining workers after their command/completion channels have been closed.
#[derive(Default)]
pub(crate) struct V2CleanupSupervisor {
    workers: Vec<SupervisedCleanupWorker>,
}

impl V2CleanupSupervisor {
    fn supervise(&mut self, identity: CleanupWorkerIdentity, join: thread::JoinHandle<()>) {
        self.workers
            .push(SupervisedCleanupWorker { identity, join });
    }

    /// Reap every completed cleanup worker without blocking height processing.
    pub(crate) fn reap_finished(&mut self) {
        let mut pending = Vec::with_capacity(self.workers.len());
        for worker in std::mem::take(&mut self.workers) {
            if worker.join.is_finished() {
                report_cleanup_worker_join(worker);
            } else {
                pending.push(worker);
            }
        }
        self.workers = pending;
    }

    #[cfg(test)]
    fn pending_workers(&self) -> usize {
        self.workers.len()
    }
}

impl Drop for V2CleanupSupervisor {
    fn drop(&mut self) {
        for worker in std::mem::take(&mut self.workers) {
            report_cleanup_worker_join(worker);
        }
    }
}

fn report_cleanup_worker_join(worker: SupervisedCleanupWorker) {
    if worker.join.join().is_err() {
        iroha_logger::warn!(
            height = worker.identity.height,
            context_id = ?worker.identity.context_id,
            block_hash = %worker.identity.block_hash,
            cleanup_target = PostFinalityCleanupTarget::CleanupWorker.as_str(),
            reason = "Sumeragi v2 I/O worker panicked during supervised finalized cleanup",
            "Sumeragi v2 finalized with retained local cleanup state"
        );
    }
}

impl V2IoHandle {
    fn spawn(
        mut body_store: V2BodyStore,
        apply_service: V2ApplyService,
        context: wire::HeightContext,
        key_pair: KeyPair,
        local_validator: Option<wire::ValidatorIndex>,
        auxiliary_queue_capacity: usize,
        consensus_queue_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<Self, String> {
        let admission = Arc::new(V2IoAdmission::new(
            auxiliary_queue_capacity,
            consensus_queue_capacity,
        )?);
        let capacity = admission.capacity();
        let (command_tx, command_rx) = v2_io_command_channel(capacity, Arc::clone(&admission));
        let (completion_tx, completion_rx) = mpsc::sync_channel(capacity);
        let allow_finalized_disconnect = Arc::new(AtomicBool::new(false));
        let worker_allow_finalized_disconnect = Arc::clone(&allow_finalized_disconnect);
        let join = super::sumeragi_thread_builder("sumeragi-v2-io")
            .spawn(move || {
                // A local guard drops before the closure environment releases
                // command/completion channels, closing output first on panic
                // or an implicit producer disconnect.
                let mut worker_failure_guard = V2IoWorkerFailureGuard::new(
                    Arc::clone(&output_guard),
                    worker_allow_finalized_disconnect,
                );
                while let Ok(command) = command_rx.recv() {
                    let work_id = command.work_id();
                    match command {
                        V2IoCommand::Retire(receipt) => {
                            let Some(completion) = execute_retire_io_command(&output_guard, || {
                                body_store
                                    .retire_height(&receipt)
                                    .map_err(|error| error.to_string())
                            }) else {
                                break;
                            };
                            let _ = completion_tx.send(completion);
                            worker_failure_guard.disarm();
                            break;
                        }
                        V2IoCommand::Shutdown => {
                            worker_failure_guard.disarm();
                            break;
                        }
                        command => {
                            let completion = execute_fail_stop_io_command(&output_guard, || {
                                match command {
                                    V2IoCommand::Sign {
                                        task,
                                        restore_outbound_payload,
                                    } => sign_consensus_task(
                                        &body_store,
                                        &context,
                                        &key_pair,
                                        task,
                                        restore_outbound_payload,
                                    ),
                                    V2IoCommand::Store(task) => body_store
                                        .execute_store_task(&task)
                                        .map(V2IoCompletion::Stored)
                                        .map_err(|error| error.to_string()),
                                    V2IoCommand::Validate(task) => body_store
                                        .execute_validation_task(&task, |body| {
                                            apply_service.validate_candidate(&context, body)
                                        })
                                        .map(V2IoCompletion::Validated)
                                        .map_err(|error| error.to_string()),
                                    V2IoCommand::Apply(task) => match apply_service.execute(
                                        &context,
                                        &mut body_store,
                                        &task,
                                    ) {
                                        Ok(completion) => Ok(V2IoCompletion::Applied(completion)),
                                        Err(
                                            super::v2_apply::V2ApplyError::MissingCertifiedMergeSidecar {
                                                reference,
                                            },
                                        ) => Ok(V2IoCompletion::ApplyDeferred {
                                            work_id: task.id(),
                                            reference,
                                        }),
                                        Err(error) if error.requires_restart_recovery() => {
                                            Ok(V2IoCompletion::RecoveryRequired(error.to_string()))
                                        }
                                        Err(error) => Err(error.to_string()),
                                    },
                                    V2IoCommand::Serve(request) => serve_certified_body(
                                        &body_store,
                                        &key_pair,
                                        local_validator,
                                        request,
                                    ),
                                    V2IoCommand::LoadCandidate { tag, subject } => {
                                        load_candidate_body(&body_store, tag, subject)
                                    }
                                    V2IoCommand::Retire(_) | V2IoCommand::Shutdown => {
                                        unreachable!(
                                            "cleanup commands handled before fail-stop I/O"
                                        )
                                    }
                                }
                            });
                            let failed = match completion {
                                Err(reason) => {
                                    if let Some(work_id) = work_id {
                                        command_rx.complete_work(work_id);
                                    }
                                    let _ = completion_tx
                                        .try_send(V2IoCompletion::RecoveryRequired(reason));
                                    true
                                }
                                Ok(completion) => {
                                    if let Some(work_id) = work_id {
                                        command_rx.complete_work(work_id);
                                    }
                                    send_completion(&completion_tx, Ok(completion));
                                    false
                                }
                            };
                            if failed {
                                break;
                            }
                        }
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            command_tx,
            completion_rx,
            join: Some(join),
            allow_finalized_disconnect,
            admission,
        })
    }

    fn enqueue(&self, command: V2IoCommand) -> Result<(), String> {
        self.try_enqueue(command).map_err(|error| match error {
            V2IoTrySendError::Full(_) => "Sumeragi v2 I/O queue is full".to_owned(),
            V2IoTrySendError::Disconnected => "Sumeragi v2 I/O worker is disconnected".to_owned(),
            V2IoTrySendError::ConflictingWorkId { work_id } => format!(
                "Sumeragi v2 I/O work {} was reused by a conflicting command",
                work_id.get()
            ),
        })
    }

    fn try_enqueue(&self, command: V2IoCommand) -> Result<(), V2IoTrySendError> {
        let class = command.admission_class();
        self.try_enqueue_as(class, command)
    }

    fn try_enqueue_as(
        &self,
        class: V2IoAdmissionClass,
        command: V2IoCommand,
    ) -> Result<(), V2IoTrySendError> {
        self.command_tx.try_send_as(class, command)
    }

    fn can_enqueue_as(&self, class: V2IoAdmissionClass) -> bool {
        self.admission.has_capacity(class)
    }

    fn cancel(
        &self,
        work_id: EffectWorkId,
        expected_kind: V2IoCancellableKind,
    ) -> Result<bool, String> {
        self.command_tx.cancel(work_id, expected_kind)
    }

    fn acknowledge_completion(&self, completion: &V2IoCompletion) {
        if let Some(work_id) = completion.work_id() {
            self.command_tx.acknowledge_completion(work_id);
        }
    }

    fn try_recv_completion(&self) -> Result<V2IoCompletion, mpsc::TryRecvError> {
        let completion = self.completion_rx.try_recv()?;
        self.acknowledge_completion(&completion);
        Ok(completion)
    }

    fn recv_completion(&self) -> Result<V2IoCompletion, mpsc::RecvError> {
        let completion = self.completion_rx.recv()?;
        self.acknowledge_completion(&completion);
        Ok(completion)
    }

    fn recv_completion_timeout(
        &self,
        timeout: Duration,
    ) -> Result<V2IoCompletion, mpsc::RecvTimeoutError> {
        let completion = self.completion_rx.recv_timeout(timeout)?;
        self.acknowledge_completion(&completion);
        Ok(completion)
    }

    fn shutdown(mut self) -> Result<(), String> {
        let mut command = V2IoCommand::Shutdown;
        loop {
            match self.try_enqueue(command) {
                Ok(()) => break,
                Err(V2IoTrySendError::Full(returned)) => {
                    command = returned;
                    if self.recv_completion().is_err() {
                        break;
                    }
                }
                Err(V2IoTrySendError::Disconnected) => break,
                Err(V2IoTrySendError::ConflictingWorkId { .. }) => {
                    unreachable!("shutdown commands do not carry work identifiers");
                }
            }
        }
        // The worker can have commands ahead of Shutdown. Drain their bounded
        // completions so it can reach Shutdown without a cyclic channel wait.
        while self.recv_completion().is_ok() {}
        if let Some(join) = self.join.take() {
            join.join()
                .map_err(|_| "Sumeragi v2 I/O worker panicked".to_owned())?;
        }
        Ok(())
    }
}

fn send_completion(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    completion: Result<V2IoCompletion, String>,
) {
    let completion = completion.unwrap_or_else(V2IoCompletion::Failed);
    let _ = sender.send(completion);
}

fn execute_fail_stop_io_command(
    output_guard: &ConsensusOutputGuard,
    execute: impl FnOnce() -> Result<V2IoCompletion, String>,
) -> Result<V2IoCompletion, String> {
    let operation = output_guard
        .begin_fail_stop_operation()
        .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
    match execute() {
        Ok(V2IoCompletion::RecoveryRequired(reason)) | Err(reason) => {
            drop(operation);
            Err(reason)
        }
        Ok(completion) => {
            operation.complete();
            Ok(completion)
        }
    }
}

fn execute_retire_io_command(
    output_guard: &ConsensusOutputGuard,
    retire: impl FnOnce() -> Result<(), String>,
) -> Option<V2IoCompletion> {
    let operation = output_guard.begin_fail_stop_operation()?;
    match retire() {
        Ok(()) => {
            operation.complete();
            Some(V2IoCompletion::Retired)
        }
        Err(reason) => {
            // Retirement failure is classified post-finality cleanup only.
            // Complete it normally before publishing the completion; an
            // unwind in `retire` instead drops the armed operation and poisons
            // this process.
            operation.complete();
            Some(V2IoCompletion::RetirementFailed(reason))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CleanupCompletionWaitError {
    DeadlineElapsed,
    Disconnected,
}

fn recv_cleanup_completion(
    io: &V2IoHandle,
    deadline: Instant,
) -> Result<V2IoCompletion, CleanupCompletionWaitError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(CleanupCompletionWaitError::DeadlineElapsed)?;
    io.recv_completion_timeout(remaining)
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => CleanupCompletionWaitError::DeadlineElapsed,
            mpsc::RecvTimeoutError::Disconnected => CleanupCompletionWaitError::Disconnected,
        })
}

fn sign_consensus_task(
    body_store: &V2BodyStore,
    context: &wire::HeightContext,
    key_pair: &KeyPair,
    task: ConsensusSignTask,
    restore_outbound_payload: bool,
) -> Result<V2IoCompletion, String> {
    let (preimage, outbound_payload) = match task.request() {
        super::v2::SignRequest::Proposal(proposal) => {
            let outbound_payload = restore_outbound_payload
                .then(|| recover_outbound_proposal_payload(body_store, context, proposal))
                .transpose()?;
            (proposal.signature_preimage(), outbound_payload)
        }
        super::v2::SignRequest::Vote(vote) => (vote.signature_preimage(), None),
        super::v2::SignRequest::TimeoutVote(vote) => (vote.signature_preimage(), None),
    };
    Signature::try_new(key_pair.private_key(), &preimage)
        .map(|signature| V2IoCompletion::Signature {
            work_id: task.id(),
            signature: signature.payload().to_vec(),
            outbound_payload,
        })
        .map_err(|error| error.to_string())
}

fn recover_outbound_proposal_payload(
    body_store: &V2BodyStore,
    context: &wire::HeightContext,
    proposal: &wire::Proposal,
) -> Result<EncodedV2Payload, String> {
    let (stored_manifest, receipt) = body_store
        .recovered(proposal.round, proposal.subject)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "replayed local proposal has no durable exact body".to_owned())?;
    if stored_manifest != proposal.manifest {
        return Err("replayed local proposal differs from its durable manifest".to_owned());
    }
    let canonical_wire = body_store
        .load_canonical_wire(&receipt)
        .map_err(|error| error.to_string())?;
    let payload = encode_payload(context, proposal.round, proposal.subject, &canonical_wire)
        .map_err(|error| error.to_string())?;
    if payload.manifest() != &proposal.manifest {
        return Err(
            "replayed local proposal payload does not reproduce its durable manifest".to_owned(),
        );
    }
    Ok(payload)
}

fn serve_certified_body(
    body_store: &V2BodyStore,
    key_pair: &KeyPair,
    local_validator: Option<wire::ValidatorIndex>,
    authenticated: AuthenticatedCertifiedBodyRequest,
) -> Result<V2IoCompletion, String> {
    let request = authenticated.request();
    let Some(responder) = local_validator else {
        return Ok(V2IoCompletion::CertifiedRequestIgnored);
    };
    if request
        .certificate
        .signers
        .binary_search(&responder)
        .is_err()
    {
        return Ok(V2IoCompletion::CertifiedRequestIgnored);
    }
    let (manifest, receipt) = body_store
        .recovered(request.round, request.subject)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "certified Sumeragi v2 body is not retained locally".to_owned())?;
    let body = body_store
        .load_canonical_wire(&receipt)
        .map_err(|error| error.to_string())?;
    let decoded = decode_framed_signed_block(&body).map_err(|error| error.to_string())?;
    if !decoded.is_resultless_proposal() {
        return Err("certified Sumeragi v2 body must be resultless".to_owned());
    }
    let mut response = wire::CertifiedBodyResponse {
        request_hash: authenticated.request_hash(),
        manifest,
        body,
        responder,
        signature: Vec::new(),
    };
    response.signature = Signature::try_new(key_pair.private_key(), &response.signature_preimage())
        .map_err(|error| error.to_string())?
        .payload()
        .to_vec();
    Ok(V2IoCompletion::CertifiedResponse {
        recipient: request.requester.clone(),
        response,
    })
}

fn load_candidate_body(
    body_store: &V2BodyStore,
    tag: EventTag,
    subject: wire::BlockSubject,
) -> Result<V2IoCompletion, String> {
    let (_, receipt) = body_store
        .latest_for_subject(subject)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "locked Sumeragi v2 subject has no durable local body".to_owned())?;
    let canonical_wire = body_store
        .load_canonical_wire(&receipt)
        .map_err(|error| error.to_string())?;
    let decoded = decode_framed_signed_block(&canonical_wire).map_err(|error| error.to_string())?;
    if !decoded.is_resultless_proposal() {
        return Err("locked Sumeragi v2 body must be resultless".to_owned());
    }
    Ok(V2IoCompletion::CandidateLoaded(LoadedCandidateBody {
        tag,
        subject,
        canonical_wire,
    }))
}

#[derive(Debug)]
struct FetchSession {
    task: BodyFetchTask,
    chunks: Option<V2ChunkSession>,
}

#[derive(Clone, Debug)]
struct BufferedPayloadChunk {
    sender: PeerId,
    chunk: wire::PayloadChunk,
}

/// Result of routing one payload chunk through the bounded reorder buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PayloadChunkDisposition {
    /// The chunk reached an active authenticated reconstruction session.
    Delivered,
    /// Proposal processing has not opened the matching session yet.
    Buffered,
    /// An exact buffered retransmission was already retained.
    Duplicate,
    /// The unauthenticated chunk failed a cheap bound/identity check or a full
    /// authentication check and was discarded without affecting consensus.
    Rejected,
}

enum LocalCompletion {
    Reconstructed {
        task: BodyFetchTask,
        manifest: wire::PayloadManifest,
        body: Vec<u8>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyFetchServiceOwner {
    None,
    Live,
    Reconstructed(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionSource {
    Io,
    Local,
}

enum PendingServiceCompletion {
    Io(V2IoCompletion),
    Local(LocalCompletion),
}

/// Exact durable bytes loaded for a locked-subject re-proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadedCandidateBody {
    tag: EventTag,
    subject: wire::BlockSubject,
    canonical_wire: Vec<u8>,
}

/// Deterministic body rejection surfaced to local candidate scheduling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RejectedCandidateBody {
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    reason: String,
}

/// Exact body/reference tuple retained when validation or decided application
/// reports that only its certified merge sidecar is unavailable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeferredMergeSidecarWork {
    work_id: EffectWorkId,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    reference: CertifiedMergeLedgerReference,
}

impl DeferredMergeSidecarWork {
    /// Exact executor work identifier owning this deferral.
    pub(crate) const fn work_id(&self) -> EffectWorkId {
        self.work_id
    }

    /// Wire proposal round retaining the exact durable work item.
    pub(crate) const fn round(&self) -> wire::ConsensusRound {
        self.round
    }

    /// Exact certified subject waiting for recovery.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }

    /// Complete compact reference recovered from the durable body.
    pub(crate) const fn reference(&self) -> &CertifiedMergeLedgerReference {
        &self.reference
    }
}

/// Exact body for which the reducer durably persisted local Prepare intent and
/// released the corresponding signing effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreparedCandidateBody {
    tag: EventTag,
    subject: wire::BlockSubject,
}

impl PreparedCandidateBody {
    /// Reducer incarnation which persisted Prepare intent.
    pub(crate) const fn tag(self) -> EventTag {
        self.tag
    }

    /// Exact subject covered by Prepare intent.
    pub(crate) const fn subject(self) -> wire::BlockSubject {
        self.subject
    }
}

impl RejectedCandidateBody {
    /// Round whose exact durable body failed validation.
    pub(crate) const fn round(&self) -> wire::ConsensusRound {
        self.round
    }

    /// Rejected exact subject.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }

    /// Deterministic validator diagnostic.
    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }
}

impl LoadedCandidateBody {
    /// Reducer incarnation which requested the load.
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }

    /// Locked subject whose exact body was loaded.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }

    /// Consume the completion into exact canonical bytes.
    pub(crate) fn into_canonical_wire(self) -> Vec<u8> {
        self.canonical_wire
    }
}

/// Concrete effect services used by the live v2 height runner.
pub(crate) struct ProductionV2Services {
    context: wire::HeightContext,
    local_peer: PeerId,
    local_validator: Option<wire::ValidatorIndex>,
    key_pair: KeyPair,
    network: IrohaNetwork,
    chunk_root: PathBuf,
    io: Option<V2IoHandle>,
    fetches: BTreeMap<EffectWorkId, FetchSession>,
    fetch_by_manifest: BTreeMap<HashOf<wire::PayloadManifest>, EffectWorkId>,
    orphan_chunks: BTreeMap<HashOf<wire::PayloadManifest>, VecDeque<BufferedPayloadChunk>>,
    orphan_chunk_count: usize,
    orphan_chunk_bytes: u64,
    max_orphan_chunks: usize,
    max_orphan_chunk_bytes: u64,
    max_merge_sidecar_deferrals: usize,
    local_completions: VecDeque<LocalCompletion>,
    next_completion_source: CompletionSource,
    pending_candidate_loads: BTreeSet<EventTag>,
    loaded_candidates: VecDeque<LoadedCandidateBody>,
    prepared_candidates: VecDeque<PreparedCandidateBody>,
    validation_rejections: VecDeque<RejectedCandidateBody>,
    merge_sidecar_deferrals: VecDeque<DeferredMergeSidecarWork>,
    outbound_chunks: BTreeMap<HashOf<wire::PayloadManifest>, Vec<wire::ConsensusMessageV2>>,
    entered_view: Option<EventTag>,
    last_status: Option<EffectExecutorStatus>,
    fatal_reason: Option<String>,
    output_guard: Arc<ConsensusOutputGuard>,
    clean_teardown: bool,
}

impl ProductionV2Services {
    /// Start the ordered I/O adapter for one immutable height context.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start(
        context: wire::HeightContext,
        validator_set_pops: Vec<Vec<u8>>,
        local_peer: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: KeyPair,
        network: IrohaNetwork,
        chunk_root: impl AsRef<Path>,
        body_store: V2BodyStore,
        state: Arc<crate::state::State>,
        queue: Arc<crate::queue::Queue>,
        kura: Arc<crate::kura::Kura>,
        block_cadence: Duration,
        genesis_account: iroha_data_model::account::AccountId,
        events_sender: EventsSender,
        consensus_io_capacity: usize,
        auxiliary_io_capacity: usize,
        orphan_chunk_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<Self, String> {
        let construction_guard = Arc::clone(&output_guard);
        let construction = construction_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if consensus_io_capacity == 0 || auxiliary_io_capacity == 0 || orphan_chunk_capacity == 0 {
            return Err("Sumeragi v2 service queue capacities must be non-zero".to_owned());
        }
        let context_chunk_root = chunk_root
            .as_ref()
            .join(hex::encode(context.id().0.as_ref()));
        let max_orphan_chunk_bytes = u64::from(context.da_layout.max_chunk_count)
            .saturating_mul(u64::from(context.da_layout.chunk_size_bytes));
        std::fs::create_dir_all(&context_chunk_root).map_err(|error| error.to_string())?;
        let apply_service = V2ApplyService::new(
            state,
            queue,
            Arc::clone(&kura),
            context.chain_id.clone(),
            block_cadence,
            genesis_account,
            events_sender,
            validator_set_pops,
        );
        let io = V2IoHandle::spawn(
            body_store,
            apply_service,
            context.clone(),
            key_pair.clone(),
            local_validator,
            auxiliary_io_capacity,
            consensus_io_capacity,
            Arc::clone(&output_guard),
        )?;
        let mut service = Self {
            context,
            local_peer,
            local_validator,
            key_pair,
            network,
            chunk_root: context_chunk_root,
            io: Some(io),
            fetches: BTreeMap::new(),
            fetch_by_manifest: BTreeMap::new(),
            orphan_chunks: BTreeMap::new(),
            orphan_chunk_count: 0,
            orphan_chunk_bytes: 0,
            max_orphan_chunks: orphan_chunk_capacity,
            max_orphan_chunk_bytes,
            max_merge_sidecar_deferrals: consensus_io_capacity,
            local_completions: VecDeque::new(),
            next_completion_source: CompletionSource::Io,
            pending_candidate_loads: BTreeSet::new(),
            loaded_candidates: VecDeque::new(),
            prepared_candidates: VecDeque::new(),
            validation_rejections: VecDeque::new(),
            merge_sidecar_deferrals: VecDeque::new(),
            outbound_chunks: BTreeMap::new(),
            entered_view: None,
            last_status: None,
            fatal_reason: None,
            output_guard,
            // The enclosing construction operation owns abnormal-exit
            // activation until its permit is released. This avoids a nested
            // activation deadlock if `service` unwinds before construction is
            // explicitly completed.
            clean_teardown: true,
        };
        construction.complete();
        service.clean_teardown = false;
        Ok(service)
    }

    /// Sign and retain all canonical chunks for proposal and retransmission.
    pub(crate) fn register_outbound_payload(
        &mut self,
        payload: EncodedV2Payload,
    ) -> Result<wire::PayloadManifest, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard.begin_fail_stop_operation().ok_or_else(|| {
            "Sumeragi v2 canonical persistence requires restart recovery".to_owned()
        })?;
        let sender = self
            .local_validator
            .ok_or_else(|| "observer cannot disperse a Sumeragi v2 proposal".to_owned())?;
        let (manifest, chunks) = payload.into_parts();
        manifest
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let manifest_hash = HashOf::new(&manifest);
        let mut messages = Vec::with_capacity(chunks.len());
        for (index, bytes) in chunks.into_iter().enumerate() {
            let mut chunk = wire::PayloadChunk {
                manifest_hash,
                index: u32::try_from(index)
                    .map_err(|_| "Sumeragi v2 chunk index overflow".to_owned())?,
                bytes,
                sender,
                signature: Vec::new(),
            };
            let preimage = chunk
                .signature_preimage(&self.context, &manifest)
                .map_err(|error| error.to_string())?;
            chunk.signature = Signature::try_new(self.key_pair.private_key(), &preimage)
                .map_err(|error| error.to_string())?
                .payload()
                .to_vec();
            messages.push(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::PayloadChunk(chunk),
            ));
        }
        if let Some(existing) = self.outbound_chunks.get(&manifest_hash) {
            if existing != &messages {
                return Err("conflicting local Sumeragi v2 payload manifest".to_owned());
            }
        } else {
            self.outbound_chunks.insert(manifest_hash, messages);
        }
        operation.complete();
        Ok(manifest)
    }

    /// Work identifier waiting for a chunk from one manifest.
    pub(crate) fn fetch_work_for_manifest(
        &self,
        manifest_hash: HashOf<wire::PayloadManifest>,
    ) -> Option<EffectWorkId> {
        self.fetch_by_manifest.get(&manifest_hash).copied()
    }

    fn body_fetch_service_owner(
        &self,
        work_id: EffectWorkId,
    ) -> Result<BodyFetchServiceOwner, String> {
        let mut queued_index = None;
        for (index, completion) in self.local_completions.iter().enumerate() {
            if matches!(
                completion,
                LocalCompletion::Reconstructed {
                    task,
                    ..
                } if task.id() == work_id
            ) && queued_index.replace(index).is_some()
            {
                return Err(format!(
                    "Sumeragi v2 body-fetch work {} has duplicate queued reconstruction owners",
                    work_id.get()
                ));
            }
        }
        let live = self.fetches.get(&work_id);
        if live.is_some() && queued_index.is_some() {
            return Err(format!(
                "Sumeragi v2 body-fetch work {} has conflicting service owners",
                work_id.get()
            ));
        }
        let indexed_manifests = self
            .fetch_by_manifest
            .iter()
            .filter_map(|(manifest, owner)| (*owner == work_id).then_some(*manifest))
            .collect::<Vec<_>>();

        if let Some(fetch) = live {
            match (fetch.task.manifest(), fetch.chunks.as_ref()) {
                (Some(manifest), Some(session)) => {
                    let expected_hash = HashOf::new(manifest);
                    if session.manifest() != manifest
                        || indexed_manifests.len() != 1
                        || indexed_manifests.first() != Some(&expected_hash)
                        || self.fetch_by_manifest.get(&expected_hash) != Some(&work_id)
                    {
                        return Err(format!(
                            "Sumeragi v2 body-fetch work {} has a mismatched manifest owner",
                            work_id.get()
                        ));
                    }
                }
                (None, None) if indexed_manifests.is_empty() => {}
                _ => {
                    return Err(format!(
                        "Sumeragi v2 body-fetch work {} has inconsistent live acquisition state",
                        work_id.get()
                    ));
                }
            }
            return Ok(BodyFetchServiceOwner::Live);
        }

        if let Some(index) = queued_index {
            let LocalCompletion::Reconstructed { task, manifest, .. } = self
                .local_completions
                .get(index)
                .expect("queued reconstruction index came from this queue");
            if !task.matches_reconstructed_manifest(manifest)
                || !indexed_manifests.is_empty()
                || self.fetch_by_manifest.contains_key(&HashOf::new(manifest))
            {
                return Err(format!(
                    "Sumeragi v2 completed body-fetch work {} has inconsistent manifest ownership",
                    work_id.get()
                ));
            }
            return Ok(BodyFetchServiceOwner::Reconstructed(index));
        }

        if !indexed_manifests.is_empty() {
            return Err(format!(
                "Sumeragi v2 body-fetch work {} has an orphaned manifest owner",
                work_id.get()
            ));
        }
        Ok(BodyFetchServiceOwner::None)
    }

    fn remove_exact_body_fetch_owner(&mut self, task: &BodyFetchTask) -> Result<(), String> {
        match self.body_fetch_service_owner(task.id())? {
            BodyFetchServiceOwner::Live => {
                let existing = self
                    .fetches
                    .get(&task.id())
                    .expect("live body-fetch owner was classified above");
                if existing.task != *task {
                    return Err(format!(
                        "Sumeragi v2 body-fetch work {} differs from executor ownership",
                        task.id().get()
                    ));
                }
                let manifest_hash = existing.task.manifest().map(HashOf::new);
                self.fetches
                    .remove(&task.id())
                    .expect("live body-fetch owner was classified above");
                if let Some(manifest_hash) = manifest_hash {
                    let removed = self.fetch_by_manifest.remove(&manifest_hash);
                    debug_assert_eq!(removed, Some(task.id()));
                }
            }
            BodyFetchServiceOwner::Reconstructed(index) => {
                let LocalCompletion::Reconstructed {
                    task: queued_task, ..
                } = self
                    .local_completions
                    .get(index)
                    .expect("queued body-fetch owner was classified above");
                if queued_task != task {
                    return Err(format!(
                        "Sumeragi v2 reconstructed work {} differs from executor ownership",
                        task.id().get()
                    ));
                }
                self.local_completions
                    .remove(index)
                    .expect("queued body-fetch owner was classified above");
            }
            BodyFetchServiceOwner::None => {
                return Err(format!(
                    "Sumeragi v2 body-fetch work {} has no service owner",
                    task.id().get()
                ));
            }
        }
        Ok(())
    }

    /// Whether the auxiliary I/O prefix can accept a certified-body service request.
    pub(crate) fn can_serve_certified_request(&self) -> bool {
        // An absent worker is not capacity backpressure: allow dequeue so the
        // subsequent enqueue reports the fatal service failure to the runner.
        self.io
            .as_ref()
            .is_none_or(|io| io.can_enqueue_as(V2IoAdmissionClass::Auxiliary))
    }

    /// Queue a previously authenticated certified-body request for service.
    pub(crate) fn serve_certified_request(
        &mut self,
        request: AuthenticatedCertifiedBodyRequest,
    ) -> Result<(), String> {
        self.enqueue_io(V2IoCommand::Serve(request))
    }

    /// Load the exact durable body required by a lock-constrained proposal.
    /// Repeated requests for one reducer incarnation are idempotent.
    pub(crate) fn request_locked_candidate(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
    ) -> Result<(), String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if !self.pending_candidate_loads.insert(tag) {
            operation.complete();
            return Ok(());
        }
        if let Err(error) = self
            .io()?
            .enqueue(V2IoCommand::LoadCandidate { tag, subject })
        {
            self.pending_candidate_loads.remove(&tag);
            return Err(error);
        }
        operation.complete();
        Ok(())
    }

    /// Take the next locked-subject body loaded by the ordered I/O worker.
    pub(crate) fn take_loaded_candidate(&mut self) -> Option<LoadedCandidateBody> {
        if self.output_guard.restart_required() {
            return None;
        }
        self.loaded_candidates.pop_front()
    }

    /// Take the next deterministic body rejection observed by the worker.
    pub(crate) fn take_validation_rejection(&mut self) -> Option<RejectedCandidateBody> {
        if self.output_guard.restart_required() {
            return None;
        }
        self.validation_rejections.pop_front()
    }

    /// Take the next exact validation deferral for bounded sidecar recovery.
    pub(crate) fn take_merge_sidecar_deferral(&mut self) -> Option<DeferredMergeSidecarWork> {
        if self.output_guard.restart_required() {
            return None;
        }
        self.merge_sidecar_deferrals.pop_front()
    }

    /// Put back a transiently capacity-blocked deferral without losing its
    /// exact durable validation intent.
    pub(crate) fn requeue_merge_sidecar_deferral(
        &mut self,
        deferred: DeferredMergeSidecarWork,
    ) -> Result<(), String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if let Some(existing) = self
            .merge_sidecar_deferrals
            .iter()
            .find(|existing| existing.work_id == deferred.work_id)
        {
            if existing.round == deferred.round
                && existing.subject == deferred.subject
                && existing.reference == deferred.reference
            {
                operation.complete();
                return Ok(());
            }
            // The conflicting claim was rejected before any state or output
            // changed. Let the caller classify the service error without
            // falsely turning this local validation into ambiguous output.
            operation.complete();
            return Err(
                "Sumeragi v2 work ID claimed conflicting merge-sidecar deferrals".to_owned(),
            );
        }
        if self.merge_sidecar_deferrals.len() >= self.max_merge_sidecar_deferrals {
            // Capacity backpressure leaves the retained FIFO unchanged and is
            // explicitly retryable by the runner.
            operation.complete();
            return Err("Sumeragi v2 merge-sidecar deferral queue is full".to_owned());
        }
        self.merge_sidecar_deferrals.push_back(deferred);
        operation.complete();
        Ok(())
    }

    /// Take the next reducer-authorized local Prepare intent.
    pub(crate) fn take_prepared_candidate(&mut self) -> Option<PreparedCandidateBody> {
        if self.output_guard.restart_required() {
            return None;
        }
        self.prepared_candidates.pop_front()
    }

    /// Route a possibly reordered payload chunk. Chunks received before their
    /// Proposal are retained under one explicit body-sized bound and undergo
    /// full signature/hash authentication only after the proposal manifest
    /// opens an exact fetch session.
    pub(crate) fn route_payload_chunk(
        &mut self,
        executor: &mut V2EffectExecutor,
        sender: PeerId,
        chunk: wire::PayloadChunk,
    ) -> Result<PayloadChunkDisposition, String> {
        let manifest_hash = chunk.manifest_hash;
        if let Some(work_id) = self.fetch_work_for_manifest(manifest_hash) {
            return self.deliver_payload_chunk(executor, work_id, sender, chunk);
        }

        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        Ok(self.buffer_orphan_payload_chunk(sender, chunk))
    }

    fn buffer_orphan_payload_chunk(
        &mut self,
        sender: PeerId,
        chunk: wire::PayloadChunk,
    ) -> PayloadChunkDisposition {
        let manifest_hash = chunk.manifest_hash;
        let sender_index = usize::try_from(chunk.sender).ok();
        let sender_matches = sender_index
            .and_then(|index| self.context.roster.get(index))
            .is_some_and(|entry| entry.validator == sender);
        let chunk_len = u64::try_from(chunk.bytes.len()).unwrap_or(u64::MAX);
        let max_chunk_count =
            usize::try_from(self.context.da_layout.max_chunk_count).unwrap_or(usize::MAX);
        let index_in_range = usize::try_from(chunk.index)
            .ok()
            .is_some_and(|index| index < max_chunk_count);
        if !sender_matches
            || !index_in_range
            || chunk.bytes.is_empty()
            || chunk_len > u64::from(self.context.da_layout.chunk_size_bytes)
        {
            return PayloadChunkDisposition::Rejected;
        }
        if let Some(buffered) = self.orphan_chunks.get(&manifest_hash) {
            if buffered.iter().any(|existing| {
                existing.sender == sender
                    && existing.chunk.index == chunk.index
                    && existing.chunk == chunk
            }) {
                return PayloadChunkDisposition::Duplicate;
            }
            // Retain at most one claim per authenticated outer sender/index. A
            // conflicting claim cannot be resolved without the manifest and is
            // discarded; a later retransmission of the canonical chunk succeeds.
            if buffered
                .iter()
                .any(|existing| existing.sender == sender && existing.chunk.index == chunk.index)
            {
                return PayloadChunkDisposition::Rejected;
            }
        }
        if self.orphan_chunk_count >= self.max_orphan_chunks
            || self.orphan_chunk_bytes.saturating_add(chunk_len) > self.max_orphan_chunk_bytes
        {
            return PayloadChunkDisposition::Rejected;
        }
        let buffered = self.orphan_chunks.entry(manifest_hash).or_default();
        buffered.push_back(BufferedPayloadChunk { sender, chunk });
        self.orphan_chunk_count = self.orphan_chunk_count.saturating_add(1);
        self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_add(chunk_len);
        PayloadChunkDisposition::Buffered
    }

    /// Replay all chunks whose proposal manifests have now opened sessions.
    pub(crate) fn replay_buffered_chunks(
        &mut self,
        executor: &mut V2EffectExecutor,
    ) -> Result<usize, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        let ready = self
            .orphan_chunks
            .keys()
            .filter_map(|hash| {
                self.fetch_work_for_manifest(*hash)
                    .map(|work_id| (*hash, work_id))
            })
            .collect::<Vec<_>>();
        let mut delivered = 0usize;
        for (manifest_hash, work_id) in ready {
            let Some(mut chunks) = self.orphan_chunks.remove(&manifest_hash) else {
                continue;
            };
            while let Some(buffered) = chunks.pop_front() {
                let bytes = u64::try_from(buffered.chunk.bytes.len()).unwrap_or(u64::MAX);
                self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
                self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(bytes);
                if self.fetch_work_for_manifest(manifest_hash) != Some(work_id) {
                    continue;
                }
                if self.deliver_payload_chunk(executor, work_id, buffered.sender, buffered.chunk)?
                    == PayloadChunkDisposition::Delivered
                {
                    delivered = delivered.saturating_add(1);
                }
            }
        }
        Ok(delivered)
    }

    fn take_io_completion(&self) -> Option<PendingServiceCompletion> {
        self.io
            .as_ref()
            .and_then(|io| io.try_recv_completion().ok())
            .map(PendingServiceCompletion::Io)
    }

    fn take_next_completion(&mut self) -> Option<PendingServiceCompletion> {
        let completion = match self.next_completion_source {
            CompletionSource::Io => self.take_io_completion().or_else(|| {
                self.local_completions
                    .pop_front()
                    .map(PendingServiceCompletion::Local)
            }),
            CompletionSource::Local => self
                .local_completions
                .pop_front()
                .map(PendingServiceCompletion::Local)
                .or_else(|| self.take_io_completion()),
        }?;
        self.next_completion_source = match &completion {
            PendingServiceCompletion::Io(_) => CompletionSource::Local,
            PendingServiceCompletion::Local(_) => CompletionSource::Io,
        };
        Some(completion)
    }

    /// Drain tagged I/O and reconstruction completions into the reducer owner.
    ///
    /// The service removes at most one runtime-producing completion per exact
    /// free FIFO slot and alternates between I/O and local reconstruction. A
    /// burst therefore remains in its bounded producer queue instead of
    /// overflowing the runtime's trusted completion reserve.
    pub(crate) fn drain_completions(
        &mut self,
        executor: &mut V2EffectExecutor,
    ) -> Result<usize, EffectExecutorError> {
        if self.output_guard.restart_required() {
            return Err(executor
                .external_service_failed("Sumeragi v2 consensus requires process restart", self));
        }
        let mut count = 0usize;
        while executor.remaining_completion_capacity() != 0 {
            let Some(completion) = self.take_next_completion() else {
                break;
            };
            count = count.saturating_add(1);
            match completion {
                PendingServiceCompletion::Io(V2IoCompletion::Signature {
                    work_id,
                    signature,
                    outbound_payload,
                }) => {
                    if let Some(payload) = outbound_payload
                        && let Err(reason) = self.register_outbound_payload(payload)
                    {
                        return Err(executor.external_service_failed(reason, self));
                    }
                    let _ = executor.complete_consensus_signature(work_id, signature, self)?;
                }
                PendingServiceCompletion::Io(V2IoCompletion::Stored(completion)) => {
                    let _ = executor.complete_body_store(completion, self)?;
                }
                PendingServiceCompletion::Io(V2IoCompletion::Validated(completion)) => {
                    let _ = executor.complete_body_validation(completion, self)?;
                }
                PendingServiceCompletion::Io(V2IoCompletion::Applied(completion)) => {
                    let _ = executor.complete_application(completion, self)?;
                }
                PendingServiceCompletion::Io(V2IoCompletion::ApplyDeferred {
                    work_id,
                    reference,
                }) => {
                    let _ =
                        executor.defer_application_for_merge_sidecar(work_id, &reference, self)?;
                }
                PendingServiceCompletion::Io(V2IoCompletion::CertifiedResponse {
                    recipient,
                    response,
                }) => self.post_to_peer(
                    recipient,
                    wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                    ),
                ),
                PendingServiceCompletion::Io(V2IoCompletion::CertifiedRequestIgnored) => {}
                PendingServiceCompletion::Io(V2IoCompletion::CandidateLoaded(candidate)) => {
                    self.pending_candidate_loads.remove(&candidate.tag());
                    self.loaded_candidates.push_back(candidate);
                }
                PendingServiceCompletion::Io(V2IoCompletion::Failed(reason)) => {
                    return Err(executor.external_service_failed(reason, self));
                }
                PendingServiceCompletion::Io(V2IoCompletion::Retired) => {
                    return Err(executor.external_service_failed(
                        "unexpected early Sumeragi v2 storage retirement",
                        self,
                    ));
                }
                PendingServiceCompletion::Io(V2IoCompletion::RetirementFailed(reason)) => {
                    return Err(executor.external_service_failed(
                        format!(
                            "unexpected early Sumeragi v2 storage retirement failure: {reason}"
                        ),
                        self,
                    ));
                }
                PendingServiceCompletion::Io(V2IoCompletion::RecoveryRequired(reason)) => {
                    return Err(executor.external_service_failed(
                        format!("canonical persistence requires restart recovery: {reason}"),
                        self,
                    ));
                }
                PendingServiceCompletion::Local(LocalCompletion::Reconstructed {
                    task,
                    manifest,
                    body,
                }) => {
                    if let Err(error) =
                        executor.complete_body_reconstruction(&task, manifest, body, self)
                    {
                        return Err(executor.external_service_failed(error, self));
                    }
                }
            }
        }
        Ok(count)
    }

    /// Retire all height-local body and chunk files after finalized rollover.
    ///
    /// The caller invokes this only after the adapter verified Kura's typed
    /// receipt. Cleanup is therefore irreversible local maintenance: every
    /// failure is retained in the returned outcome and later cleanup stages
    /// still run, but none can invalidate the committed block.
    pub(crate) fn finish_height(
        mut self,
        receipt: KuraV2CommitReceipt,
        cleanup_timeout: Duration,
        supervisor: &mut V2CleanupSupervisor,
    ) -> PostFinalityCleanupOutcome {
        self.clean_teardown = true;
        let mut outcome = PostFinalityCleanupOutcome::default();
        let identity = CleanupWorkerIdentity::from_receipt(&receipt);
        let deadline = Instant::now()
            .checked_add(cleanup_timeout)
            .unwrap_or_else(Instant::now);
        if let Some(mut io) = self.io.take() {
            let mut command = V2IoCommand::Retire(receipt);
            let retirement_guard = Arc::clone(&self.output_guard);
            let retirement_requested = 'enqueue: loop {
                let Some(retirement_enqueue_permit) = retirement_guard.acquire() else {
                    outcome.record(
                        PostFinalityCleanupTarget::CleanupWorker,
                        "process restart became required before body retirement enqueue",
                    );
                    break false;
                };
                let enqueue = io.try_enqueue(command);
                // Waiting for an older completion while holding this permit
                // would prevent fatal activation from draining output.
                drop(retirement_enqueue_permit);
                match enqueue {
                    Ok(()) => break true,
                    Err(V2IoTrySendError::Full(returned)) => {
                        command = returned;
                        match recv_cleanup_completion(&io, deadline) {
                            Ok(V2IoCompletion::Failed(reason)) => outcome.record(
                                PostFinalityCleanupTarget::CleanupWorker,
                                format!(
                                    "pending I/O work failed while enqueueing body retirement: {reason}"
                                ),
                            ),
                            Ok(V2IoCompletion::Retired) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    "I/O worker reported retirement before accepting the retirement request",
                                );
                                break 'enqueue false;
                            }
                            Ok(V2IoCompletion::RetirementFailed(reason)) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    "Sumeragi v2 I/O worker reported body retirement failure",
                                );
                                outcome.record(PostFinalityCleanupTarget::DurableBodies, reason);
                                break 'enqueue false;
                            }
                            Ok(_) => {}
                            Err(CleanupCompletionWaitError::DeadlineElapsed) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    format!(
                                        "Sumeragi v2 body retirement enqueue exceeded the configured {cleanup_timeout:?} post-finality cleanup deadline"
                                    ),
                                );
                                // Typed finality is already durable, but the
                                // full command queue prevented Retire from
                                // being enqueued before the cleanup deadline.
                                // Authorize only the ensuing normal producer
                                // disconnect, before dropping the last sender.
                                io.allow_finalized_disconnect
                                    .store(true, AtomicOrdering::Release);
                                break 'enqueue false;
                            }
                            Err(CleanupCompletionWaitError::Disconnected) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    "Sumeragi v2 I/O worker disconnected before body retirement",
                                );
                                break 'enqueue false;
                            }
                        }
                    }
                    Err(V2IoTrySendError::Disconnected) => {
                        outcome.record(
                            PostFinalityCleanupTarget::CleanupWorker,
                            "Sumeragi v2 I/O worker disconnected before body retirement",
                        );
                        break false;
                    }
                    Err(V2IoTrySendError::ConflictingWorkId { .. }) => {
                        unreachable!("retirement commands do not carry work identifiers")
                    }
                }
            };
            if retirement_requested {
                loop {
                    match recv_cleanup_completion(&io, deadline) {
                        Ok(V2IoCompletion::Retired) => break,
                        Ok(V2IoCompletion::RetirementFailed(reason)) => {
                            outcome.record(
                                PostFinalityCleanupTarget::CleanupWorker,
                                "Sumeragi v2 I/O worker reported body retirement failure",
                            );
                            outcome.record(PostFinalityCleanupTarget::DurableBodies, reason);
                            break;
                        }
                        Ok(V2IoCompletion::Failed(reason)) => outcome.record(
                            PostFinalityCleanupTarget::CleanupWorker,
                            format!(
                                "pending I/O work failed before body retirement completed: {reason}"
                            ),
                        ),
                        Ok(_) => continue,
                        Err(CleanupCompletionWaitError::DeadlineElapsed) => {
                            outcome.record(
                                PostFinalityCleanupTarget::CleanupWorker,
                                format!(
                                    "Sumeragi v2 body retirement exceeded the configured {cleanup_timeout:?} post-finality cleanup deadline"
                                ),
                            );
                            break;
                        }
                        Err(CleanupCompletionWaitError::Disconnected) => {
                            outcome.record(
                                PostFinalityCleanupTarget::CleanupWorker,
                                "Sumeragi v2 I/O worker disconnected without confirming body retirement",
                            );
                            break;
                        }
                    }
                }
            }
            let join = io.join.take();
            // Closing both channels makes a worker which accepted Retire but
            // withheld its completion leave its command loop. A worker still
            // inside context-local filesystem retirement remains owned by the
            // runner supervisor and cannot block successor construction.
            drop(io);
            if let Some(join) = join {
                if join.is_finished() {
                    if join.join().is_err() {
                        outcome.record(
                            PostFinalityCleanupTarget::CleanupWorker,
                            "Sumeragi v2 I/O worker panicked during finalized cleanup",
                        );
                    }
                } else {
                    supervisor.supervise(identity, join);
                }
            }
        } else {
            outcome.record(
                PostFinalityCleanupTarget::CleanupWorker,
                "Sumeragi v2 I/O worker was unavailable for body retirement",
            );
        }

        let output_guard = Arc::clone(&self.output_guard);
        let Some(chunk_cleanup_permit) = output_guard.acquire() else {
            outcome.record(
                PostFinalityCleanupTarget::PayloadChunks,
                "process restart became required before chunk cleanup",
            );
            return outcome;
        };
        match std::fs::remove_dir_all(&self.chunk_root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => outcome.record(
                PostFinalityCleanupTarget::PayloadChunks,
                format!(
                    "failed to remove Sumeragi v2 chunk root {}: {error}",
                    self.chunk_root.display()
                ),
            ),
        }
        drop(chunk_cleanup_permit);
        outcome
    }

    fn io(&self) -> Result<&V2IoHandle, String> {
        self.io
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())
    }

    fn output_permit(&self) -> Result<ConsensusOutputPermit<'_>, String> {
        self.output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 canonical persistence requires restart recovery".to_owned())
    }

    fn enqueue_io(&self, command: V2IoCommand) -> Result<(), String> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.io()?.enqueue(command)
    }

    fn enqueue_fail_stop_io(&self, command: V2IoCommand) -> Result<(), String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.io()?.enqueue(command)?;
        operation.complete();
        Ok(())
    }

    /// Mark an operator-requested shutdown as non-fatal before dropping services.
    pub(crate) fn allow_clean_shutdown(&mut self) {
        self.clean_teardown = true;
    }

    fn deliver_payload_chunk(
        &mut self,
        executor: &mut V2EffectExecutor,
        work_id: EffectWorkId,
        sender: PeerId,
        chunk: wire::PayloadChunk,
    ) -> Result<PayloadChunkDisposition, String> {
        match executor.accept_payload_chunk(work_id, chunk, &sender, self) {
            Ok(()) => Ok(PayloadChunkDisposition::Delivered),
            Err(EffectTransportError::FailClosed(reason)) => Err(reason),
            Err(error) => {
                iroha_logger::debug!(%sender, %error, "rejected Sumeragi v2 payload chunk");
                Ok(PayloadChunkDisposition::Rejected)
            }
        }
    }

    /// Send one already-versioned v2 transport envelope to a specific peer.
    pub(crate) fn post_to_peer(&self, peer: PeerId, message: wire::ConsensusMessageV2) {
        let Ok(permit) = self.output_permit() else {
            return;
        };
        let _ = self.post_block_message_while_guarded(peer, BlockMessage::V2(message), &permit);
    }

    /// Send one v2 envelope under a caller-owned output operation.
    pub(crate) fn post_to_peer_with_permit(
        &self,
        peer: PeerId,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        self.post_block_message_while_guarded(peer, BlockMessage::V2(message), permit)
    }

    /// Send one retained lane-local proposal, vote, or QC to a committee peer.
    pub(crate) fn post_lane_block(
        &self,
        peer: PeerId,
        message: BlockMessage,
    ) -> Result<(), String> {
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if !matches!(
            message,
            BlockMessage::LaneBlockProposal(_)
                | BlockMessage::LaneBlockVote(_)
                | BlockMessage::LaneBlockQc(_)
        ) {
            return Err("v2 lane transport rejected a legacy global block message".to_owned());
        }
        self.post_block_message_while_guarded(peer, message, operation.permit())?;
        operation.complete();
        Ok(())
    }

    /// Send one bounded certified merge-sidecar request or response through
    /// the dedicated authenticated network envelope.
    pub(crate) fn post_certified_merge_sidecar(
        &self,
        peer: PeerId,
        message: CertifiedMergeSidecarMessage,
    ) {
        let Ok(_permit) = self.output_permit() else {
            return;
        };
        self.network.post(Post {
            data: NetworkMessage::CertifiedMergeSidecar(Box::new(message)),
            peer_id: peer,
            priority: Priority::High,
        });
    }

    /// Send one context-bound Native AMX v2 message to a participant peer.
    pub(crate) fn post_native_amx(
        &self,
        peer: PeerId,
        message: crate::native_amx::NativeAmxMessage,
    ) {
        let Ok(_permit) = self.output_permit() else {
            return;
        };
        self.network.post(Post {
            data: NetworkMessage::NativeAmx(Box::new(message)),
            peer_id: peer,
            priority: Priority::High,
        });
    }

    /// Broadcast one merge signature share to every other frozen voter.
    pub(crate) fn broadcast_merge_to_voters(&self, signature: MergeCommitteeSignature) {
        let Ok(_permit) = self.output_permit() else {
            return;
        };
        for entry in &self.context.roster {
            if entry.validator == self.local_peer {
                continue;
            }
            self.network.post(Post {
                data: NetworkMessage::MergeCommitteeSignature(Box::new(signature.clone())),
                peer_id: entry.validator.clone(),
                priority: Priority::High,
            });
        }
    }

    fn post_block_message_while_guarded(
        &self,
        peer: PeerId,
        message: BlockMessage,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        let block_message = Arc::new(message);
        let wire = BlockMessageWire::try_preencoded(block_message).map_err(|error| {
            format!("failed to encode guarded Sumeragi v2 message for {peer}: {error}")
        })?;
        let data = NetworkMessage::SumeragiBlock(Box::new(wire));
        self.network.post(Post {
            data,
            peer_id: peer,
            priority: Priority::High,
        });
        Ok(())
    }

    fn preencode_v2_network_message(
        message: wire::ConsensusMessageV2,
    ) -> Result<NetworkMessage, String> {
        let wire = BlockMessageWire::try_preencoded(Arc::new(BlockMessage::V2(message)))
            .map_err(|error| format!("failed to encode guarded Sumeragi v2 message: {error}"))?;
        Ok(NetworkMessage::SumeragiBlock(Box::new(wire)))
    }

    fn broadcast_preencoded_to_voters_while_guarded(
        &self,
        data: &NetworkMessage,
        _permit: &ConsensusOutputPermit<'_>,
    ) {
        for entry in &self.context.roster {
            if entry.validator == self.local_peer {
                continue;
            }
            self.network.post(Post {
                data: data.clone(),
                peer_id: entry.validator.clone(),
                priority: Priority::High,
            });
        }
    }

    /// Broadcast under a caller-owned output permit without reacquiring it.
    pub(crate) fn broadcast_to_voters_while_guarded(
        &self,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        let data = Self::preencode_v2_network_message(message)?;
        self.broadcast_preencoded_to_voters_while_guarded(&data, permit);
        Ok(())
    }
}

impl Drop for ProductionV2Services {
    fn drop(&mut self) {
        let restart_required = !self.clean_teardown;
        if restart_required {
            self.output_guard.close_admission_for_restart();
        }
        if let Some(io) = self.io.take()
            && let Err(error) = io.shutdown()
        {
            iroha_logger::error!(%error, "failed to stop Sumeragi v2 I/O worker");
        }
        if restart_required && !thread::panicking() {
            self.output_guard.activate_restart_required();
        }
    }
}

impl V2EffectServices for ProductionV2Services {
    type Error = String;

    fn enqueue_consensus_sign(&mut self, task: ConsensusSignTask) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let restore_outbound_payload = match task.request() {
            super::v2::SignRequest::Proposal(proposal) => !self
                .outbound_chunks
                .contains_key(&HashOf::new(&proposal.manifest)),
            super::v2::SignRequest::Vote(_) | super::v2::SignRequest::TimeoutVote(_) => false,
        };
        let prepared = match task.request() {
            super::v2::SignRequest::Vote(vote) if vote.phase == wire::GlobalPhase::Prepare => {
                Some(PreparedCandidateBody {
                    tag: task.tag(),
                    subject: vote.subject,
                })
            }
            super::v2::SignRequest::Proposal(_)
            | super::v2::SignRequest::Vote(_)
            | super::v2::SignRequest::TimeoutVote(_) => None,
        };
        self.io()?.enqueue(V2IoCommand::Sign {
            task,
            restore_outbound_payload,
        })?;
        if let Some(prepared) = prepared
            && self.prepared_candidates.len() < self.max_orphan_chunks
        {
            self.prepared_candidates.push_back(prepared);
        }
        operation.complete();
        Ok(())
    }

    fn cancel_consensus_sign(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
        self.io()?.cancel(work_id, V2IoCancellableKind::Sign)?;
        Ok(())
    }

    fn broadcast_consensus(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        let mut messages = vec![message.clone()];
        if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload {
            let manifest_hash = HashOf::new(&proposal.manifest);
            let chunks = self
                .outbound_chunks
                .get(&manifest_hash)
                .ok_or_else(|| "local proposal has no retained Sumeragi v2 chunks".to_owned())?;
            messages.extend(chunks.iter().cloned());
        }
        let encoded = messages
            .into_iter()
            .map(Self::preencode_v2_network_message)
            .collect::<Result<Vec<_>, _>>()?;
        for data in &encoded {
            self.broadcast_preencoded_to_voters_while_guarded(data, operation.permit());
        }
        operation.complete();
        Ok(())
    }

    fn sign_body_request(&mut self, preimage: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let signature = Signature::try_new(self.key_pair.private_key(), preimage)
            .map(|signature| signature.payload().to_vec())
            .map_err(|error| error.to_string())?;
        operation.complete();
        Ok(signature)
    }

    fn enqueue_body_fetch(&mut self, task: BodyFetchTask) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard.begin_fail_stop_operation().ok_or_else(|| {
            "Sumeragi v2 canonical persistence requires restart recovery".to_owned()
        })?;
        match self.body_fetch_service_owner(task.id())? {
            BodyFetchServiceOwner::Reconstructed(index) => {
                let LocalCompletion::Reconstructed {
                    task: queued_task, ..
                } = self
                    .local_completions
                    .get(index)
                    .expect("queued reconstruction owner was classified above");
                if task != *queued_task && !task.monotonically_extends(queued_task) {
                    return Err(format!(
                        "conflicting Sumeragi v2 body-fetch retransmission for completed work {}",
                        task.id().get()
                    ));
                }
                let LocalCompletion::Reconstructed {
                    task: queued_task, ..
                } = self
                    .local_completions
                    .get_mut(index)
                    .expect("queued reconstruction owner was classified above");
                *queued_task = task;
                operation.complete();
                return Ok(());
            }
            BodyFetchServiceOwner::Live => {
                let existing = self
                    .fetches
                    .get(&task.id())
                    .expect("live body-fetch owner was classified above");
                if task != existing.task && !task.monotonically_extends(&existing.task) {
                    return Err("conflicting Sumeragi v2 body-fetch task".to_owned());
                }
                let manifest_upgrade =
                    existing.task.manifest().is_none() && task.manifest().is_some();
                let opened_chunks = manifest_upgrade
                    .then(|| {
                        V2ChunkSession::open(
                            &self.chunk_root,
                            &self.context,
                            task.manifest()
                                .expect("manifest upgrade was checked above")
                                .clone(),
                        )
                    })
                    .transpose()
                    .map_err(|error| error.to_string())?;

                if let Some(request) = task.certified_request() {
                    let message = wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
                    );
                    for peer in task.sources() {
                        if peer != &self.local_peer {
                            self.post_block_message_while_guarded(
                                peer.clone(),
                                BlockMessage::V2(message.clone()),
                                operation.permit(),
                            )?;
                        }
                    }
                }
                if let Some(chunks) = opened_chunks {
                    let manifest_hash =
                        HashOf::new(task.manifest().expect("opened chunks require a manifest"));
                    match self.fetch_by_manifest.entry(manifest_hash) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(task.id());
                        }
                        std::collections::btree_map::Entry::Occupied(_) => {
                            return Err("duplicate Sumeragi v2 fetch manifest".to_owned());
                        }
                    }
                    self.fetches
                        .get_mut(&task.id())
                        .expect("live body-fetch owner was classified above")
                        .chunks = Some(chunks);
                }
                let work_id = task.id();
                self.fetches
                    .get_mut(&work_id)
                    .expect("live body-fetch owner was classified above")
                    .task = task;
                operation.complete();
                return Ok(());
            }
            BodyFetchServiceOwner::None => {}
        }

        if task.manifest().is_none() && task.certified_request().is_none() {
            return Err("Sumeragi v2 body-fetch task has no acquisition authority".to_owned());
        }
        let chunks = task
            .manifest()
            .cloned()
            .map(|manifest| V2ChunkSession::open(&self.chunk_root, &self.context, manifest))
            .transpose()
            .map_err(|error| error.to_string())?;
        if let Some(manifest) = task.manifest() {
            let hash = HashOf::new(manifest);
            match self.fetch_by_manifest.entry(hash) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(task.id());
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err("duplicate Sumeragi v2 fetch manifest".to_owned());
                }
            }
        }
        if let Some(request) = task.certified_request() {
            let message = wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
            );
            for peer in task.sources() {
                if peer != &self.local_peer {
                    self.post_block_message_while_guarded(
                        peer.clone(),
                        BlockMessage::V2(message.clone()),
                        operation.permit(),
                    )?;
                }
            }
        }
        self.fetches
            .insert(task.id(), FetchSession { task, chunks });
        operation.complete();
        Ok(())
    }

    fn cancel_body_fetch(&mut self, task: &BodyFetchTask) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.remove_exact_body_fetch_owner(task)?;
        operation.complete();
        Ok(())
    }

    fn complete_certified_body_fetch(&mut self, task: &BodyFetchTask) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if task.certified_request().is_none() {
            return Err(format!(
                "Sumeragi v2 body-fetch work {} completed without certified authority",
                task.id().get()
            ));
        }
        self.remove_exact_body_fetch_owner(task)?;
        operation.complete();
        Ok(())
    }

    fn accept_authenticated_chunk(
        &mut self,
        task: &BodyFetchTask,
        chunk: AuthenticatedPayloadChunk,
    ) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if self.body_fetch_service_owner(task.id())? != BodyFetchServiceOwner::Live {
            return Err("Sumeragi v2 chunk fetch has no exact live owner".to_owned());
        }
        let fetch = self
            .fetches
            .get_mut(&task.id())
            .expect("live body-fetch owner was classified above");
        if fetch.task != *task {
            return Err(format!(
                "Sumeragi v2 chunk task {} differs from service ownership",
                task.id().get()
            ));
        }
        let session = fetch
            .chunks
            .as_mut()
            .ok_or_else(|| "manifest-less certified body fetch cannot accept chunks".to_owned())?;
        session
            .admit(chunk.chunk())
            .map_err(|error| error.to_string())?;
        let Some(body) = session.reconstruct().map_err(|error| error.to_string())? else {
            operation.complete();
            return Ok(());
        };
        let manifest = session.manifest().clone();
        if self.body_fetch_service_owner(task.id())? != BodyFetchServiceOwner::Live {
            return Err("Sumeragi v2 reconstructed fetch lost its exact live owner".to_owned());
        }
        let removed = self.fetch_by_manifest.remove(&HashOf::new(&manifest));
        if removed != Some(task.id()) {
            return Err(format!(
                "Sumeragi v2 reconstructed work {} lost its manifest index",
                task.id().get()
            ));
        }
        let fetch = self
            .fetches
            .remove(&task.id())
            .expect("live body-fetch owner was classified above");
        if fetch.task != *task {
            return Err(format!(
                "Sumeragi v2 reconstructed work {} changed task ownership",
                task.id().get()
            ));
        }
        self.local_completions
            .push_back(LocalCompletion::Reconstructed {
                task: fetch.task,
                manifest,
                body,
            });
        operation.complete();
        Ok(())
    }

    fn enqueue_body_store(&mut self, task: BodyStoreTask) -> Result<(), Self::Error> {
        self.enqueue_fail_stop_io(V2IoCommand::Store(task))
    }

    fn cancel_body_store(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
        self.io()?.cancel(work_id, V2IoCancellableKind::Store)?;
        Ok(())
    }

    fn enqueue_body_validation(&mut self, task: BodyValidationTask) -> Result<(), Self::Error> {
        self.enqueue_fail_stop_io(V2IoCommand::Validate(task))
    }

    fn cancel_body_validation(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
        self.io()?.cancel(work_id, V2IoCancellableKind::Validate)?;
        Ok(())
    }

    fn work_deferred_for_merge_sidecar(
        &mut self,
        work_id: EffectWorkId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reference: &CertifiedMergeLedgerReference,
    ) -> Result<(), Self::Error> {
        self.requeue_merge_sidecar_deferral(DeferredMergeSidecarWork {
            work_id,
            round,
            subject,
            reference: reference.clone(),
        })
    }

    fn enqueue_apply(&mut self, task: ApplyTask) -> Result<(), Self::Error> {
        self.enqueue_fail_stop_io(V2IoCommand::Apply(task))
    }

    fn entered_view(
        &mut self,
        tag: EventTag,
        _certificate: wire::TimeoutCertificate,
    ) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.entered_view = Some(tag);
        Ok(())
    }

    fn report_equivocation(
        &mut self,
        offender: PeerId,
        round: wire::ConsensusRound,
        kind: EquivocationKind,
    ) -> Result<(), Self::Error> {
        let _permit = self.output_permit()?;
        iroha_logger::warn!(%offender, ?round, ?kind, "authenticated Sumeragi v2 equivocation");
        Ok(())
    }

    fn report_invalid_certified_body(
        &mut self,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
    ) -> Result<(), Self::Error> {
        let _permit = self.output_permit()?;
        iroha_logger::error!(
            ?subject,
            ?certificate,
            "invalid body certified by Sumeragi v2 PrepareQC"
        );
        Ok(())
    }

    fn validation_rejected(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reason: &str,
    ) {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(_permit) = output_guard.acquire() else {
            return;
        };
        if self.validation_rejections.len() < self.max_orphan_chunks {
            self.validation_rejections.push_back(RejectedCandidateBody {
                round,
                subject,
                reason: reason.to_owned(),
            });
        }
        iroha_logger::warn!(
            ?round,
            ?subject,
            reason,
            "Sumeragi v2 proposal validation rejected"
        );
    }

    fn publish_effect_status(&mut self, status: &EffectExecutorStatus) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.last_status = Some(status.clone());
        Ok(())
    }

    fn fail_closed(&mut self, reason: &str) {
        self.output_guard.activate_restart_required();
        self.fatal_reason = Some(reason.to_owned());
        iroha_logger::error!(reason, "Sumeragi v2 effect services failed closed");
    }
}

#[cfg(test)]
mod tests {
    use std::{
        num::NonZeroU64,
        sync::atomic::{AtomicBool, Ordering},
    };

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId,
        block::{BlockHeader, BlockSignature, CertifiedMergeLedgerReference, SignedBlock},
        merge::{MergeLedgerEntry, MergeQuorumCertificate},
    };
    use tempfile::TempDir;

    use super::*;
    use crate::sumeragi::{v2_body_store::DurableBodyReceipt, v2_chunks::encode_payload};

    fn test_io_command_channel(
        capacity: usize,
    ) -> (V2IoCommandSender, V2IoCommandReceiver, Arc<V2IoAdmission>) {
        let admission = V2IoAdmission::unbounded_for_tests();
        let (sender, receiver) = v2_io_command_channel(capacity, Arc::clone(&admission));
        (sender, receiver, admission)
    }

    fn fixture() -> (ProductionV2Services, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic validator key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            chain_id: ChainId::from("v2-worker-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("dual quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"v2-worker-test-context"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 8,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 32,
                max_chunk_count: 4,
            },
            leader_seed: [0x33; 32],
        };
        context.validate().expect("valid context");
        let local_peer = context.roster[0].validator.clone();
        let service = ProductionV2Services {
            context,
            local_peer,
            local_validator: Some(0),
            key_pair: keys[0].clone(),
            network: crate::IrohaNetwork::closed_for_tests(),
            chunk_root: PathBuf::new(),
            io: None,
            fetches: BTreeMap::new(),
            fetch_by_manifest: BTreeMap::new(),
            orphan_chunks: BTreeMap::new(),
            orphan_chunk_count: 0,
            orphan_chunk_bytes: 0,
            max_orphan_chunks: 1,
            max_orphan_chunk_bytes: 32,
            max_merge_sidecar_deferrals: 1,
            local_completions: VecDeque::new(),
            next_completion_source: CompletionSource::Io,
            pending_candidate_loads: BTreeSet::new(),
            loaded_candidates: VecDeque::new(),
            prepared_candidates: VecDeque::new(),
            validation_rejections: VecDeque::new(),
            merge_sidecar_deferrals: VecDeque::new(),
            outbound_chunks: BTreeMap::new(),
            entered_view: None,
            last_status: None,
            fatal_reason: None,
            output_guard: ConsensusOutputGuard::isolated(),
            clean_teardown: true,
        };
        (service, keys)
    }

    fn proposal_body_and_payload(
        context: &wire::HeightContext,
        keys: &[KeyPair],
    ) -> (Vec<u8>, EncodedV2Payload, wire::Proposal) {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let proposer = context.leader(round.view);
        let proposer_index = usize::try_from(proposer).expect("fixture proposer index");
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero fixture height"),
            None,
            None,
            None,
            1_000,
            round.view,
        );
        let signature =
            SignatureOf::try_from_hash(keys[proposer_index].private_key(), header.hash())
                .expect("sign fixture block header");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(proposer), signature),
            header,
            Vec::new(),
        );
        let canonical_wire = block.encode_wire().expect("canonical fixture block");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let payload = encode_payload(context, round, subject, &canonical_wire)
            .expect("encode fixture proposal payload");
        let proposal = wire::Proposal {
            round,
            proposer,
            subject,
            manifest: payload.manifest().clone(),
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        (canonical_wire, payload, proposal)
    }

    fn allow_fixture_block_payload(context: &mut wire::HeightContext) {
        context.da_layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1_024,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 16_384,
            max_chunk_count: 16,
        };
        context.validate().expect("widened fixture context");
    }

    fn install_temporary_chunk_root(service: &mut ProductionV2Services) -> TempDir {
        let directory = TempDir::new().expect("temporary chunk root");
        service.chunk_root = directory.path().to_path_buf();
        directory
    }

    fn certified_fetch_task(
        service: &ProductionV2Services,
        id: u64,
        tag: EventTag,
        manifest: Option<wire::PayloadManifest>,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> BodyFetchTask {
        let certificate = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"fetch fixture parent state"),
                Hash::new(b"fetch fixture post state"),
                Hash::new(b"fetch fixture writes"),
                Hash::new(b"fetch fixture block"),
            ),
            signers: vec![0],
            aggregate_signature: vec![1],
        };
        let request = wire::CertifiedBodyRequest {
            round,
            subject,
            certificate,
            requester: service.local_peer.clone(),
            signature: vec![1],
        };
        BodyFetchTask::certified_for_test(
            id,
            tag,
            manifest,
            vec![service.local_peer.clone()],
            request,
        )
    }

    #[test]
    fn replayed_proposal_signature_restores_exact_durable_payload() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let directory = TempDir::new().expect("temporary body store");
        let mut body_store =
            V2BodyStore::open(directory.path(), service.context.clone()).expect("open body store");
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let _receipt = body_store
            .store(payload.manifest().clone(), canonical_wire)
            .expect("store exact proposal body");
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let proposer = usize::try_from(proposal.proposer).expect("fixture proposer index");
        let task =
            ConsensusSignTask::for_test(7, tag, super::super::v2::SignRequest::Proposal(proposal));
        let expected_work_id = task.id();
        let completion =
            sign_consensus_task(&body_store, &service.context, &keys[proposer], task, true)
                .expect("sign replayed proposal");

        let V2IoCompletion::Signature {
            work_id,
            signature,
            outbound_payload: Some(restored),
        } = completion
        else {
            panic!("proposal replay must restore its outbound payload");
        };
        assert_eq!(work_id, expected_work_id);
        assert!(!signature.is_empty());
        assert_eq!(restored, payload);
    }

    #[test]
    fn replayed_proposal_signature_rejects_missing_durable_payload() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let directory = TempDir::new().expect("temporary body store");
        let body_store = V2BodyStore::open(directory.path(), service.context.clone())
            .expect("open empty body store");
        let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let proposer = usize::try_from(proposal.proposer).expect("fixture proposer index");
        let error = match sign_consensus_task(
            &body_store,
            &service.context,
            &keys[proposer],
            ConsensusSignTask::for_test(8, tag, super::super::v2::SignRequest::Proposal(proposal)),
            true,
        ) {
            Ok(_) => panic!("missing durable proposal body must fail closed"),
            Err(error) => error,
        };
        assert!(error.contains("no durable exact body"));
    }

    #[test]
    fn proposal_signing_restores_chunks_only_when_outbound_payload_is_absent() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (command_tx, command_rx, admission) = test_io_command_channel(2);
        let (_completion_tx, completion_rx) = mpsc::sync_channel(2);
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );

        service
            .enqueue_consensus_sign(ConsensusSignTask::for_test(
                9,
                tag,
                super::super::v2::SignRequest::Proposal(proposal.clone()),
            ))
            .expect("queue replayed proposal signature");
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Sign {
                restore_outbound_payload: true,
                ..
            })
        ));

        service
            .register_outbound_payload(payload)
            .expect("register live proposal payload");
        service
            .enqueue_consensus_sign(ConsensusSignTask::for_test(
                10,
                tag,
                super::super::v2::SignRequest::Proposal(proposal),
            ))
            .expect("queue live proposal signature");
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Sign {
                restore_outbound_payload: false,
                ..
            })
        ));

        // No worker owns this synthetic channel; remove it before service Drop
        // attempts the production shutdown handshake.
        drop(service.io.take());
    }

    #[test]
    fn completion_sources_alternate_under_simultaneous_bursts() {
        let (mut service, _) = fixture();
        let (command_tx, _command_rx, admission) = test_io_command_channel(2);
        let (completion_tx, completion_rx) = mpsc::sync_channel(2);
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });
        completion_tx
            .try_send(V2IoCompletion::CertifiedRequestIgnored)
            .expect("first I/O completion");
        completion_tx
            .try_send(V2IoCompletion::CertifiedRequestIgnored)
            .expect("second I/O completion");

        let payload = b"completion fairness body";
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"completion fairness block",
            )),
            payload_hash: Hash::new(payload),
        };
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 0,
        };
        let manifest = encode_payload(&service.context, round, subject, payload)
            .expect("encode completion fairness body")
            .manifest()
            .clone();
        let completion_tag = EventTag::new(
            service.context.height,
            round.view,
            Generation::new(service.context.height),
        );
        for id in 1..=2 {
            service
                .local_completions
                .push_back(LocalCompletion::Reconstructed {
                    task: BodyFetchTask::ordinary_for_test(id, completion_tag, manifest.clone()),
                    manifest: manifest.clone(),
                    body: payload.to_vec(),
                });
        }

        assert!(matches!(
            service.take_next_completion(),
            Some(PendingServiceCompletion::Io(_))
        ));
        assert!(matches!(
            service.take_next_completion(),
            Some(PendingServiceCompletion::Local(_))
        ));
        assert!(matches!(
            service.take_next_completion(),
            Some(PendingServiceCompletion::Io(_))
        ));
        assert!(matches!(
            service.take_next_completion(),
            Some(PendingServiceCompletion::Local(_))
        ));
        assert!(service.take_next_completion().is_none());

        drop(service.io.take());
    }

    #[test]
    fn cancelling_fetch_consumes_queued_reconstruction_owner() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(41, tag, payload.manifest().clone());
        service
            .local_completions
            .push_back(LocalCompletion::Reconstructed {
                task: task.clone(),
                manifest: payload.manifest().clone(),
                body: canonical_wire,
            });

        service
            .cancel_body_fetch(&task)
            .expect("queued reconstruction owns the cancelled fetch");

        assert!(service.fetches.is_empty());
        assert!(service.fetch_by_manifest.is_empty());
        assert!(service.local_completions.is_empty());
        assert!(!service.output_guard.restart_required());
    }

    #[test]
    fn retransmitting_fetch_with_queued_reconstruction_is_idempotent() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(45, tag, payload.manifest().clone());
        service
            .local_completions
            .push_back(LocalCompletion::Reconstructed {
                task: task.clone(),
                manifest: payload.manifest().clone(),
                body: canonical_wire,
            });

        service
            .enqueue_body_fetch(task)
            .expect("queued reconstruction makes retransmission idempotent");

        assert!(service.fetches.is_empty());
        assert!(service.fetch_by_manifest.is_empty());
        assert_eq!(service.local_completions.len(), 1);
        assert!(!service.output_guard.restart_required());
    }

    #[test]
    fn retransmitting_fetch_with_conflicting_queued_manifest_fails_closed() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(46, tag, payload.manifest().clone());
        let mut conflicting_manifest = payload.manifest().clone();
        conflicting_manifest.payload_size_bytes = conflicting_manifest
            .payload_size_bytes
            .checked_add(1)
            .expect("small fixture payload size");
        service
            .local_completions
            .push_back(LocalCompletion::Reconstructed {
                task: task.clone(),
                manifest: conflicting_manifest,
                body: canonical_wire,
            });

        let error = service
            .enqueue_body_fetch(task)
            .expect_err("conflicting queued result must fail closed");

        assert!(error.contains("inconsistent manifest ownership"));
        assert!(service.fetches.is_empty());
        assert!(service.fetch_by_manifest.is_empty());
        assert_eq!(service.local_completions.len(), 1);
        assert!(service.output_guard.restart_required());
    }

    #[test]
    fn cancelling_fetch_consumes_live_session_and_manifest_owner() {
        let (mut service, keys) = fixture();
        let _chunk_root = install_temporary_chunk_root(&mut service);
        allow_fixture_block_payload(&mut service.context);
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(42, tag, payload.manifest().clone());
        service
            .enqueue_body_fetch(task.clone())
            .expect("open exact live reconstruction session");

        service
            .cancel_body_fetch(&task)
            .expect("live reconstruction session owns the cancelled fetch");

        assert!(service.fetches.is_empty());
        assert!(service.fetch_by_manifest.is_empty());
        assert!(service.local_completions.is_empty());
        assert!(!service.output_guard.restart_required());
    }

    #[test]
    fn cancelling_unowned_fetch_fails_closed() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(43, tag, payload.manifest().clone());

        let error = service
            .cancel_body_fetch(&task)
            .expect_err("missing service ownership must fail closed");

        assert!(error.contains("has no service owner"));
        assert!(service.output_guard.restart_required());
    }

    #[test]
    fn cancelling_fetch_with_overlapping_owners_fails_closed() {
        let (mut service, keys) = fixture();
        let _chunk_root = install_temporary_chunk_root(&mut service);
        allow_fixture_block_payload(&mut service.context);
        let (canonical_wire, payload, proposal) =
            proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(44, tag, payload.manifest().clone());
        let work_id = task.id();
        let manifest_hash = HashOf::new(payload.manifest());
        service
            .enqueue_body_fetch(task.clone())
            .expect("open exact live reconstruction session");
        service
            .local_completions
            .push_back(LocalCompletion::Reconstructed {
                task: task.clone(),
                manifest: payload.manifest().clone(),
                body: canonical_wire,
            });

        let error = service
            .cancel_body_fetch(&task)
            .expect_err("overlapping service ownership must fail closed");

        assert!(error.contains("has conflicting service owners"));
        assert!(service.fetches.contains_key(&work_id));
        assert_eq!(
            service.fetch_by_manifest.get(&manifest_hash),
            Some(&work_id)
        );
        assert_eq!(service.local_completions.len(), 1);
        assert!(service.output_guard.restart_required());
    }

    #[test]
    fn service_monotonically_upgrades_body_fetch_authority_in_both_orders() {
        let (mut service, keys) = fixture();
        let _chunk_root = install_temporary_chunk_root(&mut service);
        allow_fixture_block_payload(&mut service.context);
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let ordinary = BodyFetchTask::ordinary_for_test(51, tag, payload.manifest().clone());
        let hybrid = certified_fetch_task(
            &service,
            51,
            tag,
            Some(payload.manifest().clone()),
            proposal.round,
            proposal.subject,
        );
        service
            .enqueue_body_fetch(ordinary)
            .expect("start manifest acquisition");
        service
            .enqueue_body_fetch(hybrid.clone())
            .expect("add certified authority");
        let live = service.fetches.get(&hybrid.id()).expect("hybrid owner");
        assert_eq!(live.task, hybrid);
        assert!(live.chunks.is_some());
        assert_eq!(
            service
                .fetch_by_manifest
                .get(&HashOf::new(payload.manifest())),
            Some(&hybrid.id())
        );

        let (mut service, keys) = fixture();
        let _chunk_root = install_temporary_chunk_root(&mut service);
        allow_fixture_block_payload(&mut service.context);
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let certified =
            certified_fetch_task(&service, 52, tag, None, proposal.round, proposal.subject);
        let hybrid = certified_fetch_task(
            &service,
            52,
            tag,
            Some(payload.manifest().clone()),
            proposal.round,
            proposal.subject,
        );
        service
            .enqueue_body_fetch(certified)
            .expect("start certified acquisition");
        assert!(service.fetch_by_manifest.is_empty());
        service
            .enqueue_body_fetch(hybrid.clone())
            .expect("add manifest authority");
        let live = service.fetches.get(&hybrid.id()).expect("hybrid owner");
        assert_eq!(live.task, hybrid);
        assert!(live.chunks.is_some());
        assert_eq!(
            service
                .fetch_by_manifest
                .get(&HashOf::new(payload.manifest())),
            Some(&hybrid.id())
        );
    }

    #[test]
    fn certified_completion_retires_exact_live_or_reconstructed_owner() {
        let (mut service, keys) = fixture();
        let _chunk_root = install_temporary_chunk_root(&mut service);
        allow_fixture_block_payload(&mut service.context);
        let (body, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let live_task = certified_fetch_task(
            &service,
            53,
            tag,
            Some(payload.manifest().clone()),
            proposal.round,
            proposal.subject,
        );
        service
            .enqueue_body_fetch(live_task.clone())
            .expect("start hybrid fetch");
        service
            .complete_certified_body_fetch(&live_task)
            .expect("certified response retires live owner");
        assert!(service.fetches.is_empty());
        assert!(service.fetch_by_manifest.is_empty());

        let queued_ordinary = BodyFetchTask::ordinary_for_test(54, tag, payload.manifest().clone());
        let queued_task = certified_fetch_task(
            &service,
            54,
            tag,
            Some(payload.manifest().clone()),
            proposal.round,
            proposal.subject,
        );
        service
            .local_completions
            .push_back(LocalCompletion::Reconstructed {
                task: queued_ordinary,
                manifest: payload.manifest().clone(),
                body,
            });
        service
            .enqueue_body_fetch(queued_task.clone())
            .expect("queued reconstruction accepts certified upgrade");
        service
            .complete_certified_body_fetch(&queued_task)
            .expect("certified response retires queued reconstruction");
        assert!(service.local_completions.is_empty());
        assert!(!service.output_guard.restart_required());
    }

    #[test]
    fn cancellation_rejects_a_different_task_without_consuming_exact_owner() {
        let (mut service, keys) = fixture();
        let _chunk_root = install_temporary_chunk_root(&mut service);
        allow_fixture_block_payload(&mut service.context);
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(55, tag, payload.manifest().clone());
        service
            .enqueue_body_fetch(task.clone())
            .expect("start exact fetch");
        let wrong = BodyFetchTask::ordinary_for_test(
            55,
            EventTag::new(
                service.context.height,
                proposal.round.view,
                Generation::new(service.context.height + 1),
            ),
            payload.manifest().clone(),
        );

        let error = service
            .cancel_body_fetch(&wrong)
            .expect_err("different task identity must fail closed");

        assert!(error.contains("differs from executor ownership"));
        assert!(service.fetches.contains_key(&task.id()));
        assert_eq!(
            service
                .fetch_by_manifest
                .get(&HashOf::new(payload.manifest())),
            Some(&task.id())
        );
        assert!(service.output_guard.restart_required());
    }

    #[test]
    fn corrupt_manifest_index_is_preserved_and_fails_closed_before_cancellation() {
        let (mut service, keys) = fixture();
        let _chunk_root = install_temporary_chunk_root(&mut service);
        allow_fixture_block_payload(&mut service.context);
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(56, tag, payload.manifest().clone());
        service
            .enqueue_body_fetch(task.clone())
            .expect("start exact fetch");
        let manifest_hash = HashOf::new(payload.manifest());
        let innocent_owner = EffectWorkId::for_test(999);
        service
            .fetch_by_manifest
            .insert(manifest_hash, innocent_owner);

        let error = service
            .cancel_body_fetch(&task)
            .expect_err("corrupt manifest ownership must fail closed");

        assert!(error.contains("mismatched manifest owner"));
        assert_eq!(
            service.fetch_by_manifest.get(&manifest_hash),
            Some(&innocent_owner)
        );
        assert!(service.fetches.contains_key(&task.id()));
        assert!(service.output_guard.restart_required());
    }

    #[test]
    fn duplicate_queued_fetch_owners_fail_closed_without_consumption() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (body, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(57, tag, payload.manifest().clone());
        for _ in 0..2 {
            service
                .local_completions
                .push_back(LocalCompletion::Reconstructed {
                    task: task.clone(),
                    manifest: payload.manifest().clone(),
                    body: body.clone(),
                });
        }

        let error = service
            .cancel_body_fetch(&task)
            .expect_err("duplicate queue ownership must fail closed");

        assert!(error.contains("duplicate queued reconstruction owners"));
        assert_eq!(service.local_completions.len(), 2);
        assert!(service.output_guard.restart_required());
    }

    #[test]
    fn missing_orphan_and_wrong_manifest_indices_fail_closed_without_consumption() {
        let (mut service, keys) = fixture();
        let _chunk_root = install_temporary_chunk_root(&mut service);
        allow_fixture_block_payload(&mut service.context);
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(58, tag, payload.manifest().clone());
        service
            .enqueue_body_fetch(task.clone())
            .expect("start exact fetch");
        let manifest_hash = HashOf::new(payload.manifest());
        assert_eq!(
            service.fetch_by_manifest.remove(&manifest_hash),
            Some(task.id())
        );

        let error = service
            .cancel_body_fetch(&task)
            .expect_err("missing manifest index must fail closed");
        assert!(error.contains("mismatched manifest owner"));
        assert!(service.fetch_by_manifest.is_empty());
        assert!(service.fetches.contains_key(&task.id()));
        assert!(service.output_guard.restart_required());

        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(59, tag, payload.manifest().clone());
        let manifest_hash = HashOf::new(payload.manifest());
        service.fetch_by_manifest.insert(manifest_hash, task.id());

        let error = service
            .cancel_body_fetch(&task)
            .expect_err("orphan manifest index must fail closed");
        assert!(error.contains("orphaned manifest owner"));
        assert_eq!(
            service.fetch_by_manifest.get(&manifest_hash),
            Some(&task.id())
        );
        assert!(service.fetches.is_empty());
        assert!(service.output_guard.restart_required());

        let (mut service, keys) = fixture();
        let _chunk_root = install_temporary_chunk_root(&mut service);
        allow_fixture_block_payload(&mut service.context);
        let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = BodyFetchTask::ordinary_for_test(60, tag, payload.manifest().clone());
        service
            .enqueue_body_fetch(task.clone())
            .expect("start exact fetch");
        let manifest_hash = HashOf::new(payload.manifest());
        let wrong_owner = EffectWorkId::for_test(1_000);
        service.fetch_by_manifest.insert(manifest_hash, wrong_owner);

        let error = service
            .cancel_body_fetch(&task)
            .expect_err("wrong manifest index must fail closed");
        assert!(error.contains("mismatched manifest owner"));
        assert_eq!(
            service.fetch_by_manifest.get(&manifest_hash),
            Some(&wrong_owner)
        );
        assert!(service.fetches.contains_key(&task.id()));
        assert!(service.output_guard.restart_required());
    }

    #[test]
    fn io_queue_cancellation_frees_capacity_without_reordering_retained_work() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let sign = |id| V2IoCommand::Sign {
            task: ConsensusSignTask::for_test(
                id,
                tag,
                super::super::v2::SignRequest::Proposal(proposal.clone()),
            ),
            restore_outbound_payload: false,
        };
        let (command_tx, command_rx, admission) = test_io_command_channel(2);

        command_tx
            .try_send(sign(1))
            .expect("queue first signing task");
        command_tx
            .try_send(sign(2))
            .expect("queue retained signing task");
        assert!(matches!(
            command_tx.try_send(sign(3)),
            Err(V2IoTrySendError::Full(_))
        ));
        assert!(
            command_tx
                .cancel(EffectWorkId::for_test(1), V2IoCancellableKind::Sign)
                .expect("cancel queued signing task")
        );
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);
        command_tx
            .try_send(sign(3))
            .expect("reclaimed slot accepts current-view work");

        for expected in [2, 3] {
            let command = command_rx.try_recv().expect("retained queued command");
            let work_id = command.work_id().expect("signing work identifier");
            assert_eq!(work_id, EffectWorkId::for_test(expected));
            command_rx.complete_work(work_id);
            command_tx.acknowledge_completion(work_id);
        }
        assert!(matches!(
            command_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);

        let durable = DurableBodyReceipt::for_test(
            service.context.id(),
            proposal.round,
            proposal.subject,
            HashOf::new(&proposal.manifest),
        );
        let validation = BodyValidationTask::for_test(4, tag, durable);
        let validation_id = validation.id();
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission: Arc::clone(&admission),
        });
        service
            .io()
            .expect("synthetic validation queue")
            .enqueue(V2IoCommand::Validate(validation))
            .expect("queue stale validation");
        service
            .cancel_body_validation(validation_id)
            .expect("production callback cancels queued validation");
        assert!(matches!(
            command_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
        drop(service.io.take());
    }

    #[test]
    fn io_queue_duplicate_apply_coalesces_and_conflicting_work_id_fails_closed() {
        let (mut service, keys) = fixture();
        allow_fixture_block_payload(&mut service.context);
        let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
        let durable = DurableBodyReceipt::for_test(
            service.context.id(),
            proposal.round,
            proposal.subject,
            HashOf::new(&proposal.manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable);
        let certificate = wire::QuorumCertificate {
            round: proposal.round,
            phase: wire::GlobalPhase::Commit,
            subject: proposal.subject,
            execution_commitment: validated.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let tag = EventTag::new(
            service.context.height,
            proposal.round.view,
            Generation::new(service.context.height),
        );
        let task = ApplyTask::for_test(
            7,
            tag,
            proposal.subject,
            certificate.clone(),
            validated.clone(),
        );
        let conflicting = ApplyTask::for_test(
            7,
            EventTag::new(
                service.context.height,
                proposal.round.view + 1,
                Generation::new(service.context.height),
            ),
            proposal.subject,
            certificate,
            validated,
        );
        let (command_tx, command_rx, admission) = test_io_command_channel(1);

        command_tx
            .try_send(V2IoCommand::Apply(task.clone()))
            .expect("queue exact apply");
        command_tx
            .try_send(V2IoCommand::Apply(task.clone()))
            .expect("coalesce queued exact apply retransmission");
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);
        assert!(matches!(
            command_tx.try_send(V2IoCommand::Apply(conflicting)),
            Err(V2IoTrySendError::ConflictingWorkId { work_id })
                if work_id == EffectWorkId::for_test(7)
        ));

        let command = command_rx.try_recv().expect("single coalesced apply");
        let work_id = command.work_id().expect("apply work identifier");
        assert_eq!(work_id, EffectWorkId::for_test(7));
        assert!(matches!(command, V2IoCommand::Apply(_)));
        command_tx
            .try_send(V2IoCommand::Apply(task.clone()))
            .expect("coalesce exact retransmission while apply is active");
        command_rx.complete_work(work_id);
        command_tx
            .try_send(V2IoCommand::Apply(task.clone()))
            .expect("coalesce exact retransmission while completion is pending");
        assert!(matches!(
            command_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        drop(command_rx);
        command_tx.acknowledge_completion(work_id);
        assert!(command_tx.queue.lock().work.is_empty());
    }

    #[test]
    fn remote_auxiliary_flood_cannot_consume_consensus_or_control_reservations() {
        let admission = Arc::new(V2IoAdmission::new(1, 2).expect("bounded I/O admission"));
        assert_eq!(admission.capacity(), 4);
        let (command_tx, command_rx) =
            v2_io_command_channel(admission.capacity(), Arc::clone(&admission));
        let (_completion_tx, completion_rx) = mpsc::sync_channel(admission.capacity());
        let io = V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"reserved I/O block",
            )),
            payload_hash: Hash::new(b"reserved I/O payload"),
        };
        let command = |view| V2IoCommand::LoadCandidate {
            tag: EventTag::new(1, view, Generation::new(1)),
            subject,
        };
        assert_eq!(command(97).admission_class(), V2IoAdmissionClass::Control);
        assert!(V2IoAdmission::new(usize::MAX, 1).is_err());

        io.try_enqueue_as(V2IoAdmissionClass::Auxiliary, command(0))
            .expect("first authenticated service request occupies its prefix");
        assert!(!io.can_enqueue_as(V2IoAdmissionClass::Auxiliary));
        assert!(io.can_enqueue_as(V2IoAdmissionClass::Consensus));
        assert!(io.can_enqueue_as(V2IoAdmissionClass::Control));
        assert!(matches!(
            io.try_enqueue_as(V2IoAdmissionClass::Auxiliary, command(99)),
            Err(V2IoTrySendError::Full(_))
        ));
        io.try_enqueue_as(V2IoAdmissionClass::Consensus, command(1))
            .expect("first reserved consensus command");
        io.try_enqueue_as(V2IoAdmissionClass::Consensus, command(2))
            .expect("second reserved consensus command");
        assert!(matches!(
            io.try_enqueue_as(V2IoAdmissionClass::Consensus, command(98)),
            Err(V2IoTrySendError::Full(_))
        ));
        io.try_enqueue_as(V2IoAdmissionClass::Control, command(3))
            .expect("trusted local control reserve");

        let views = command_rx
            .try_iter()
            .map(|command| match command {
                V2IoCommand::LoadCandidate { tag, .. } => tag.view(),
                _ => panic!("unexpected command in admission test"),
            })
            .collect::<Vec<_>>();
        assert_eq!(views, vec![0, 1, 2, 3]);
        assert_eq!(io.admission.queued.load(AtomicOrdering::Acquire), 0);
        assert!(io.can_enqueue_as(V2IoAdmissionClass::Auxiliary));
        io.try_enqueue_as(V2IoAdmissionClass::Auxiliary, command(4))
            .expect("worker receive releases auxiliary admission");
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::LoadCandidate { tag, .. }) if tag.view() == 4
        ));
    }

    #[test]
    fn abnormal_service_drop_shuts_worker_down_before_blocking_final_drain() {
        let (mut service, _) = fixture();
        service.clean_teardown = false;
        let output_guard = Arc::clone(&service.output_guard);
        let permit_guard = Arc::clone(&output_guard);
        let (permit_ready_tx, permit_ready_rx) = mpsc::sync_channel(1);
        let (release_permit_tx, release_permit_rx) = mpsc::sync_channel(1);
        let permit_holder = thread::spawn(move || {
            let admitted_output = permit_guard.acquire().expect("admit earlier output");
            permit_ready_tx.send(()).expect("publish admitted output");
            release_permit_rx
                .recv()
                .expect("release admitted output after worker shutdown");
            drop(admitted_output);
        });
        permit_ready_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("earlier output must be admitted before abnormal teardown");
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let (shutdown_seen_tx, shutdown_seen_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Shutdown)));
            shutdown_seen_tx.send(()).expect("publish worker shutdown");
            release_permit_tx
                .send(())
                .expect("release output after worker shutdown");
            drop(completion_tx);
        });
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(worker),
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });

        drop(service);

        shutdown_seen_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("abnormal teardown must stop the worker before draining admitted output");
        permit_holder.join().expect("join admitted-output holder");
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn recovery_gate_is_cross_thread_and_precedes_fatal_completion() {
        let gate = ConsensusOutputGuard::isolated();
        let admitted_output = gate.acquire().expect("initial output permit");
        let worker_gate = Arc::clone(&gate);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let later_candidate_published = Arc::new(AtomicBool::new(false));
        let worker_candidate_published = Arc::clone(&later_candidate_published);
        let worker = thread::spawn(move || {
            let fatal_operation = worker_gate
                .begin_fail_stop_operation()
                .expect("fatal worker output operation");
            drop(fatal_operation);
            let _ = completion_tx.try_send(V2IoCompletion::RecoveryRequired(
                "committed marker requires restart".to_owned(),
            ));
            assert!(worker_gate.restart_required());
            if worker_gate.acquire().is_some() {
                worker_candidate_published.store(true, Ordering::Release);
            }
        });

        let completion = completion_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("fatal completion must follow recovery admission closure");
        assert!(matches!(
            completion,
            V2IoCompletion::RecoveryRequired(reason)
                if reason == "committed marker requires restart"
        ));
        assert!(
            gate.restart_required(),
            "the guard must close before publishing the fatal completion"
        );
        assert!(
            gate.acquire().is_none(),
            "a second output must not enter while fatal recovery activation drains"
        );
        drop(admitted_output);
        worker.join().expect("join recovery worker");
        assert!(gate.restart_required());
        assert!(gate.acquire().is_none());
        assert!(
            !later_candidate_published.load(Ordering::Acquire),
            "no candidate may be published after the fatal durability transition"
        );
    }

    #[test]
    fn io_command_panic_latches_restart_required_before_unwinding() {
        let output_guard = ConsensusOutputGuard::isolated();
        let unwind = std::panic::catch_unwind({
            let output_guard = Arc::clone(&output_guard);
            move || {
                let _ = execute_fail_stop_io_command(&output_guard, || {
                    panic!("model I/O command panic");
                });
            }
        });

        assert!(unwind.is_err());
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn retire_panic_closes_gate_before_inflight_output_drains() {
        let output_guard = ConsensusOutputGuard::isolated();
        let admitted_output = output_guard.acquire().expect("admit earlier output");
        let worker_guard = Arc::clone(&output_guard);
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let unwind = std::panic::catch_unwind(move || {
                let _ = execute_retire_io_command(&worker_guard, || {
                    entered_tx.send(()).expect("publish Retire entry");
                    panic!("model Retire panic");
                });
            });
            assert!(unwind.is_err());
        });

        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("Retire operation entered");
        let activation_deadline = Instant::now() + Duration::from_secs(1);
        while !output_guard.restart_required() && Instant::now() < activation_deadline {
            thread::yield_now();
        }
        assert!(
            output_guard.restart_required(),
            "Retire panic must close admission while earlier output still drains"
        );
        assert!(
            output_guard.acquire().is_none(),
            "no later output may cross the gate after the Retire panic"
        );

        drop(admitted_output);
        worker.join().expect("join panicking Retire model");
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn retire_failure_is_nonfatal_and_leaves_output_guard_open() {
        let output_guard = ConsensusOutputGuard::isolated();
        let mut worker_failure_guard = V2IoWorkerFailureGuard::new(
            Arc::clone(&output_guard),
            Arc::new(AtomicBool::new(false)),
        );
        let completion = execute_retire_io_command(&output_guard, || {
            Err("injected post-finality retirement failure".to_owned())
        })
        .expect("open guard admits Retire");
        assert!(matches!(
            completion,
            V2IoCompletion::RetirementFailed(reason)
                if reason == "injected post-finality retirement failure"
        ));
        worker_failure_guard.disarm();
        drop(worker_failure_guard);

        assert!(!output_guard.restart_required());
        assert!(output_guard.acquire().is_some());
    }

    #[test]
    fn io_worker_lifetime_guard_latches_panic_after_success_before_completion_delivery() {
        let output_guard = ConsensusOutputGuard::isolated();
        let unwind = std::panic::catch_unwind({
            let output_guard = Arc::clone(&output_guard);
            move || {
                let _worker_failure_guard = V2IoWorkerFailureGuard::new(
                    Arc::clone(&output_guard),
                    Arc::new(AtomicBool::new(false)),
                );
                let completion = execute_fail_stop_io_command(&output_guard, || {
                    Ok(V2IoCompletion::CertifiedRequestIgnored)
                })
                .expect("model successful I/O operation");
                assert!(matches!(
                    completion,
                    V2IoCompletion::CertifiedRequestIgnored
                ));
                panic!("model panic before completion delivery");
            }
        });

        assert!(unwind.is_err());
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn io_worker_explicit_shutdown_leaves_output_guard_open() {
        let output_guard = ConsensusOutputGuard::isolated();
        let mut worker_failure_guard = V2IoWorkerFailureGuard::new(
            Arc::clone(&output_guard),
            Arc::new(AtomicBool::new(false)),
        );
        worker_failure_guard.disarm();
        drop(worker_failure_guard);

        assert!(!output_guard.restart_required());
        assert!(output_guard.acquire().is_some());
    }

    #[test]
    fn flagged_finalized_disconnect_leaves_output_guard_open() {
        let output_guard = ConsensusOutputGuard::isolated();
        let allow_finalized_disconnect = Arc::new(AtomicBool::new(false));
        allow_finalized_disconnect.store(true, AtomicOrdering::Release);
        let worker_failure_guard =
            V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), allow_finalized_disconnect);

        drop(worker_failure_guard);

        assert!(!output_guard.restart_required());
        assert!(output_guard.acquire().is_some());
    }

    #[test]
    fn flagged_worker_panic_closes_gate_before_inflight_output_drains() {
        let output_guard = ConsensusOutputGuard::isolated();
        let admitted_output = output_guard.acquire().expect("admit earlier output");
        let allow_finalized_disconnect = Arc::new(AtomicBool::new(true));
        let worker_output_guard = Arc::clone(&output_guard);
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let _worker_failure_guard =
                V2IoWorkerFailureGuard::new(worker_output_guard, allow_finalized_disconnect);
            entered_tx.send(()).expect("publish worker entry");
            panic!("model flagged finalized-cleanup worker panic");
        });

        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("flagged worker entered");
        let activation_deadline = Instant::now() + Duration::from_secs(1);
        while !output_guard.restart_required() && Instant::now() < activation_deadline {
            thread::yield_now();
        }
        assert!(output_guard.restart_required());
        assert!(
            output_guard.acquire().is_none(),
            "the finalized-disconnect flag must never suppress panic closure"
        );

        drop(admitted_output);
        assert!(worker.join().is_err());
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn flagged_worker_fail_stop_error_still_latches_restart_required() {
        let output_guard = ConsensusOutputGuard::isolated();
        let allow_finalized_disconnect = Arc::new(AtomicBool::new(true));
        let worker_failure_guard =
            V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), allow_finalized_disconnect);

        assert!(
            execute_fail_stop_io_command(&output_guard, || {
                Err("injected fail-stop I/O error".to_owned())
            })
            .is_err()
        );
        drop(worker_failure_guard);

        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn recovery_gate_rejects_service_outputs_and_candidate_delivery() {
        let (mut service, _) = fixture();
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });
        let encoded = encode_payload(
            &service.context,
            wire::ConsensusRound {
                context_id: service.context.id(),
                height: service.context.height,
                view: 0,
            },
            wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked block")),
                payload_hash: Hash::new(b"blocked body"),
            },
            b"blocked body",
        )
        .expect("encode bounded payload");
        service
            .prepared_candidates
            .push_back(PreparedCandidateBody {
                tag: EventTag::new(1, 0, Generation::new(1)),
                subject: wire::BlockSubject {
                    parent_block_hash: None,
                    block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked candidate")),
                    payload_hash: Hash::new(b"blocked payload"),
                },
            });
        service.output_guard.activate_restart_required();

        assert!(service.take_prepared_candidate().is_none());
        let blocked_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked load block")),
            payload_hash: Hash::new(b"blocked load payload"),
        };
        assert!(
            service
                .request_locked_candidate(EventTag::new(1, 0, Generation::new(1)), blocked_subject,)
                .is_err()
        );
        assert!(service.pending_candidate_loads.is_empty());
        assert!(
            command_rx.try_recv().is_err(),
            "post-latch service work must not mutate the ordered I/O queue"
        );
        assert!(
            service.register_outbound_payload(encoded).is_err(),
            "recovery must reject new proposal material before publication"
        );
        assert!(service.output_permit().is_err());
        drop(completion_tx);
    }

    fn manifest_hash(label: &[u8]) -> HashOf<wire::PayloadManifest> {
        HashOf::from_untyped_unchecked(Hash::new(label))
    }

    fn durable_receipt(service: &ProductionV2Services, keys: &[KeyPair]) -> KuraV2CommitReceipt {
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"finalized worker block")),
            payload_hash: Hash::new(b"finalized worker payload"),
        };
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 0,
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"worker parent state"),
            Hash::new(b"worker post state"),
            Hash::new(b"worker ordinary writes"),
            Hash::new(b"worker executed block wire"),
        );
        let preimage = wire::Vote {
            round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let signature_shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signature_shares
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let certificate = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate valid worker CommitQC"),
        };
        let artifact = wire::finality::V2FinalityArtifact::new(
            service.context.clone(),
            subject,
            certificate,
            keys.iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("worker fixture validator PoP")
                })
                .collect(),
        );
        artifact.validate().expect("valid worker finality artifact");
        KuraV2CommitReceipt::for_test(&artifact)
    }

    #[test]
    fn finalized_cleanup_reports_absent_worker_and_accumulates_chunk_warning() {
        let (mut service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);
        let directory = TempDir::new().expect("cleanup test directory");
        let chunk_root = directory.path().join("chunk-root-is-a-file");
        std::fs::write(&chunk_root, b"not a directory").expect("create adversarial chunk root");
        service.chunk_root = chunk_root;

        let mut supervisor = V2CleanupSupervisor::default();
        let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);

        assert_eq!(
            outcome
                .warnings()
                .iter()
                .map(|warning| warning.target())
                .collect::<Vec<_>>(),
            vec![
                PostFinalityCleanupTarget::CleanupWorker,
                PostFinalityCleanupTarget::PayloadChunks,
            ],
            "an unavailable worker must not prevent independent chunk cleanup diagnostics"
        );
        assert!(outcome.warnings()[0].reason().contains("unavailable"));
        assert!(outcome.warnings()[1].reason().contains("chunk root"));
    }

    #[test]
    fn finalized_cleanup_reports_disconnected_worker_without_failing_rollover() {
        let (mut service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);
        let directory = TempDir::new().expect("cleanup test directory");
        service.chunk_root = directory.path().join("already-absent-chunks");
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        drop(command_rx);
        let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });

        let mut supervisor = V2CleanupSupervisor::default();
        let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);

        assert_eq!(outcome.warnings().len(), 1);
        assert_eq!(
            outcome.warnings()[0].target(),
            PostFinalityCleanupTarget::CleanupWorker
        );
        assert!(outcome.warnings()[0].reason().contains("disconnected"));
    }

    #[test]
    fn prelatched_finalized_cleanup_mutates_neither_queue_nor_chunks() {
        let (mut service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);
        let directory = TempDir::new().expect("cleanup test directory");
        let chunk_root = directory.path().join("retained-chunks");
        std::fs::create_dir_all(&chunk_root).expect("seed retained chunk root");
        std::fs::write(chunk_root.join("chunk"), b"retained").expect("seed retained chunk");
        service.chunk_root = chunk_root.clone();
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });
        service.output_guard.activate_restart_required();

        let mut supervisor = V2CleanupSupervisor::default();
        let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);

        assert!(command_rx.try_recv().is_err());
        assert!(chunk_root.join("chunk").is_file());
        assert_eq!(outcome.warnings().len(), 2);
        assert!(
            outcome
                .warnings()
                .iter()
                .all(|warning| warning.reason().contains("restart"))
        );
    }

    #[test]
    fn finalized_cleanup_retains_pending_worker_failure_then_confirms_retirement() {
        let (mut service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);
        let directory = TempDir::new().expect("cleanup test directory");
        service.chunk_root = directory.path().join("already-absent-chunks");
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(2);
        let join = thread::spawn(move || {
            assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Retire(_))));
            completion_tx
                .send(V2IoCompletion::Failed(
                    "late queued service diagnostic".to_owned(),
                ))
                .expect("send retained worker failure");
            completion_tx
                .send(V2IoCompletion::Retired)
                .expect("confirm body retirement");
        });
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(join),
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });

        let mut supervisor = V2CleanupSupervisor::default();
        let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);

        assert_eq!(outcome.warnings().len(), 1);
        assert_eq!(
            outcome.warnings()[0].target(),
            PostFinalityCleanupTarget::CleanupWorker
        );
        assert!(
            outcome.warnings()[0]
                .reason()
                .contains("late queued service diagnostic")
        );
    }

    #[test]
    fn finalized_cleanup_deadline_releases_rollover_and_supervises_silent_worker() {
        let (mut service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);
        let directory = TempDir::new().expect("cleanup test directory");
        service.chunk_root = directory.path().join("already-absent-chunks");
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let (accepted_tx, accepted_rx) = mpsc::sync_channel(1);
        let join = thread::spawn(move || {
            assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Retire(_))));
            accepted_tx
                .send(())
                .expect("announce accepted retirement request");
            // Deliberately withhold a completion. Closing the command channel
            // at the deadline must still give this worker a supervised exit.
            assert!(command_rx.recv().is_err());
            drop(completion_tx);
        });
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(join),
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });
        let mut supervisor = V2CleanupSupervisor::default();
        let started = Instant::now();

        let outcome = service.finish_height(receipt, Duration::from_millis(10), &mut supervisor);

        accepted_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("worker accepted the queued Retire request");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "a silent post-finality worker must not hold successor rollover"
        );
        assert_eq!(outcome.warnings().len(), 1);
        assert_eq!(
            outcome.warnings()[0].target(),
            PostFinalityCleanupTarget::CleanupWorker
        );
        assert!(outcome.warnings()[0].reason().contains("deadline"));

        let reap_deadline = Instant::now() + Duration::from_secs(1);
        while supervisor.pending_workers() != 0 && Instant::now() < reap_deadline {
            supervisor.reap_finished();
            thread::yield_now();
        }
        assert_eq!(
            supervisor.pending_workers(),
            0,
            "the timed-out worker must be reaped rather than detached"
        );
    }

    #[test]
    fn finalized_cleanup_full_queue_timeout_allows_normal_worker_disconnect() {
        let (mut service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);
        let directory = TempDir::new().expect("cleanup test directory");
        service.chunk_root = directory.path().join("already-absent-chunks");
        let output_guard = Arc::clone(&service.output_guard);
        let allow_finalized_disconnect = Arc::new(AtomicBool::new(false));
        let worker_allow_finalized_disconnect = Arc::clone(&allow_finalized_disconnect);
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        let queued_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"queued cleanup block")),
            payload_hash: Hash::new(b"queued cleanup payload"),
        };
        assert!(
            command_tx
                .try_send(V2IoCommand::LoadCandidate {
                    tag: EventTag::new(
                        service.context.height,
                        0,
                        Generation::new(service.context.height),
                    ),
                    subject: queued_subject,
                })
                .is_ok(),
            "fill ordered I/O queue before Retire enqueue"
        );
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let worker_output_guard = Arc::clone(&output_guard);
        let join = thread::spawn(move || {
            let _worker_failure_guard =
                V2IoWorkerFailureGuard::new(worker_output_guard, worker_allow_finalized_disconnect);
            release_rx
                .recv()
                .expect("release full-queue cleanup worker");
            assert!(matches!(
                command_rx.recv(),
                Ok(V2IoCommand::LoadCandidate { .. })
            ));
            assert!(command_rx.recv().is_err());
            drop(completion_tx);
        });
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(join),
            allow_finalized_disconnect: Arc::clone(&allow_finalized_disconnect),
            admission,
        });
        let mut supervisor = V2CleanupSupervisor::default();

        let outcome = service.finish_height(receipt, Duration::from_millis(10), &mut supervisor);

        assert!(
            allow_finalized_disconnect.load(AtomicOrdering::Acquire),
            "typed-finality timeout must authorize the ensuing normal disconnect"
        );
        assert_eq!(outcome.warnings().len(), 1);
        assert!(outcome.warnings()[0].reason().contains("enqueue exceeded"));
        assert!(!output_guard.restart_required());
        release_tx.send(()).expect("release cleanup worker");
        let reap_deadline = Instant::now() + Duration::from_secs(1);
        while supervisor.pending_workers() != 0 && Instant::now() < reap_deadline {
            supervisor.reap_finished();
            thread::yield_now();
        }
        assert_eq!(supervisor.pending_workers(), 0);
        assert!(!output_guard.restart_required());
        assert!(output_guard.acquire().is_some());
    }

    #[test]
    fn retirement_failure_and_chunk_failure_preserve_typed_warning_order() {
        let (mut service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);
        let directory = TempDir::new().expect("cleanup test directory");
        let chunk_root = directory.path().join("chunk-root-is-a-file");
        std::fs::write(&chunk_root, b"not a directory").expect("create adversarial chunk root");
        service.chunk_root = chunk_root;
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let join = thread::spawn(move || {
            assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Retire(_))));
            completion_tx
                .send(V2IoCompletion::RetirementFailed(
                    "adversarial body retirement failure".to_owned(),
                ))
                .expect("send body retirement failure");
        });
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(join),
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });
        let mut supervisor = V2CleanupSupervisor::default();

        let outcome = service.finish_height(receipt, Duration::from_secs(1), &mut supervisor);

        assert_eq!(
            outcome
                .warnings()
                .iter()
                .map(|warning| warning.target())
                .collect::<Vec<_>>(),
            vec![
                PostFinalityCleanupTarget::CleanupWorker,
                PostFinalityCleanupTarget::DurableBodies,
                PostFinalityCleanupTarget::PayloadChunks,
            ]
        );
        assert!(outcome.warnings()[1].reason().contains("adversarial"));
        assert!(outcome.warnings()[2].reason().contains("chunk root"));
    }

    #[test]
    fn cleanup_diagnostics_retain_height_context_and_block_hash() {
        let (service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);

        let identity = CleanupWorkerIdentity::from_receipt(&receipt);

        assert_eq!(identity.height, receipt.height());
        assert_eq!(identity.context_id, receipt.context_id());
        assert_eq!(identity.block_hash, receipt.block_hash());
    }

    fn merge_sidecar_reference(label: &[u8]) -> CertifiedMergeLedgerReference {
        CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::<MergeLedgerEntry>::from_untyped_unchecked(Hash::new(label)),
            encoded_len: 512,
            epoch_id: 9,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate::new(
                2,
                9,
                1,
                HashOf::from_untyped_unchecked(Hash::new(b"merge parent")),
                Hash::new(b"chain id"),
                1,
                HashOf::new(&Vec::<PeerId>::new()),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"merge certificate message"),
            ),
        }
    }

    fn chunk(
        manifest_hash: HashOf<wire::PayloadManifest>,
        index: u32,
        bytes: &[u8],
        sender: wire::ValidatorIndex,
    ) -> wire::PayloadChunk {
        wire::PayloadChunk {
            manifest_hash,
            index,
            bytes: bytes.to_vec(),
            sender,
            signature: vec![0xA5],
        }
    }

    #[test]
    fn orphan_chunk_bounds_preserve_exact_duplicate_semantics_at_capacity() {
        let (mut service, _) = fixture();
        let hash = manifest_hash(b"manifest-a");
        let sender = service.context.roster[0].validator.clone();
        let first = chunk(hash, 0, b"a", 0);

        assert_eq!(
            service.buffer_orphan_payload_chunk(sender.clone(), first.clone()),
            PayloadChunkDisposition::Buffered
        );
        assert_eq!(service.orphan_chunk_count, 1);
        assert_eq!(service.orphan_chunk_bytes, 1);
        assert_eq!(
            service.buffer_orphan_payload_chunk(sender.clone(), first),
            PayloadChunkDisposition::Duplicate,
            "an exact retransmission remains idempotent even when the buffer is full"
        );
        assert_eq!(
            service.buffer_orphan_payload_chunk(sender.clone(), chunk(hash, 0, b"b", 0)),
            PayloadChunkDisposition::Rejected,
            "a conflicting claim cannot replace retained bytes"
        );
        assert_eq!(
            service.buffer_orphan_payload_chunk(
                sender,
                chunk(manifest_hash(b"manifest-b"), 0, b"c", 0)
            ),
            PayloadChunkDisposition::Rejected,
            "one unknown manifest cannot force storage beyond the global bound"
        );
        assert_eq!(service.orphan_chunk_count, 1);
        assert_eq!(service.orphan_chunk_bytes, 1);
    }

    #[test]
    fn orphan_chunk_cheap_checks_reject_spoofing_and_oversize_without_allocation() {
        let (mut service, _) = fixture();
        service.max_orphan_chunks = 8;
        let hash = manifest_hash(b"manifest-cheap-checks");
        let validator_zero = service.context.roster[0].validator.clone();
        let validator_one = service.context.roster[1].validator.clone();

        assert_eq!(
            service.buffer_orphan_payload_chunk(validator_one, chunk(hash, 0, b"a", 0)),
            PayloadChunkDisposition::Rejected,
            "outer transport identity must match the claimed validator index"
        );
        assert_eq!(
            service.buffer_orphan_payload_chunk(validator_zero.clone(), chunk(hash, 4, b"a", 0)),
            PayloadChunkDisposition::Rejected
        );
        assert_eq!(
            service.buffer_orphan_payload_chunk(validator_zero.clone(), chunk(hash, 0, &[], 0)),
            PayloadChunkDisposition::Rejected
        );
        assert_eq!(
            service.buffer_orphan_payload_chunk(
                validator_zero.clone(),
                chunk(hash, 0, b"123456789", 0)
            ),
            PayloadChunkDisposition::Rejected
        );
        service.max_orphan_chunk_bytes = 1;
        assert_eq!(
            service.buffer_orphan_payload_chunk(validator_zero, chunk(hash, 0, b"ab", 0)),
            PayloadChunkDisposition::Rejected
        );
        assert!(service.orphan_chunks.is_empty());
        assert_eq!(service.orphan_chunk_count, 0);
        assert_eq!(service.orphan_chunk_bytes, 0);
    }

    #[test]
    fn merge_sidecar_validation_deferral_retains_exact_request_idempotently() {
        let (mut service, _) = fixture();
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                b"merge carrier parent",
            ))),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"merge carrier block")),
            payload_hash: Hash::new(b"merge carrier payload"),
        };
        let reference = merge_sidecar_reference(b"merge sidecar");
        let work_id = EffectWorkId::for_test(7);

        service
            .work_deferred_for_merge_sidecar(work_id, round, subject, &reference)
            .expect("retain exact merge-sidecar deferral");
        service
            .work_deferred_for_merge_sidecar(work_id, round, subject, &reference)
            .expect("exact retransmission is idempotent");
        let mut conflicting = reference.clone();
        conflicting.encoded_len += 1;
        assert!(
            service
                .work_deferred_for_merge_sidecar(work_id, round, subject, &conflicting)
                .is_err(),
            "one work ID cannot claim conflicting reference metadata"
        );

        assert_eq!(service.merge_sidecar_deferrals.len(), 1);
        let deferred = service
            .take_merge_sidecar_deferral()
            .expect("retained merge-sidecar deferral");
        assert_eq!(deferred.round(), round);
        assert_eq!(deferred.work_id(), work_id);
        assert_eq!(deferred.subject(), subject);
        assert_eq!(deferred.reference(), &reference);
        assert!(service.take_merge_sidecar_deferral().is_none());
    }

    #[test]
    fn merge_sidecar_validation_deferral_returns_error_at_capacity_without_eviction() {
        let (mut service, _) = fixture();
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 3,
        };
        let first_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"first merge carrier")),
            payload_hash: Hash::new(b"first merge payload"),
        };
        let second_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"second merge carrier")),
            payload_hash: Hash::new(b"second merge payload"),
            ..first_subject
        };
        let first_reference = merge_sidecar_reference(b"first merge sidecar");
        let second_reference = merge_sidecar_reference(b"second merge sidecar");

        service
            .work_deferred_for_merge_sidecar(
                EffectWorkId::for_test(1),
                round,
                first_subject,
                &first_reference,
            )
            .expect("fill bounded deferral queue");
        assert_eq!(service.merge_sidecar_deferrals.len(), 1);
        assert!(
            service
                .work_deferred_for_merge_sidecar(
                    EffectWorkId::for_test(2),
                    round,
                    second_subject,
                    &second_reference,
                )
                .is_err(),
            "a different validation cannot displace the retained exact request"
        );

        assert_eq!(service.merge_sidecar_deferrals.len(), 1);
        let retained = service
            .take_merge_sidecar_deferral()
            .expect("original deferral remains retained");
        assert_eq!(retained.subject(), first_subject);
        assert_eq!(retained.reference(), &first_reference);
    }

    #[test]
    fn outbound_payload_registration_is_exactly_idempotent_and_signed() {
        let (mut service, _) = fixture();
        service.max_orphan_chunks = 8;
        let payload = b"authoritative body";
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block")),
            payload_hash: Hash::new(payload),
        };
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 0,
        };
        let encoded = encode_payload(&service.context, round, subject, payload).expect("encode");
        let expected_manifest = encoded.manifest().clone();

        assert_eq!(
            service
                .register_outbound_payload(encoded.clone())
                .expect("first registration"),
            expected_manifest
        );
        assert_eq!(
            service
                .register_outbound_payload(encoded)
                .expect("exact retransmission"),
            expected_manifest
        );
        let messages = service
            .outbound_chunks
            .get(&HashOf::new(&expected_manifest))
            .expect("retained chunks");
        assert_eq!(messages.len(), expected_manifest.chunk_hashes.len());
        assert!(messages.iter().all(|message| matches!(
            &message.payload,
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk) if !chunk.signature.is_empty()
        )));
    }

    #[test]
    fn observer_cannot_register_or_disseminate_a_proposal_payload() {
        let (mut service, _) = fixture();
        service.local_validator = None;
        let payload = b"observer payload";
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"observer block")),
            payload_hash: Hash::new(payload),
        };
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 0,
        };
        let encoded = encode_payload(&service.context, round, subject, payload).expect("encode");

        assert!(service.register_outbound_payload(encoded).is_err());
        assert!(service.outbound_chunks.is_empty());
    }

    #[test]
    fn pipeline_release_tracks_only_successfully_queued_durable_prepare_intent() {
        let (mut service, _) = fixture();
        let (command_tx, command_rx, admission) = test_io_command_channel(1);
        let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });
        let tag = EventTag::new(
            service.context.height,
            0,
            Generation::new(service.context.height),
        );
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"prepared block")),
            payload_hash: Hash::new(b"prepared payload"),
        };
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 0,
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"worker prepared parent state"),
            Hash::new(b"worker prepared post state"),
            Hash::new(b"worker prepared ordinary writes"),
            Hash::new(b"worker prepared executed block wire"),
        );
        let vote = |phase| wire::Vote {
            round,
            phase,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };

        service
            .enqueue_consensus_sign(ConsensusSignTask::for_test(
                1,
                tag,
                super::super::v2::SignRequest::Vote(vote(wire::GlobalPhase::Prepare)),
            ))
            .expect("queue Prepare signature");
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Sign {
                restore_outbound_payload: false,
                ..
            })
        ));
        assert_eq!(
            service.take_prepared_candidate(),
            Some(PreparedCandidateBody { tag, subject })
        );

        service
            .enqueue_consensus_sign(ConsensusSignTask::for_test(
                2,
                tag,
                super::super::v2::SignRequest::Vote(vote(wire::GlobalPhase::Commit)),
            ))
            .expect("queue Commit signature");
        assert!(matches!(
            command_rx.try_recv(),
            Ok(V2IoCommand::Sign {
                restore_outbound_payload: false,
                ..
            })
        ));
        assert_eq!(service.take_prepared_candidate(), None);

        // No worker owns this synthetic channel; remove it before service Drop
        // attempts the production shutdown handshake.
        drop(service.io.take());
    }
}
