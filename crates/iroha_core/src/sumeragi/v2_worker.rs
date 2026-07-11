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

use iroha_config::parameters::actual::SumeragiNpos;
use iroha_crypto::{HashOf, KeyPair, Signature};
use iroha_data_model::{block::consensus_v2 as wire, merge::MergeCommitteeSignature, peer::PeerId};
use iroha_p2p::{Post, Priority};

use super::{
    message::{BlockMessage, BlockMessageWire},
    v2_apply::V2ApplyService,
    v2_body_store::{BodyStoreCompletion, BodyValidationCompletion, V2BodyStore},
    v2_chunks::{EncodedV2Payload, V2ChunkSession},
    v2_context_store::V2ContextStore,
    v2_core::EventTag,
    v2_effects::{
        ApplyTask, BodyFetchTask, BodyStoreTask, BodyValidationTask, ConsensusSignTask,
        DurableApplyCompletion, EffectExecutorError, EffectExecutorStatus, EffectTransportError,
        EffectWorkId, V2EffectExecutor, V2EffectServices,
    },
    v2_transport::{AuthenticatedCertifiedBodyRequest, AuthenticatedPayloadChunk},
};
use crate::{EventsSender, IrohaNetwork, NetworkMessage, kura::KuraV2CommitReceipt};

/// Hard wall-clock budget for stopping or retiring one height-local I/O
/// worker. Finality is already durable before retirement starts, so exceeding
/// this budget retains height-local files and detaches the wedged worker rather
/// than blocking successor construction indefinitely.
const V2_IO_CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const V2_IO_JOIN_POLL_INTERVAL: Duration = Duration::from_millis(1);

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
    CertifiedResponse {
        recipient: PeerId,
        response: wire::CertifiedBodyResponse,
    },
    CertifiedRequestIgnored,
    CandidateLoaded(LoadedCandidateBody),
    Retired,
    Stopped,
    Failed(String),
}

struct V2IoHandle {
    command_tx: mpsc::SyncSender<V2IoCommand>,
    completion_rx: mpsc::Receiver<V2IoCompletion>,
    join: Option<thread::JoinHandle<()>>,
}

