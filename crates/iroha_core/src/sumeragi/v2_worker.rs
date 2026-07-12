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
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use iroha_crypto::{HashOf, KeyPair, Signature};
use iroha_data_model::{
    block::{CertifiedMergeLedgerReference, consensus_v2 as wire},
    merge::MergeCommitteeSignature,
    peer::PeerId,
};
use iroha_p2p::{Post, Priority};
use iroha_sumeragi_core::{EquivocationKind, EventTag};

use super::{
    message::{BlockMessage, BlockMessageWire},
    output_guard::{ConsensusOutputGuard, ConsensusOutputPermit},
    v2_apply::V2ApplyService,
    v2_body_store::{BodyStoreCompletion, BodyValidationCompletion, V2BodyStore},
    v2_chunks::{EncodedV2Payload, V2ChunkSession},
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
    Sign(ConsensusSignTask),
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

enum V2IoCompletion {
    Signature(EffectWorkId, Vec<u8>),
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

struct V2IoHandle {
    command_tx: mpsc::SyncSender<V2IoCommand>,
    completion_rx: mpsc::Receiver<V2IoCompletion>,
    join: Option<thread::JoinHandle<()>>,
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
        queue_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<Self, String> {
        let capacity = queue_capacity.max(1);
        let (command_tx, command_rx) = mpsc::sync_channel(capacity);
        let (completion_tx, completion_rx) = mpsc::sync_channel(capacity);
        let join = super::sumeragi_thread_builder("sumeragi-v2-io")
            .spawn(move || {
                while let Ok(command) = command_rx.recv() {
                    let Some(output_permit) = output_guard.acquire() else {
                        break;
                    };
                    let completion = match command {
                        V2IoCommand::Sign(task) => sign_consensus_task(&key_pair, task),
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
                        V2IoCommand::Apply(task) => {
                            match apply_service.execute(&context, &mut body_store, &task) {
                                Ok(completion) => Ok(V2IoCompletion::Applied(completion)),
                                Err(
                                    super::v2_apply::V2ApplyError::MissingCertifiedMergeSidecar {
                                        reference,
                                    },
                                ) => Ok(V2IoCompletion::ApplyDeferred {
                                    work_id: task.id(),
                                    reference,
                                }),
                                Err(error) if error.requires_restart_recovery() => Ok(
                                    V2IoCompletion::RecoveryRequired(error.to_string()),
                                ),
                                Err(error) => Err(error.to_string()),
                            }
                        }
                        V2IoCommand::Serve(request) => {
                            serve_certified_body(&body_store, &key_pair, local_validator, request)
                        }
                        V2IoCommand::LoadCandidate { tag, subject } => {
                            load_candidate_body(&body_store, tag, subject)
                        }
                        V2IoCommand::Retire(receipt) => {
                            let completion = match body_store.retire_height(&receipt) {
                                Ok(()) => V2IoCompletion::Retired,
                                Err(error) => V2IoCompletion::RetirementFailed(error.to_string()),
                            };
                            drop(output_permit);
                            let _ = completion_tx.send(completion);
                            break;
                        }
                        V2IoCommand::Shutdown => {
                            drop(output_permit);
                            break;
                        }
                    };
                    drop(output_permit);
                    match completion {
                        Ok(V2IoCompletion::RecoveryRequired(reason)) | Err(reason) => {
                            publish_recovery_required(&output_guard, &completion_tx, reason);
                            break;
                        }
                        Ok(completion) => send_completion(&completion_tx, Ok(completion)),
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            command_tx,
            completion_rx,
            join: Some(join),
        })
    }

    fn enqueue(&self, command: V2IoCommand) -> Result<(), String> {
        self.command_tx
            .try_send(command)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Sumeragi v2 I/O queue is full".to_owned(),
                mpsc::TrySendError::Disconnected(_) => {
                    "Sumeragi v2 I/O worker is disconnected".to_owned()
                }
            })
    }

    fn shutdown(mut self) -> Result<(), String> {
        let mut command = V2IoCommand::Shutdown;
        loop {
            match self.command_tx.try_send(command) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Full(returned)) => {
                    command = returned;
                    if self.completion_rx.recv().is_err() {
                        break;
                    }
                }
                Err(mpsc::TrySendError::Disconnected(_)) => break,
            }
        }
        // The worker can have commands ahead of Shutdown. Drain their bounded
        // completions so it can reach Shutdown without a cyclic channel wait.
        while self.completion_rx.recv().is_ok() {}
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

fn publish_recovery_required(
    output_guard: &ConsensusOutputGuard,
    sender: &mpsc::SyncSender<V2IoCompletion>,
    reason: String,
) {
    output_guard.activate_restart_required();
    let _ = sender.try_send(V2IoCompletion::RecoveryRequired(reason));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CleanupCompletionWaitError {
    DeadlineElapsed,
    Disconnected,
}

fn recv_cleanup_completion(
    receiver: &mpsc::Receiver<V2IoCompletion>,
    deadline: Instant,
) -> Result<V2IoCompletion, CleanupCompletionWaitError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(CleanupCompletionWaitError::DeadlineElapsed)?;
    receiver
        .recv_timeout(remaining)
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => CleanupCompletionWaitError::DeadlineElapsed,
            mpsc::RecvTimeoutError::Disconnected => CleanupCompletionWaitError::Disconnected,
        })
}