impl V2IoHandle {
    fn spawn(
        mut body_store: V2BodyStore,
        apply_service: V2ApplyService,
        context: wire::HeightContext,
        key_pair: KeyPair,
        local_validator: Option<wire::ValidatorIndex>,
        queue_capacity: usize,
    ) -> Result<Self, String> {
        let capacity = queue_capacity.max(1);
        let (command_tx, command_rx) = mpsc::sync_channel(capacity);
        let (completion_tx, completion_rx) = mpsc::sync_channel(capacity);
        let join = super::sumeragi_thread_builder("sumeragi-v2-io")
            .spawn(move || {
                while let Ok(command) = command_rx.recv() {
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
                        V2IoCommand::Apply(task) => apply_service
                            .execute(&context, &mut body_store, &task)
                            .map(V2IoCompletion::Applied)
                            .map_err(|error| error.to_string()),
                        V2IoCommand::Serve(request) => {
                            serve_certified_body(&body_store, &key_pair, local_validator, request)
                        }
                        V2IoCommand::LoadCandidate { tag, subject } => {
                            load_candidate_body(&body_store, tag, subject)
                        }
                        V2IoCommand::Retire(receipt) => {
                            let result = body_store
                                .retire_height(&receipt)
                                .map(|()| V2IoCompletion::Retired)
                                .map_err(|error| error.to_string());
                            send_completion(&completion_tx, result);
                            break;
                        }
                        V2IoCommand::Shutdown => {
                            send_completion(&completion_tx, Ok(V2IoCompletion::Stopped));
                            break;
                        }
                    };
                    send_completion(&completion_tx, completion);
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

    fn shutdown(self) -> Result<(), String> {
        self.shutdown_with_timeout(V2_IO_CONTROL_TIMEOUT)
    }

    fn shutdown_with_timeout(mut self, timeout: Duration) -> Result<(), String> {
        let deadline = control_deadline(timeout, "shutdown")?;
        let protocol = self.request_shutdown_until(deadline);
        self.finish_control(protocol, deadline, "shutdown")
    }

    fn retire(self, receipt: KuraV2CommitReceipt) -> Result<(), String> {
        self.retire_with_timeout(receipt, V2_IO_CONTROL_TIMEOUT)
    }

    fn retire_with_timeout(
        mut self,
        receipt: KuraV2CommitReceipt,
        timeout: Duration,
    ) -> Result<(), String> {
        let deadline = control_deadline(timeout, "retirement")?;
        let retirement = self.request_retirement_until(receipt, deadline);
        self.finish_control(retirement, deadline, "retirement")
    }

    fn request_shutdown_until(&self, deadline: Instant) -> Result<(), String> {
        let mut command = V2IoCommand::Shutdown;
        let mut prior_failure = None;
        loop {
            match self.command_tx.try_send(command) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Full(returned)) => {
                    command = returned;
                    match self.recv_until(deadline, "shutdown")? {
                        V2IoCompletion::Failed(reason) => prior_failure = Some(reason),
                        V2IoCompletion::Stopped => {
                            return prior_failure.map_or(Ok(()), Err);
                        }
                        V2IoCompletion::Retired
                        | V2IoCompletion::Signature(_, _)
                        | V2IoCompletion::Stored(_)
                        | V2IoCompletion::Validated(_)
                        | V2IoCompletion::Applied(_)
                        | V2IoCompletion::CertifiedResponse { .. }
                        | V2IoCompletion::CertifiedRequestIgnored
                        | V2IoCompletion::CandidateLoaded(_) => {}
                    }
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err("Sumeragi v2 I/O worker disconnected during shutdown".to_owned());
                }
            }
        }
        loop {
            match self.recv_until(deadline, "shutdown")? {
                V2IoCompletion::Stopped => return prior_failure.map_or(Ok(()), Err),
                V2IoCompletion::Failed(reason) => prior_failure = Some(reason),
                V2IoCompletion::Retired
                | V2IoCompletion::Signature(_, _)
                | V2IoCompletion::Stored(_)
                | V2IoCompletion::Validated(_)
                | V2IoCompletion::Applied(_)
                | V2IoCompletion::CertifiedResponse { .. }
                | V2IoCompletion::CertifiedRequestIgnored
                | V2IoCompletion::CandidateLoaded(_) => {}
            }
        }
    }

    fn request_retirement_until(
        &self,
        receipt: KuraV2CommitReceipt,
        deadline: Instant,
    ) -> Result<(), String> {
        let mut command = V2IoCommand::Retire(receipt);
        loop {
            match self.command_tx.try_send(command) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Full(returned)) => {
                    command = returned;
                    match self.recv_until(deadline, "retirement")? {
                        V2IoCompletion::Failed(reason) => return Err(reason),
                        V2IoCompletion::Stopped => {
                            return Err(
                                "Sumeragi v2 I/O worker stopped before retirement".to_owned()
                            );
                        }
                        V2IoCompletion::Retired
                        | V2IoCompletion::Signature(_, _)
                        | V2IoCompletion::Stored(_)
                        | V2IoCompletion::Validated(_)
                        | V2IoCompletion::Applied(_)
                        | V2IoCompletion::CertifiedResponse { .. }
                        | V2IoCompletion::CertifiedRequestIgnored
                        | V2IoCompletion::CandidateLoaded(_) => {}
                    }
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err("Sumeragi v2 I/O worker disconnected during retirement".to_owned());
                }
            }
        }
        loop {
            match self.recv_until(deadline, "retirement")? {
                V2IoCompletion::Retired => return Ok(()),
                V2IoCompletion::Failed(reason) => return Err(reason),
                V2IoCompletion::Stopped => {
                    return Err("Sumeragi v2 I/O worker stopped before retirement".to_owned());
                }
                V2IoCompletion::Signature(_, _)
                | V2IoCompletion::Stored(_)
                | V2IoCompletion::Validated(_)
                | V2IoCompletion::Applied(_)
                | V2IoCompletion::CertifiedResponse { .. }
                | V2IoCompletion::CertifiedRequestIgnored
                | V2IoCompletion::CandidateLoaded(_) => {}
            }
        }
    }

    fn recv_until(&self, deadline: Instant, operation: &str) -> Result<V2IoCompletion, String> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!(
                "Sumeragi v2 I/O {operation} deadline expired before acknowledgement"
            ));
        }
        match self.completion_rx.recv_timeout(remaining) {
            Ok(completion) => Ok(completion),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
                "Sumeragi v2 I/O {operation} deadline expired before acknowledgement"
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
                "Sumeragi v2 I/O worker disconnected during {operation}"
            )),
        }
    }

    fn finish_control(
        &mut self,
        protocol: Result<(), String>,
        deadline: Instant,
        operation: &str,
    ) -> Result<(), String> {
        if protocol.is_err() {
            // The control command may have failed before reaching the worker.
            // A best-effort shutdown is cancellation, not a second wait: the
            // original wall-clock deadline remains the absolute bound.
            let _ = self.command_tx.try_send(V2IoCommand::Shutdown);
        }
        let joined = self.join_until(deadline, operation);
        match (protocol, joined) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(protocol), Err(join)) => Err(format!(
                "{protocol}; additionally failed to stop worker: {join}"
            )),
        }
    }

    fn join_until(&mut self, deadline: Instant, operation: &str) -> Result<(), String> {
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        loop {
            if join.is_finished() {
                return join
                    .join()
                    .map_err(|_| "Sumeragi v2 I/O worker panicked".to_owned());
            }
            // A completion producer can otherwise be blocked behind a full
            // bounded channel while it is trying to reach the terminal command.
            while self.completion_rx.try_recv().is_ok() {}
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                drop(join);
                return Err(format!(
                    "Sumeragi v2 I/O {operation} deadline expired; detached unfinished worker"
                ));
            }
            thread::park_timeout(remaining.min(V2_IO_JOIN_POLL_INTERVAL));
        }
    }
}