fn sign_consensus_task(
    key_pair: &KeyPair,
    task: ConsensusSignTask,
) -> Result<V2IoCompletion, String> {
    let preimage = match task.request() {
        super::v2::SignRequest::Proposal(proposal) => proposal.signature_preimage(),
        super::v2::SignRequest::Vote(vote) => vote.signature_preimage(),
        super::v2::SignRequest::TimeoutVote(vote) => vote.signature_preimage(),
    };
    Signature::try_new(key_pair.private_key(), &preimage)
        .map(|signature| V2IoCompletion::Signature(task.id(), signature.payload().to_vec()))
        .map_err(|error| error.to_string())
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
        work_id: EffectWorkId,
        manifest: wire::PayloadManifest,
        body: Vec<u8>,
    },
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
        io_queue_capacity: usize,
        orphan_chunk_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<Self, String> {
        if io_queue_capacity == 0 || orphan_chunk_capacity == 0 {
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
            io_queue_capacity,
            Arc::clone(&output_guard),
        )?;
        Ok(Self {
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
            max_merge_sidecar_deferrals: io_queue_capacity,
            local_completions: VecDeque::new(),
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
            clean_teardown: false,
        })
    }

    /// Sign and retain all canonical chunks for proposal and retransmission.
    pub(crate) fn register_outbound_payload(
        &mut self,
        payload: EncodedV2Payload,
    ) -> Result<wire::PayloadManifest, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard.acquire().ok_or_else(|| {
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
        Ok(manifest)
    }

    /// Work identifier waiting for a chunk from one manifest.
    pub(crate) fn fetch_work_for_manifest(
        &self,
        manifest_hash: HashOf<wire::PayloadManifest>,
    ) -> Option<EffectWorkId> {
        self.fetch_by_manifest.get(&manifest_hash).copied()
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
        if !self.pending_candidate_loads.insert(tag) {
            return Ok(());
        }
        if let Err(error) = self.enqueue_io(V2IoCommand::LoadCandidate { tag, subject })
        {
            self.pending_candidate_loads.remove(&tag);
            return Err(error);
        }
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
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if let Some(existing) = self
            .merge_sidecar_deferrals
            .iter()
            .find(|existing| existing.work_id == deferred.work_id)
        {
            return if existing.round == deferred.round
                && existing.subject == deferred.subject
                && existing.reference == deferred.reference
            {
                Ok(())
            } else {
                Err("Sumeragi v2 work ID claimed conflicting merge-sidecar deferrals".to_owned())
            };
        }
        if self.merge_sidecar_deferrals.len() >= self.max_merge_sidecar_deferrals {
            return Err("Sumeragi v2 merge-sidecar deferral queue is full".to_owned());
        }
        self.merge_sidecar_deferrals.push_back(deferred);
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

    /// Drain tagged I/O and reconstruction completions into the reducer owner.
    pub(crate) fn drain_completions(
        &mut self,
        executor: &mut V2EffectExecutor,
    ) -> Result<usize, EffectExecutorError> {
        if self.output_guard.restart_required() {
            return Err(executor.external_service_failed(
                "Sumeragi v2 consensus requires process restart",
                self,
            ));
        }
        let mut completions = Vec::new();
        if let Some(io) = self.io.as_ref() {
            while let Ok(completion) = io.completion_rx.try_recv() {
                completions.push(completion);
            }
        }
        let mut count = 0usize;
        for completion in completions {
            count = count.saturating_add(1);
            match completion {
                V2IoCompletion::Signature(id, signature) => {
                    let _ = executor.complete_consensus_signature(id, signature, self)?;
                }
                V2IoCompletion::Stored(completion) => {
                    let _ = executor.complete_body_store(completion, self)?;
                }
                V2IoCompletion::Validated(completion) => {
                    let _ = executor.complete_body_validation(completion, self)?;
                }
                V2IoCompletion::Applied(completion) => {
                    let _ = executor.complete_application(completion, self)?;
                }
                V2IoCompletion::ApplyDeferred { work_id, reference } => {
                    let _ =
                        executor.defer_application_for_merge_sidecar(work_id, &reference, self)?;
                }
                V2IoCompletion::CertifiedResponse {
                    recipient,
                    response,
                } => self.post_to_peer(
                    recipient,
                    wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                    ),
                ),
                V2IoCompletion::CertifiedRequestIgnored => {}
                V2IoCompletion::CandidateLoaded(candidate) => {
                    self.pending_candidate_loads.remove(&candidate.tag());
                    self.loaded_candidates.push_back(candidate);
                }
                V2IoCompletion::Failed(reason) => {
                    return Err(executor.external_service_failed(reason, self));
                }
                V2IoCompletion::Retired => {
                    return Err(executor.external_service_failed(
                        "unexpected early Sumeragi v2 storage retirement",
                        self,
                    ));
                }
                V2IoCompletion::RetirementFailed(reason) => {
                    return Err(executor.external_service_failed(
                        format!(
                            "unexpected early Sumeragi v2 storage retirement failure: {reason}"
                        ),
                        self,
                    ));
                }
                V2IoCompletion::RecoveryRequired(reason) => {
                    return Err(executor.external_service_failed(
                        format!("canonical persistence requires restart recovery: {reason}"),
                        self,
                    ));
                }
            }
        }
        while let Some(completion) = self.local_completions.pop_front() {
            count = count.saturating_add(1);
            match completion {
                LocalCompletion::Reconstructed {
                    work_id,
                    manifest,
                    body,
                } => {
                    if let Err(error) =
                        executor.complete_body_reconstruction(work_id, manifest, body, self)
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
            let retirement_requested = 'enqueue: loop {
                match io.command_tx.try_send(command) {
                    Ok(()) => break true,
                    Err(mpsc::TrySendError::Full(returned)) => {
                        command = returned;
                        match recv_cleanup_completion(&io.completion_rx, deadline) {
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
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        outcome.record(
                            PostFinalityCleanupTarget::CleanupWorker,
                            "Sumeragi v2 I/O worker disconnected before body retirement",
                        );
                        break false;
                    }
                }
            };
            if retirement_requested {
                loop {
                    match recv_cleanup_completion(&io.completion_rx, deadline) {
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
        outcome
    }

    fn io(&self) -> Result<&V2IoHandle, String> {
        self.io
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())
    }

    fn output_permit(&self) -> Result<ConsensusOutputPermit<'_>, String> {
        self.output_guard.acquire().ok_or_else(|| {
            "Sumeragi v2 canonical persistence requires restart recovery".to_owned()
        })
    }

    fn enqueue_io(&self, command: V2IoCommand) -> Result<(), String> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.io()?.enqueue(command)
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
        self.post_block_message_while_guarded(peer, BlockMessage::V2(message), &permit);
    }

    /// Send one retained lane-local proposal, vote, or QC to a committee peer.
    pub(crate) fn post_lane_block(
        &self,
        peer: PeerId,
        message: BlockMessage,
    ) -> Result<(), String> {
        if !matches!(
            message,
            BlockMessage::LaneBlockProposal(_)
                | BlockMessage::LaneBlockVote(_)
                | BlockMessage::LaneBlockQc(_)
        ) {
            return Err("v2 lane transport rejected a legacy global block message".to_owned());
        }
        let permit = self.output_permit()?;
        self.post_block_message_while_guarded(peer, message, &permit);
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
    ) {
        let block_message = Arc::new(message);
        let wire = match BlockMessageWire::try_preencoded(block_message) {
            Ok(wire) => wire,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    %peer,
                    "refusing to send a non-canonical Sumeragi v2 block message"
                );
                return;
            }
        };
        let data = NetworkMessage::SumeragiBlock(Box::new(wire));
        self.network.post(Post {
            data,
            peer_id: peer,
            priority: Priority::High,
        });
    }

    /// Retransmit one v2 transport envelope to every other frozen voter.
    pub(crate) fn broadcast_to_voters(&self, message: wire::ConsensusMessageV2) {
        let Ok(permit) = self.output_permit() else {
            return;
        };
        self.broadcast_to_voters_while_guarded(message, &permit);
    }

    fn broadcast_to_voters_while_guarded(
        &self,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) {
        for entry in &self.context.roster {
            if entry.validator == self.local_peer {
                continue;
            }
            self.post_block_message_while_guarded(
                entry.validator.clone(),
                BlockMessage::V2(message.clone()),
                permit,
            );
        }
    }
}

impl Drop for ProductionV2Services {
    fn drop(&mut self) {
        if !self.clean_teardown {
            self.output_guard.activate_restart_required();
        }
        let Some(io) = self.io.take() else {
            return;
        };
        if let Err(error) = io.shutdown() {
            iroha_logger::error!(%error, "failed to stop Sumeragi v2 I/O worker");
        }
    }
}

impl V2EffectServices for ProductionV2Services {
    type Error = String;

    fn enqueue_consensus_sign(&mut self, task: ConsensusSignTask) -> Result<(), Self::Error> {
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
        self.enqueue_io(V2IoCommand::Sign(task))?;
        if let Some(prepared) = prepared
            && self.prepared_candidates.len() < self.max_orphan_chunks
        {
            self.prepared_candidates.push_back(prepared);
        }
        Ok(())
    }

    fn cancel_consensus_sign(&mut self, _work_id: EffectWorkId) -> Result<(), Self::Error> {
        // The ordered worker may already be signing. Its tagged completion is
        // harmless and will be classified stale by the executor.
        Ok(())
    }

    fn broadcast_consensus(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<(), Self::Error> {
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        let permit = self.output_permit()?;
        self.broadcast_to_voters_while_guarded(message.clone(), &permit);
        if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload {
            let manifest_hash = HashOf::new(&proposal.manifest);
            let chunks = self
                .outbound_chunks
                .get(&manifest_hash)
                .ok_or_else(|| "local proposal has no retained Sumeragi v2 chunks".to_owned())?;
            for chunk in chunks {
                self.broadcast_to_voters_while_guarded(chunk.clone(), &permit);
            }
        }
        Ok(())
    }

    fn sign_body_request(&mut self, preimage: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        Signature::try_new(self.key_pair.private_key(), preimage)
            .map(|signature| signature.payload().to_vec())
            .map_err(|error| error.to_string())
    }

    fn enqueue_body_fetch(&mut self, task: BodyFetchTask) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let permit = output_guard.acquire().ok_or_else(|| {
            "Sumeragi v2 canonical persistence requires restart recovery".to_owned()
        })?;
        if let Some(existing) = self.fetches.get(&task.id()) {
            if existing.task != task {
                return Err("conflicting Sumeragi v2 body-fetch task".to_owned());
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
                            &permit,
                        );
                    }
                }
            }
            return Ok(());
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
                        &permit,
                    );
                }
            }
        }
        self.fetches
            .insert(task.id(), FetchSession { task, chunks });
        Ok(())
    }

    fn cancel_body_fetch(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if let Some(fetch) = self.fetches.remove(&work_id)
            && let Some(manifest) = fetch.task.manifest()
        {
            self.fetch_by_manifest.remove(&HashOf::new(manifest));
        }
        Ok(())
    }

    fn accept_authenticated_chunk(
        &mut self,
        work_id: EffectWorkId,
        chunk: AuthenticatedPayloadChunk,
    ) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let fetch = self
            .fetches
            .get_mut(&work_id)
            .ok_or_else(|| "unknown Sumeragi v2 chunk fetch".to_owned())?;
        let session = fetch
            .chunks
            .as_mut()
            .ok_or_else(|| "certified body fetch cannot accept chunks".to_owned())?;
        session
            .admit(chunk.chunk())
            .map_err(|error| error.to_string())?;
        let Some(body) = session.reconstruct().map_err(|error| error.to_string())? else {
            return Ok(());
        };
        let manifest = session.manifest().clone();
        self.fetch_by_manifest.remove(&HashOf::new(&manifest));
        self.fetches.remove(&work_id);
        self.local_completions
            .push_back(LocalCompletion::Reconstructed {
                work_id,
                manifest,
                body,
            });
        Ok(())
    }

    fn enqueue_body_store(&mut self, task: BodyStoreTask) -> Result<(), Self::Error> {
        self.enqueue_io(V2IoCommand::Store(task))
    }

    fn cancel_body_store(&mut self, _work_id: EffectWorkId) -> Result<(), Self::Error> {
        Ok(())
    }

    fn enqueue_body_validation(&mut self, task: BodyValidationTask) -> Result<(), Self::Error> {
        self.enqueue_io(V2IoCommand::Validate(task))
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
        self.enqueue_io(V2IoCommand::Apply(task))
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
        iroha_logger::warn!(%offender, ?round, ?kind, "authenticated Sumeragi v2 equivocation");
        Ok(())
    }

    fn report_invalid_certified_body(
        &mut self,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
    ) -> Result<(), Self::Error> {
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
    use std::sync::atomic::{AtomicBool, Ordering};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ChainId,
        block::{BlockHeader, CertifiedMergeLedgerReference},
        merge::{MergeLedgerEntry, MergeQuorumCertificate},
    };
    use tempfile::TempDir;

    use super::*;
    use crate::sumeragi::v2_chunks::encode_payload;

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

    #[test]
    fn recovery_gate_is_cross_thread_and_precedes_fatal_completion() {
        let gate = ConsensusOutputGuard::isolated();
        let admitted_output = gate.acquire().expect("initial output permit");
        let worker_gate = Arc::clone(&gate);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let later_candidate_published = Arc::new(AtomicBool::new(false));
        let worker_candidate_published = Arc::clone(&later_candidate_published);
        let worker = thread::spawn(move || {
            publish_recovery_required(
                &worker_gate,
                &completion_tx,
                "committed marker requires restart".to_owned(),
            );
            assert!(worker_gate.restart_required());
            if worker_gate.acquire().is_some() {
                worker_candidate_published.store(true, Ordering::Release);
            }
        });

        assert!(
            completion_rx
                .recv_timeout(Duration::from_millis(25))
                .is_err(),
            "recovery activation must wait for an already-admitted output"
        );
        drop(admitted_output);
        let completion = completion_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("fatal completion must follow recovery activation");
        assert!(matches!(
            completion,
            V2IoCompletion::RecoveryRequired(reason)
                if reason == "committed marker requires restart"
        ));
        worker.join().expect("join recovery worker");
        assert!(gate.restart_required());
        assert!(gate.acquire().is_none());
        assert!(
            !later_candidate_published.load(Ordering::Acquire),
            "no candidate may be published after the fatal durability transition"
        );
    }

    #[test]
    fn recovery_gate_rejects_service_outputs_and_candidate_delivery() {
        let (mut service, _) = fixture();
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
        service.prepared_candidates.push_back(PreparedCandidateBody {
            tag: EventTag::new(1, 0, iroha_sumeragi_core::Generation::new(1)),
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked candidate")),
                payload_hash: Hash::new(b"blocked payload"),
            },
        });
        service.output_guard.activate_restart_required();

        assert!(service.take_prepared_candidate().is_none());
        assert!(
            service.register_outbound_payload(encoded).is_err(),
            "recovery must reject new proposal material before publication"
        );
        assert!(service.output_permit().is_err());
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
        let (command_tx, command_rx) = mpsc::sync_channel(1);
        drop(command_rx);
        let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
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
    fn finalized_cleanup_retains_pending_worker_failure_then_confirms_retirement() {
        let (mut service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);
        let directory = TempDir::new().expect("cleanup test directory");
        service.chunk_root = directory.path().join("already-absent-chunks");
        let (command_tx, command_rx) = mpsc::sync_channel(1);
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
        let (command_tx, command_rx) = mpsc::sync_channel(1);
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
    fn retirement_failure_and_chunk_failure_preserve_typed_warning_order() {
        let (mut service, keys) = fixture();
        let receipt = durable_receipt(&service, &keys);
        let directory = TempDir::new().expect("cleanup test directory");
        let chunk_root = directory.path().join("chunk-root-is-a-file");
        std::fs::write(&chunk_root, b"not a directory").expect("create adversarial chunk root");
        service.chunk_root = chunk_root;
        let (command_tx, command_rx) = mpsc::sync_channel(1);
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
        let (command_tx, command_rx) = mpsc::sync_channel(1);
        let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
        service.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
        });
        let tag = EventTag::new(
            service.context.height,
            0,
            iroha_sumeragi_core::Generation::new(service.context.height),
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
        assert!(matches!(command_rx.try_recv(), Ok(V2IoCommand::Sign(_))));
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
        assert!(matches!(command_rx.try_recv(), Ok(V2IoCommand::Sign(_))));
        assert_eq!(service.take_prepared_candidate(), None);

        // No worker owns this synthetic channel; remove it before service Drop
        // attempts the production shutdown handshake.
        drop(service.io.take());
    }
}