fn control_deadline(timeout: Duration, operation: &str) -> Result<Instant, String> {
    Instant::now().checked_add(timeout).ok_or_else(|| {
        format!("Sumeragi v2 I/O {operation} timeout cannot be represented by the local clock")
    })
}

fn send_completion(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    completion: Result<V2IoCompletion, String>,
) {
    let completion = completion.unwrap_or_else(V2IoCompletion::Failed);
    let _ = sender.send(completion);
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

    /// Canonical `SignedBlockWire` bytes retained by the durable body store.
    pub(crate) fn canonical_wire(&self) -> &[u8] {
        &self.canonical_wire
    }

    /// Consume the completion into exact canonical bytes.
    pub(crate) fn into_canonical_wire(self) -> Vec<u8> {
        self.canonical_wire
    }
}

/// Concrete effect services used by the live v2 height runner.
pub(crate) struct ProductionV2Services {
    context: wire::HeightContext,
    evidence_proofs_of_possession: Vec<Vec<u8>>,
    state: Arc<crate::state::State>,
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
    local_completions: VecDeque<LocalCompletion>,
    pending_candidate_loads: BTreeSet<EventTag>,
    loaded_candidates: VecDeque<LoadedCandidateBody>,
    prepared_candidates: VecDeque<PreparedCandidateBody>,
    validation_rejections: VecDeque<RejectedCandidateBody>,
    outbound_chunks: BTreeMap<HashOf<wire::PayloadManifest>, Vec<wire::ConsensusMessageV2>>,
    entered_view: Option<EventTag>,
    last_status: Option<EffectExecutorStatus>,
    fatal_reason: Option<String>,
}

impl ProductionV2Services {
    /// Start the ordered I/O adapter for one immutable height context.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start(
        context: wire::HeightContext,
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
        npos_config: SumeragiNpos,
        genesis_account: iroha_data_model::account::AccountId,
        events_sender: EventsSender,
        io_queue_capacity: usize,
        orphan_chunk_capacity: usize,
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
        let context_record = V2ContextStore::open(kura.sumeragi_v2_storage_root())
            .map_err(|error| error.to_string())?
            .load(context.height)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                format!(
                    "missing immutable Sumeragi v2 context record for evidence at height {}",
                    context.height
                )
            })?;
        if context_record.context() != &context {
            return Err(
                "immutable Sumeragi v2 evidence context differs from the active context".to_owned(),
            );
        }
        let evidence_proofs_of_possession = context_record.proofs_of_possession().to_vec();
        let apply_service = V2ApplyService::new(
            Arc::clone(&state),
            queue,
            Arc::clone(&kura),
            context.chain_id.clone(),
            block_cadence,
            npos_config,
            genesis_account,
            events_sender,
        );
        let io = V2IoHandle::spawn(
            body_store,
            apply_service,
            context.clone(),
            key_pair.clone(),
            local_validator,
            io_queue_capacity,
        )?;
        Ok(Self {
            context,
            evidence_proofs_of_possession,
            state,
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
            local_completions: VecDeque::new(),
            pending_candidate_loads: BTreeSet::new(),
            loaded_candidates: VecDeque::new(),
            prepared_candidates: VecDeque::new(),
            validation_rejections: VecDeque::new(),
            outbound_chunks: BTreeMap::new(),
            entered_view: None,
            last_status: None,
            fatal_reason: None,
        })
    }

    /// Sign and retain all canonical chunks for proposal and retransmission.
    pub(crate) fn register_outbound_payload(
        &mut self,
        payload: EncodedV2Payload,
    ) -> Result<wire::PayloadManifest, String> {
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
        self.io()?.enqueue(V2IoCommand::Serve(request))
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
        if let Err(error) = self
            .io()?
            .enqueue(V2IoCommand::LoadCandidate { tag, subject })
        {
            self.pending_candidate_loads.remove(&tag);
            return Err(error);
        }
        Ok(())
    }

    /// Take the next locked-subject body loaded by the ordered I/O worker.
    pub(crate) fn take_loaded_candidate(&mut self) -> Option<LoadedCandidateBody> {
        self.loaded_candidates.pop_front()
    }

    /// Take the next deterministic body rejection observed by the worker.
    pub(crate) fn take_validation_rejection(&mut self) -> Option<RejectedCandidateBody> {
        self.validation_rejections.pop_front()
    }

    /// Take the next reducer-authorized local Prepare intent.
    pub(crate) fn take_prepared_candidate(&mut self) -> Option<PreparedCandidateBody> {
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
                V2IoCompletion::Stopped => {
                    return Err(executor.external_service_failed(
                        "unexpected early Sumeragi v2 I/O worker shutdown",
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
    /// Cleanup happens after Kura has made finality irreversible. Failures are
    /// therefore returned as bounded warnings. Cooperative workers are joined;
    /// a worker that misses the absolute deadline is detached with all result
    /// receivers dropped, so callers may continue with the verified successor
    /// height and let restart recovery retry retained files.
    pub(crate) fn finish_height(mut self, receipt: KuraV2CommitReceipt) -> Vec<String> {
        let mut warnings = Vec::new();
        let retirement_completed = match self.io.take() {
            Some(io) => {
                if let Err(error) = io.retire(receipt) {
                    warnings.push(error);
                    false
                } else {
                    true
                }
            }
            None => {
                warnings.push("Sumeragi v2 I/O worker already stopped".to_owned());
                false
            }
        };
        if retirement_completed {
            match std::fs::remove_dir_all(&self.chunk_root) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => warnings.push(error.to_string()),
            }
        }
        warnings
    }

    fn io(&self) -> Result<&V2IoHandle, String> {
        self.io
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())
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
        self.post_block_message(peer, BlockMessage::V2(message));
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
        self.post_block_message(peer, message);
        Ok(())
    }

    /// Send one context-bound Native AMX v2 message to a participant peer.
    pub(crate) fn post_native_amx(
        &self,
        peer: PeerId,
        message: crate::native_amx::NativeAmxMessage,
    ) {
        self.network.post(Post {
            data: NetworkMessage::NativeAmx(Box::new(message)),
            peer_id: peer,
            priority: Priority::High,
        });
    }

    /// Broadcast one merge signature share to every other frozen voter.
    pub(crate) fn broadcast_merge_to_voters(&self, signature: MergeCommitteeSignature) {
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

    fn post_block_message(&self, peer: PeerId, message: BlockMessage) {
        let block_message = Arc::new(message);
        let encoded = Arc::new(BlockMessageWire::encode_message(block_message.as_ref()));
        let data = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::with_encoded(
            block_message,
            encoded,
        )));
        self.network.post(Post {
            data,
            peer_id: peer,
            priority: Priority::High,
        });
    }

    /// Retransmit one v2 transport envelope to every other frozen voter.
    pub(crate) fn broadcast_to_voters(&self, message: wire::ConsensusMessageV2) {
        for entry in &self.context.roster {
            if entry.validator == self.local_peer {
                continue;
            }
            self.post_to_peer(entry.validator.clone(), message.clone());
        }
    }
}

impl Drop for ProductionV2Services {
    fn drop(&mut self) {
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
        self.io()?.enqueue(V2IoCommand::Sign(task))?;
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
        self.broadcast_to_voters(message.clone());
        if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload {
            let manifest_hash = HashOf::new(&proposal.manifest);
            let chunks = self
                .outbound_chunks
                .get(&manifest_hash)
                .ok_or_else(|| "local proposal has no retained Sumeragi v2 chunks".to_owned())?;
            for chunk in chunks {
                self.broadcast_to_voters(chunk.clone());
            }
        }
        Ok(())
    }

    fn sign_body_request(&mut self, preimage: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Signature::try_new(self.key_pair.private_key(), preimage)
            .map(|signature| signature.payload().to_vec())
            .map_err(|error| error.to_string())
    }

    fn enqueue_body_fetch(&mut self, task: BodyFetchTask) -> Result<(), Self::Error> {
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
                        self.post_to_peer(peer.clone(), message.clone());
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
                    self.post_to_peer(peer.clone(), message.clone());
                }
            }
        }
        self.fetches
            .insert(task.id(), FetchSession { task, chunks });
        Ok(())
    }

    fn cancel_body_fetch(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
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
        self.io()?.enqueue(V2IoCommand::Store(task))
    }

    fn cancel_body_store(&mut self, _work_id: EffectWorkId) -> Result<(), Self::Error> {
        Ok(())
    }

    fn enqueue_body_validation(&mut self, task: BodyValidationTask) -> Result<(), Self::Error> {
        self.io()?.enqueue(V2IoCommand::Validate(task))
    }

    fn enqueue_apply(&mut self, task: ApplyTask) -> Result<(), Self::Error> {
        self.io()?.enqueue(V2IoCommand::Apply(task))
    }

    fn entered_view(
        &mut self,
        tag: EventTag,
        _certificate: wire::TimeoutCertificate,
    ) -> Result<(), Self::Error> {
        self.entered_view = Some(tag);
        Ok(())
    }

    fn report_equivocation(
        &mut self,
        evidence: wire::SumeragiV2Equivocation,
    ) -> Result<(), Self::Error> {
        let (signer, round, kind) = match &evidence {
            wire::SumeragiV2Equivocation::Proposal { first, .. } => {
                (first.proposer, first.round, "proposal")
            }
            wire::SumeragiV2Equivocation::PhaseVote { first, .. } => {
                (first.signer, first.round, "phase_vote")
            }
            wire::SumeragiV2Equivocation::TimeoutVote { first, .. } => {
                (first.signer, first.round, "timeout_vote")
            }
        };
        let offender = self
            .context
            .roster
            .get(usize::try_from(signer).unwrap_or(usize::MAX))
            .map(|entry| entry.validator.clone())
            .ok_or_else(|| "Sumeragi v2 evidence signer is outside the frozen roster".to_owned())?;
        let inserted = super::evidence::persist_sumeragi_v2_equivocation(
            self.state.as_ref(),
            &self.context,
            &self.evidence_proofs_of_possession,
            evidence,
        )
        .map_err(|error| error.to_string())?;
        if inserted {
            iroha_logger::warn!(%offender, ?round, kind, "persisted authenticated Sumeragi v2 equivocation evidence");
        } else {
            iroha_logger::debug!(%offender, ?round, kind, "deduplicated replayed Sumeragi v2 equivocation evidence");
        }
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
        self.last_status = Some(status.clone());
        Ok(())
    }

    fn fail_closed(&mut self, reason: &str) {
        self.fatal_reason = Some(reason.to_owned());
        iroha_logger::error!(reason, "Sumeragi v2 effect services failed closed");
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, thread, time::Duration};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{ChainId, block::BlockHeader};

    use super::*;
    use crate::{
        state::{State, World},
        sumeragi::v2_chunks::encode_payload,
    };

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
        let evidence_proofs_of_possession = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("deterministic validator PoP")
            })
            .collect();
        let state = Arc::new(State::new_for_testing(
            World::default(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        ));
        let local_peer = context.roster[0].validator.clone();
        let service = ProductionV2Services {
            context,
            evidence_proofs_of_possession,
            state,
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
            local_completions: VecDeque::new(),
            pending_candidate_loads: BTreeSet::new(),
            loaded_candidates: VecDeque::new(),
            prepared_candidates: VecDeque::new(),
            validation_rejections: VecDeque::new(),
            outbound_chunks: BTreeMap::new(),
            entered_view: None,
            last_status: None,
            fatal_reason: None,
        };
        (service, keys)
    }

    fn retirement_receipt(context: &wire::HeightContext) -> KuraV2CommitReceipt {
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"v2-worker-retirement-block",
            )),
            payload_hash: Hash::new(b"v2-worker-retirement-payload"),
        };
        let commit_qc = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            },
            phase: wire::GlobalPhase::Commit,
            subject,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let artifact =
            wire::finality::V2FinalityArtifact::new(context.clone(), subject, commit_qc, None);
        KuraV2CommitReceipt::for_test(&artifact)
    }

    #[test]
    fn io_retirement_drains_a_full_bounded_queue_and_terminates() {
        let (service, _) = fixture();
        let receipt = retirement_receipt(&service.context);
        let (command_tx, command_rx) = mpsc::sync_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        command_tx
            .send(V2IoCommand::LoadCandidate {
                tag: EventTag::new(
                    service.context.height,
                    0,
                    crate::sumeragi::v2_core::Generation::new(service.context.height),
                ),
                subject: receipt.subject(),
            })
            .expect("prefill bounded command queue");
        let worker = thread::spawn(move || {
            assert!(matches!(
                command_rx.recv(),
                Ok(V2IoCommand::LoadCandidate { .. })
            ));
            completion_tx
                .send(V2IoCompletion::CertifiedRequestIgnored)
                .expect("release retirement producer");
            assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Retire(_))));
            completion_tx
                .send(V2IoCompletion::Retired)
                .expect("acknowledge retirement");
        });
        let handle = V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(worker),
        };
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let _ = result_tx.send(handle.retire(receipt));
        });

        assert_eq!(
            result_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("retirement must have a bounded completion"),
            Ok(())
        );
    }

    #[test]
    fn io_retirement_reports_disconnect_and_worker_panic_without_hanging() {
        let (service, _) = fixture();
        let receipt = retirement_receipt(&service.context);
        let (command_tx, command_rx) = mpsc::sync_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            drop(command_rx);
            panic!("synthetic v2 I/O worker failure");
        });
        while !worker.is_finished() {
            thread::yield_now();
        }
        drop(completion_tx);
        let handle = V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(worker),
        };
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let _ = result_tx.send(handle.retire(receipt));
        });

        let error = result_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("failed retirement must terminate")
            .expect_err("disconnect and panic must be reported");
        assert!(error.contains("disconnected"));
        assert!(error.contains("panicked"));
    }

    #[test]
    fn io_shutdown_deadline_detaches_worker_that_never_consumes() {
        let (command_tx, command_rx) = mpsc::sync_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        command_tx
            .send(V2IoCommand::Shutdown)
            .expect("prefill bounded command queue");
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let _ = release_rx.recv();
            drop(command_rx);
            drop(completion_tx);
        });
        let handle = V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(worker),
        };

        let started = Instant::now();
        let error = handle
            .shutdown_with_timeout(Duration::from_millis(50))
            .expect_err("a non-consuming worker must miss the shutdown deadline");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "shutdown must retain a strict wall-clock bound"
        );
        assert!(error.contains("shutdown deadline expired"));
        assert!(error.contains("detached unfinished worker"));
        release_tx
            .send(())
            .expect("release detached synthetic worker");
    }

    #[test]
    fn io_retirement_deadline_detaches_worker_that_never_acknowledges() {
        let (service, _) = fixture();
        let receipt = retirement_receipt(&service.context);
        let (command_tx, command_rx) = mpsc::sync_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let (consumed_tx, consumed_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Retire(_))));
            consumed_tx
                .send(())
                .expect("report consumed retirement command");
            let _ = release_rx.recv();
            drop(completion_tx);
        });
        let handle = V2IoHandle {
            command_tx,
            completion_rx,
            join: Some(worker),
        };

        let started = Instant::now();
        let error = handle
            .retire_with_timeout(receipt, Duration::from_millis(50))
            .expect_err("an unacknowledged retirement must miss its deadline");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "retirement must retain a strict wall-clock bound"
        );
        consumed_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("synthetic worker must consume retirement before withholding its ack");
        assert!(error.contains("retirement deadline expired"));
        assert!(error.contains("detached unfinished worker"));
        release_tx
            .send(())
            .expect("release detached synthetic worker");
    }

    #[test]
    fn production_equivocation_hook_persists_validated_exact_pair_and_deduplicates_replay() {
        use mv::storage::StorageReadOnly as _;

        let (mut service, keys) = fixture();
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 0,
        };
        let subject = |seed| wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])),
            payload_hash: Hash::prehashed([seed.wrapping_add(1); 32]),
        };
        let signed_vote = |subject| {
            let mut vote = wire::Vote {
                round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                signer: 1,
                signature: Vec::new(),
            };
            vote.signature = Signature::try_new(keys[1].private_key(), &vote.signature_preimage())
                .expect("sign evidence vote")
                .payload()
                .to_vec();
            vote
        };
        let first = signed_vote(subject(0x91));
        let second = signed_vote(subject(0x92));
        service
            .report_equivocation(wire::SumeragiV2Equivocation::PhaseVote {
                first: first.clone(),
                second: second.clone(),
            })
            .expect("persist production evidence");
        service
            .report_equivocation(wire::SumeragiV2Equivocation::PhaseVote {
                first: second.clone(),
                second: first.clone(),
            })
            .expect("deduplicate swapped production replay");
        assert_eq!(
            service.state.world.consensus_evidence.view().iter().count(),
            1
        );

        let mut forged = second;
        forged.signature[0] ^= 0x80;
        assert!(
            service
                .report_equivocation(wire::SumeragiV2Equivocation::PhaseVote {
                    first,
                    second: forged,
                })
                .is_err(),
            "forged evidence must fail before WSV mutation"
        );
        assert_eq!(
            service.state.world.consensus_evidence.view().iter().count(),
            1
        );
    }

    fn manifest_hash(label: &[u8]) -> HashOf<wire::PayloadManifest> {
        HashOf::from_untyped_unchecked(Hash::new(label))
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
            crate::sumeragi::v2_core::Generation::new(service.context.height),
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
        let vote = |phase| wire::Vote {
            round,
            phase,
            subject,
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
