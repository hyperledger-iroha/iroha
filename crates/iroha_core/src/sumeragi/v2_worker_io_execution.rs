fn send_completion(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    admission: &V2IoAdmission,
    completion: Result<V2IoCompletion, String>,
) {
    send_completion_with_lifecycle_ordinal(sender, admission, completion, None);
}
fn send_completion_with_lifecycle_ordinal(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    admission: &V2IoAdmission,
    completion: Result<V2IoCompletion, String>,
    runtime_lifecycle_ordinal: Option<u128>,
) {
    let completion = completion.unwrap_or_else(V2IoCompletion::Failed);
    let _ = send_tracked_completion_with_lifecycle_ordinal(
        sender,
        admission,
        completion,
        runtime_lifecycle_ordinal,
    );
}
fn send_tracked_completion(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    admission: &V2IoAdmission,
    completion: V2IoCompletion,
) -> Result<(), mpsc::SendError<V2IoCompletion>> {
    send_tracked_completion_with_lifecycle_ordinal(sender, admission, completion, None)
}
fn send_tracked_completion_with_lifecycle_ordinal(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    admission: &V2IoAdmission,
    completion: V2IoCompletion,
    runtime_lifecycle_ordinal: Option<u128>,
) -> Result<(), mpsc::SendError<V2IoCompletion>> {
    let lifecycle_decision_apply = completion.lifecycle_decision_apply_key();
    let recovered_lifecycle_sign = completion.recovered_lifecycle_sign_key();
    let recovered_decision_fetch = completion.recovered_decision_fetch_key();
    let lifecycle_validate = completion.lifecycle_validate_key();
    let lifecycle_certified_serve = completion.lifecycle_certified_serve_ordinal();
    admission.retain_completion(
        Instant::now(),
        completion.requires_runtime_capacity(),
        runtime_lifecycle_ordinal,
        lifecycle_decision_apply,
        recovered_lifecycle_sign,
        recovered_decision_fetch,
        lifecycle_validate,
        lifecycle_certified_serve,
    );
    sender.send(completion).inspect_err(|_| {
        admission.abandon_latest_completion();
    })
}
fn try_send_tracked_completion(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    admission: &V2IoAdmission,
    completion: V2IoCompletion,
) -> Result<(), mpsc::TrySendError<V2IoCompletion>> {
    try_send_tracked_completion_with_lifecycle_ordinal(sender, admission, completion, None)
}
fn try_send_tracked_completion_with_lifecycle_ordinal(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    admission: &V2IoAdmission,
    completion: V2IoCompletion,
    runtime_lifecycle_ordinal: Option<u128>,
) -> Result<(), mpsc::TrySendError<V2IoCompletion>> {
    let lifecycle_decision_apply = completion.lifecycle_decision_apply_key();
    let recovered_lifecycle_sign = completion.recovered_lifecycle_sign_key();
    let recovered_decision_fetch = completion.recovered_decision_fetch_key();
    let lifecycle_validate = completion.lifecycle_validate_key();
    let lifecycle_certified_serve = completion.lifecycle_certified_serve_ordinal();
    admission.retain_completion(
        Instant::now(),
        completion.requires_runtime_capacity(),
        runtime_lifecycle_ordinal,
        lifecycle_decision_apply,
        recovered_lifecycle_sign,
        recovered_decision_fetch,
        lifecycle_validate,
        lifecycle_certified_serve,
    );
    sender.try_send(completion).inspect_err(|_| {
        admission.abandon_latest_completion();
    })
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
            // Log before closing output. The retained relay exits the process
            // as soon as it observes the closed guard, so logging after this
            // drop races with `process::exit` and can erase the only precise
            // failure diagnostic.
            iroha_logger::error!(reason, "Sumeragi v2 I/O command failed closed");
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
    // Zero remains a useful non-blocking poll for an already-buffered completion.
    let remaining = deadline.saturating_duration_since(Instant::now());
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
fn sign_recovered_lifecycle_task(
    body_store: &V2BodyStore,
    context: &wire::HeightContext,
    key_pair: &KeyPair,
    task: RecoveredLifecycleSignTaskV1,
) -> Result<RecoveredLifecycleSignWorkerResultV1, String> {
    let (preimage, outbound_payload) = match &task.request {
        super::v2::SignRequest::Proposal(proposal) => (
            proposal.signature_preimage(),
            Some(recover_outbound_proposal_payload(
                body_store, context, proposal,
            )?),
        ),
        super::v2::SignRequest::Vote(vote) => (vote.signature_preimage(), None),
        super::v2::SignRequest::TimeoutVote(vote) => (vote.signature_preimage(), None),
    };
    Signature::try_new(key_pair.private_key(), &preimage)
        .map(|signature| RecoveredLifecycleSignWorkerResultV1 {
            task,
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
fn serve_lifecycle_certified_body(
    body_store: &V2BodyStore,
    key_pair: &KeyPair,
    local_validator: Option<wire::ValidatorIndex>,
    task: LifecycleCertifiedServeTaskV1,
) -> Result<LifecycleCertifiedServeWorkerResultV1, String> {
    let (durable_body, response) =
        build_certified_body_response(body_store, key_pair, local_validator, &task.authenticated)?;
    let body_readback = body_store
        .read_durable_body_for_certified_serve(&durable_body)
        .map_err(|error| error.to_string())?;
    if body_readback.canonical_wire() != response.body.as_slice() {
        return Err("Certified-Serve response changed after durable body readback".to_owned());
    }
    Ok(LifecycleCertifiedServeWorkerResultV1 {
        task,
        body_readback: Some(body_readback),
        response,
    })
}
fn build_certified_body_response(
    body_store: &V2BodyStore,
    key_pair: &KeyPair,
    local_validator: Option<wire::ValidatorIndex>,
    authenticated: &AuthenticatedCertifiedBodyRequest,
) -> Result<(DurableBodyReceipt, wire::CertifiedBodyResponse), String> {
    let request = authenticated.request();
    let Some(responder) = local_validator else {
        return Err("local observer crossed certified-body Serve admission".to_owned());
    };
    if request
        .certificate
        .signers
        .binary_search(&responder)
        .is_err()
    {
        return Err(
            "local validator crossed certified-body Serve admission without retention authority"
                .to_owned(),
        );
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
    Ok((receipt, response))
}
fn load_candidate_body(
    body_store: &V2BodyStore,
    acquisition_id: LockedCandidateAcquisitionId,
    subject: wire::BlockSubject,
) -> Result<Option<LockedCandidateLoad>, String> {
    let Some((_, receipt)) = body_store
        .latest_for_subject(subject)
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let canonical_wire = body_store
        .load_canonical_wire(&receipt)
        .map_err(|error| error.to_string())?;
    let decoded = decode_framed_signed_block(&canonical_wire).map_err(|error| error.to_string())?;
    if !decoded.is_resultless_proposal() {
        return Err("locked Sumeragi v2 body must be resultless".to_owned());
    }
    let loaded_subject = wire::BlockSubject {
        parent_block_hash: decoded.header().prev_block_hash(),
        block_hash: decoded.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    if loaded_subject != subject {
        return Err("locked Sumeragi v2 durable body does not match its subject".to_owned());
    }
    Ok(Some(LockedCandidateLoad {
        acquisition_id,
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
    ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
}
// A lifecycle classification revalidates the complete fair-ingress carrier and
// scans the executor's exact body stages. Limit that adversarially expensive
// work to one orphan per service turn; the persistent cursor below still gives
// every retained orphan deterministic round-robin progress.
const MAX_ORPHAN_LIFECYCLE_VISITS_PER_REPLAY: usize = 1;
#[derive(Clone, Copy, Debug)]
struct OrphanPayloadLifecycleSweepCursor {
    manifest_hash: HashOf<wire::PayloadManifest>,
    chunk_offset: usize,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OrphanPayloadChunkBufferResult {
    Disposition(PayloadChunkDisposition),
    /// A productive runtime owner could not be retained without replacing a
    /// different productive owner. The caller must fail closed; silently
    /// dropping or terminalizing it would suppress the canonical retry.
    ProductiveRetentionConflict,
}
impl OrphanPayloadChunkBufferResult {
    #[cfg(test)]
    const fn public_disposition(self) -> PayloadChunkDisposition {
        match self {
            Self::Disposition(disposition) => disposition,
            Self::ProductiveRetentionConflict => PayloadChunkDisposition::Rejected,
        }
    }
}
#[derive(Clone)]
enum LocalCompletion {
    Reconstructed {
        task: BodyFetchTask,
        manifest: wire::PayloadManifest,
        body: Arc<[u8]>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyFetchServiceOwner {
    None,
    Live,
    Reconstructed(usize),
}
/// Service-owner removal frozen under an exclusive borrow until late-response
/// commit atomically joins its claim, queue CAS, reservation, swap, and wake.
pub(in crate::sumeragi) struct PreparedCertifiedBodyFetchOwnerRemoval<'a> {
    services: &'a mut ProductionV2Services,
    task: BodyFetchTask,
    owner: BodyFetchServiceOwner,
}
impl PreparedCertifiedBodyFetchOwnerRemoval<'_> {
    pub(in crate::sumeragi) fn commit(self, permit: &ConsensusOutputPermit<'_>) {
        assert!(
            permit.authorizes(self.services.output_guard.as_ref()),
            "certified body-fetch removal requires this service's live output permit"
        );
        self.services
            .commit_exact_body_fetch_owner_removal(&self.task, self.owner);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionSource {
    Io,
    Local,
}
enum PendingServiceCompletion {
    Io {
        completion: V2IoCompletion,
        ownership_position: usize,
    },
    Local(LocalCompletion),
}
struct IoCompletionTake {
    completion: Option<PendingServiceCompletion>,
    retained_runtime: bool,
}
impl IoCompletionTake {
    fn ready(completion: PendingServiceCompletion) -> Self {
        Self {
            completion: Some(completion),
            retained_runtime: false,
        }
    }
    const fn retained_runtime() -> Self {
        Self {
            completion: None,
            retained_runtime: true,
        }
    }
    const fn unavailable() -> Self {
        Self {
            completion: None,
            retained_runtime: false,
        }
    }
}
const MAX_COMPLETION_DRAIN_BATCH: usize = 256;
/// Exact durable bytes loaded for a locked-subject re-proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadedCandidateBody {
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    canonical_wire: Vec<u8>,
}
/// Physical result of one immutable locked-subject disk acquisition.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LockedCandidateLoad {
    acquisition_id: LockedCandidateAcquisitionId,
    subject: wire::BlockSubject,
    canonical_wire: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LockedCandidateAcquisitionId(u64);
#[derive(Clone, Debug, PartialEq, Eq)]
enum LockedCandidateAcquisitionState {
    Loading {
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    },
    Ready {
        acquisition_id: LockedCandidateAcquisitionId,
        canonical_wire: Vec<u8>,
        delivered_to: Option<(wire::ConsensusRound, EventTag)>,
    },
    Waiting {
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LockedCandidateRebind {
    Unchanged,
    ConsumerAdvanced,
    ReplacementDeferred,
    ReplacementRequired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LockedCandidateCompletion {
    Ready(EventTag),
    Stale,
    Waiting,
    ReplacementRequired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LockedCandidatePhysicalOwner {
    Desired(LockedCandidateAcquisitionId),
    Stale,
    Superseded,
}
/// Height-scoped durable-lock owner whose immutable subject permits the same
/// bounded ready body to rebind without another disk read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LockedCandidateAcquisition {
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    consumer: EventTag,
    state: LockedCandidateAcquisitionState,
}
impl LockedCandidateAcquisition {
    const fn loading(
        acquisition_id: LockedCandidateAcquisitionId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        consumer: EventTag,
    ) -> Self {
        Self {
            round,
            subject,
            consumer,
            state: LockedCandidateAcquisitionState::Loading {
                acquisition_id,
                subject,
            },
        }
    }
    fn rebind_consumer(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        consumer: EventTag,
    ) -> Result<LockedCandidateRebind, String> {
        if round.context_id != self.round.context_id || round.height != self.round.height {
            return Err("Sumeragi v2 locked-body acquisition changed height context".to_owned());
        }
        let same_consumer = consumer == self.consumer;
        if !same_consumer && !consumer.strictly_advances(self.consumer) {
            return Err(
                "Sumeragi v2 locked-body acquisition consumer did not advance monotonically"
                    .to_owned(),
            );
        }
        if round.view < self.round.view {
            return Err("Sumeragi v2 locked-body acquisition lock rank regressed".to_owned());
        }
        if same_consumer && round == self.round {
            return if subject == self.subject {
                Ok(LockedCandidateRebind::Unchanged)
            } else {
                Err(
                    "Sumeragi v2 locked-body acquisition changed subject without a higher lock"
                        .to_owned(),
                )
            };
        }
        if subject != self.subject && round.view <= self.round.view {
            return Err(
                "Sumeragi v2 locked-body acquisition changed subject without a higher lock"
                    .to_owned(),
            );
        }
        let replacing_subject = subject != self.subject;
        self.round = round;
        self.subject = subject;
        self.consumer = consumer;
        if !replacing_subject {
            return Ok(LockedCandidateRebind::ConsumerAdvanced);
        }
        Ok(match &self.state {
            LockedCandidateAcquisitionState::Loading { .. } => {
                LockedCandidateRebind::ReplacementDeferred
            }
            LockedCandidateAcquisitionState::Ready { .. }
            | LockedCandidateAcquisitionState::Waiting { .. } => {
                LockedCandidateRebind::ReplacementRequired
            }
        })
    }
    fn start_replacement(&mut self, acquisition_id: LockedCandidateAcquisitionId) {
        self.state = LockedCandidateAcquisitionState::Loading {
            acquisition_id,
            subject: self.subject,
        };
    }
    fn physical_owner(
        &self,
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    ) -> Result<LockedCandidatePhysicalOwner, String> {
        let (owned_id, owned_subject, terminal) = match &self.state {
            LockedCandidateAcquisitionState::Loading {
                acquisition_id,
                subject,
            } => (*acquisition_id, *subject, false),
            LockedCandidateAcquisitionState::Ready { acquisition_id, .. } => {
                (*acquisition_id, self.subject, true)
            }
            LockedCandidateAcquisitionState::Waiting {
                acquisition_id,
                subject,
            } => (*acquisition_id, *subject, true),
        };
        if acquisition_id < owned_id {
            return Ok(LockedCandidatePhysicalOwner::Stale);
        }
        if acquisition_id > owned_id {
            return Err(
                "Sumeragi v2 locked-body completion has an unknown future acquisition ID"
                    .to_owned(),
            );
        }
        if terminal {
            return Err("Sumeragi v2 locked-body acquisition completed more than once".to_owned());
        }
        if subject != owned_subject {
            return Err(
                "Sumeragi v2 locked-body completion has a different acquisition subject".to_owned(),
            );
        }
        if owned_subject != self.subject {
            return Ok(LockedCandidatePhysicalOwner::Superseded);
        }
        Ok(LockedCandidatePhysicalOwner::Desired(owned_id))
    }
    fn complete(
        &mut self,
        loaded: LockedCandidateLoad,
    ) -> Result<LockedCandidateCompletion, String> {
        let owned_id = match self.physical_owner(loaded.acquisition_id, loaded.subject)? {
            LockedCandidatePhysicalOwner::Stale => {
                return Ok(LockedCandidateCompletion::Stale);
            }
            LockedCandidatePhysicalOwner::Superseded => {
                return Ok(LockedCandidateCompletion::ReplacementRequired);
            }
            LockedCandidatePhysicalOwner::Desired(owned_id) => owned_id,
        };
        self.state = LockedCandidateAcquisitionState::Ready {
            acquisition_id: owned_id,
            canonical_wire: loaded.canonical_wire,
            delivered_to: None,
        };
        Ok(LockedCandidateCompletion::Ready(self.consumer))
    }
    fn unavailable(
        &mut self,
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    ) -> Result<LockedCandidateCompletion, String> {
        match self.physical_owner(acquisition_id, subject)? {
            LockedCandidatePhysicalOwner::Stale => Ok(LockedCandidateCompletion::Stale),
            LockedCandidatePhysicalOwner::Superseded => {
                Ok(LockedCandidateCompletion::ReplacementRequired)
            }
            LockedCandidatePhysicalOwner::Desired(acquisition_id) => {
                self.state = LockedCandidateAcquisitionState::Waiting {
                    acquisition_id,
                    subject,
                };
                Ok(LockedCandidateCompletion::Waiting)
            }
        }
    }
    fn failed(
        &self,
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    ) -> Result<LockedCandidateCompletion, String> {
        match self.physical_owner(acquisition_id, subject)? {
            LockedCandidatePhysicalOwner::Stale => Ok(LockedCandidateCompletion::Stale),
            LockedCandidatePhysicalOwner::Superseded => {
                Ok(LockedCandidateCompletion::ReplacementRequired)
            }
            LockedCandidatePhysicalOwner::Desired(_) => {
                Err("active Sumeragi v2 locked-body acquisition failed durable loading".to_owned())
            }
        }
    }
    fn pending_count(&self) -> usize {
        match &self.state {
            LockedCandidateAcquisitionState::Loading { .. }
            | LockedCandidateAcquisitionState::Waiting { .. } => 1,
            LockedCandidateAcquisitionState::Ready { delivered_to, .. } => {
                usize::from(*delivered_to != Some((self.round, self.consumer)))
            }
        }
    }
    fn take_ready(&mut self) -> Option<LoadedCandidateBody> {
        let LockedCandidateAcquisitionState::Ready {
            canonical_wire,
            delivered_to,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if *delivered_to == Some((self.round, self.consumer)) {
            return None;
        }
        *delivered_to = Some((self.round, self.consumer));
        Some(LoadedCandidateBody {
            tag: self.consumer,
            round: self.round,
            subject: self.subject,
            canonical_wire: canonical_wire.clone(),
        })
    }
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
impl LoadedCandidateBody {
    /// Reducer incarnation which requested the load.
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }
    /// Exact durable Prepare round which owns this delivery.
    pub(crate) const fn round(&self) -> wire::ConsensusRound {
        self.round
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
#[derive(Clone, Debug, PartialEq, Eq)]
struct RetainedOutboundPayload {
    owner: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    messages: Vec<wire::ConsensusMessageV2>,
}
/// Compact semantic fanout owning one message, unique peers, per-peer retry
/// lanes, and only each recoverable admission's current [`Post`] and ticket.
#[derive(Clone, Debug, Default)]
enum ExactTargetRoute {
    /// Resolve the target through the actor-published direct topology.
    #[default]
    Topology,
    /// Return a response through the exact authenticated request tenure.
    Reply(NetworkReplyRoute),
}
type ExactOutputClass = ReliableProgressClass;
type ExactOutputClassMask = u8;
type ExactFanoutFifoId = u64;
const EXACT_OUTPUT_CLASSES: [ExactOutputClass; V2_EXACT_OUTPUT_CLASS_COUNT] = [
    ExactOutputClass::Safety,
    ExactOutputClass::Lane,
    ExactOutputClass::Bulk,
];
const ATOMIC_PROPOSAL_FANOUT_COUNT: usize = 2;
const fn exact_output_class_bit(class: ExactOutputClass) -> ExactOutputClassMask {
    match class {
        ExactOutputClass::Safety => 1 << 0,
        ExactOutputClass::Lane => 1 << 1,
        ExactOutputClass::Bulk => 1 << 2,
    }
}
const fn exact_output_class_priority(class: ExactOutputClass) -> u8 {
    match class {
        ExactOutputClass::Safety => 3,
        ExactOutputClass::Lane => 2,
        ExactOutputClass::Bulk => 1,
    }
}
fn exact_output_classes(mask: ExactOutputClassMask) -> impl Iterator<Item = ExactOutputClass> {
    EXACT_OUTPUT_CLASSES
        .into_iter()
        .filter(move |class| mask & exact_output_class_bit(*class) != 0)
}
fn validate_shared_ownership_geometry(
    shared_ownership_unit_capacity: usize,
    max_reply_sources_per_request: usize,
) -> Result<(), String> {
    validate_sumeragi_v2_exact_output_geometry(
        shared_ownership_unit_capacity,
        max_reply_sources_per_request,
    )
    .map_err(|error| error.to_string())
}
fn exact_output_class(message: &NetworkMessage) -> Result<ExactOutputClass, String> {
    let topic = message.topic();
    reliable_progress_class(topic, message.subscriber_route()).ok_or_else(|| {
        format!("Sumeragi v2 exact output has no reliable progress class: {topic:?}")
    })
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ExactTargetAuthority {
    Topology(PeerId),
    Reply(NetworkReplySourceKey),
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ExactTargetSource {
    authority: ExactTargetAuthority,
    class: ExactOutputClass,
}
/// Bounded target/class/kind ownership unit. FIFO follows authenticated source;
/// reservation follows frozen semantic targets, with distinct fanout-level
/// topology-progress and reproducible responder-control credits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ExactTargetReservationKind {
    Reliable,
    /// One topology-routed timeout vote/certificate can escape ordinary
    /// Safety-class backlog and certify the view which retires that backlog.
    Pacemaker,
    SidecarTopologyProgress,
    SidecarReplyControl,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ExactTargetReservation {
    semantic_target: PeerId,
    class: ExactOutputClass,
    kind: ExactTargetReservationKind,
}
impl ExactTargetRoute {
    fn source(&self, semantic_peer: &PeerId, class: ExactOutputClass) -> ExactTargetSource {
        let authority = match self {
            Self::Topology => ExactTargetAuthority::Topology(semantic_peer.clone()),
            Self::Reply(route) => ExactTargetAuthority::Reply(route.source_key()),
        };
        ExactTargetSource { authority, class }
    }
}
#[derive(Debug)]
struct PendingExactReplyFlush {
    flush_ack: NetworkReplyFlushAck,
    /// Immutable adaptive-timeout generation admitted with this exact writer
    /// occurrence. The mutable target generation must remain equal until the
    /// terminal receipt is consumed or finality supersedes volatile output.
    reply_writer_timeout_attempt: u8,
    /// Sidecar chunks retain their process-local lane admission receipt beside
    /// the exact writer occurrence. Ordinary reliable replies leave this
    /// empty, but both kinds keep the same target cursor until writer flush.
    sidecar_admission: Option<CertifiedMergeSidecarChunkAdmission>,
}
#[derive(Debug, Default)]
struct PendingExactTarget {
    route: ExactTargetRoute,
    message_index: usize,
    /// Bounded adaptive writer-timeout generation for this semantic item.
    reply_writer_timeout_attempt: u8,
    current: Option<Post<NetworkMessage>>,
    ticket: Option<NetworkActorAdmissionTicket>,
    /// Exact actor-owned reply occurrence awaiting its peer writer's complete
    /// write and flush. The semantic cursor cannot advance while this exists.
    pending_flush: Option<PendingExactReplyFlush>,
    /// Mark the source unavailable while retaining payload, cursor, age, FIFO,
    /// and reservation ownership until authenticated reconnect.
    parked: bool,
}
impl PendingExactTarget {
    /// Commit one already-preflighted authenticated-source update.
    fn apply_reply_route_update(
        &mut self,
        candidate: &NetworkReplyRoute,
        update: NetworkReplyRouteSourceUpdate,
    ) {
        debug_assert!(matches!(self.route, ExactTargetRoute::Reply(_)));
        match update {
            NetworkReplyRouteSourceUpdate::Exact => {}
            NetworkReplyRouteSourceUpdate::LaterDelivery => {
                // Admission tickets are bound to connection tenure and the
                // canonical payload, not to a local delivery ordinal.
                self.route = ExactTargetRoute::Reply(candidate.clone());
            }
            NetworkReplyRouteSourceUpdate::Reconnected => {
                // Admission state belongs to the retired connection tenure,
                // but the semantic request's exact-output cursor belongs to
                // this authenticated source attempt. Retry the current item
                // through the replacement writer without regressing rank.
                self.current = None;
                self.ticket = None;
                self.parked = false;
                self.route = ExactTargetRoute::Reply(candidate.clone());
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExactOutputCreationScope {
    context_id: wire::HeightContextId,
    height: wire::Height,
}
impl ExactOutputCreationScope {
    fn covers(self, artifact: &wire::finality::V2FinalityArtifact) -> bool {
        self.context_id == artifact.context_id() && self.height == artifact.height
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct CertifiedSidecarTransferIdentity {
    service_generation: crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    request_id: Hash,
    entry_hash: HashOf<iroha_data_model::merge::MergeLedgerEntry>,
    encoded_len: u64,
    epoch_id: u64,
    reference_digest: Hash,
    requester: PeerId,
    responder: PeerId,
}
impl CertifiedSidecarTransferIdentity {
    fn from_request(request: &CertifiedMergeSidecarRequestV1) -> Self {
        Self {
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            semantic_sequence: request.semantic_sequence,
            request_id: request.request_id,
            entry_hash: request.entry_hash,
            encoded_len: request.encoded_len,
            epoch_id: request.epoch_id,
            reference_digest: request.reference_digest,
            requester: request.requester.clone(),
            responder: request.responder.clone(),
        }
    }
    fn from_chunk(chunk: &CertifiedMergeSidecarChunkV1) -> Self {
        Self {
            service_generation: chunk.service_generation,
            stream_epoch: chunk.stream_epoch,
            semantic_sequence: chunk.semantic_sequence,
            request_id: chunk.request_id,
            entry_hash: chunk.entry_hash,
            encoded_len: chunk.encoded_len,
            epoch_id: chunk.epoch_id,
            reference_digest: chunk.reference_digest,
            requester: chunk.requester.clone(),
            responder: chunk.responder.clone(),
        }
    }
}
include!("v2_worker/exact_output_rollover_claim.rs");
include!("v2_worker/queue_plan_admission_handoff.rs");
include!("v2_worker/exact_output_pending_state.rs");
#[derive(Debug)]
struct PendingExactFanout {
    messages: Vec<NetworkMessage>,
    message_hashes: Vec<HashOf<NetworkMessage>>,
    /// Reliable class for each immutable message occurrence.
    message_classes: Vec<ExactOutputClass>,
    /// Three-bit reliable-class mask for each message suffix, including the empty suffix.
    message_class_suffixes: Vec<ExactOutputClassMask>,
    peers: Vec<PeerId>,
    targets: Vec<PendingExactTarget>,
    /// Bounded live attempts and retired-delivery tombstones for a reply fanout.
    ///
    /// Targets retain independent cursors, while this set remains the
    /// authoritative capability history across pruning and coalescing.
    reply_routes: Option<NetworkReplyRoutes>,
    /// Exact fair-ingress owner whose immutable request materialized this
    /// reply fanout. It is merged and pruned atomically with `reply_routes`.
    ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
    /// Current per-source target positions; the first position is the local FIFO head.
    current_source_targets: BTreeMap<ExactTargetSource, BTreeSet<usize>>,
    next_target_index: usize,
    /// Stable enqueue order used by the global per-source FIFO index.
    fifo_id: Option<ExactFanoutFifoId>,
    rollover_claim: ExactOutputRolloverClaim,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplyTargetMerge {
    Park {
        prior_index: usize,
    },
    Update {
        prior_index: usize,
        candidate_index: usize,
        update: NetworkReplyRouteSourceUpdate,
    },
    Append {
        candidate_index: usize,
    },
}
#[derive(Debug)]
struct ReplyTargetMergePlan {
    targets: Vec<ReplyTargetMerge>,
    reply_routes: NetworkReplyRoutes,
    ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
}
enum ReplyRouteMergeReceipt {
    Strict(NetworkReplyRoutesStrictMergeReceipt),
    Superseded(NetworkReplyRoutesObservedMergeReceipt),
}
#[derive(Debug)]
struct ReplyTargetMergePreview {
    current_source_targets: BTreeMap<ExactTargetSource, BTreeSet<usize>>,
    outstanding_sources: BTreeSet<ExactTargetSource>,
}
#[derive(Debug)]
struct ResponderControlReplacementPlan {
    retained_index: usize,
    replacement_fifo_id: ExactFanoutFifoId,
    next_fanout_fifo_id: ExactFanoutFifoId,
    next_fanout_index: usize,
    source_fifo_owners: BTreeMap<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>,
    reservation_owner_counts: BTreeMap<ExactTargetReservation, usize>,
    ownership_units: usize,
    shared_ownership_units: usize,
}
impl PendingExactFanout {
    fn semantic_peers(&self) -> Vec<PeerId> {
        let mut seen = BTreeSet::new();
        self.peers
            .iter()
            .filter(|peer| seen.insert((*peer).clone()))
            .cloned()
            .collect()
    }
    #[cfg(test)]
    fn new(messages: Vec<NetworkMessage>, peers: Vec<PeerId>) -> Option<Self> {
        let routes = vec![ExactTargetRoute::Topology; peers.len()];
        Self::new_with_routes(messages, peers, routes)
    }
    #[cfg(test)]
    fn new_with_routes(
        messages: Vec<NetworkMessage>,
        peers: Vec<PeerId>,
        routes: Vec<ExactTargetRoute>,
    ) -> Option<Self> {
        Self::classified_with_routes(messages, peers, routes)
            .ok()
            .flatten()
    }
    #[cfg(test)]
    fn new_with_reply_routes(
        messages: Vec<NetworkMessage>,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
    ) -> Option<Self> {
        Self::classified_with_reply_routes(messages, peer, reply_routes)
            .ok()
            .flatten()
    }
    fn synthesized_reply_routes(routes: &[ExactTargetRoute]) -> Option<NetworkReplyRoutes> {
        let mut history: Option<NetworkReplyRoutes> = None;
        for route in routes {
            let ExactTargetRoute::Reply(route) = route else {
                return None;
            };
            let singleton = NetworkReplyRoutes::try_from_route(route.clone()).ok()?;
            if let Some(history) = history.as_mut() {
                history.merge(&singleton).ok()?;
            } else {
                history = Some(singleton);
            }
        }
        history
    }
    fn classified_with_routes(
        messages: Vec<NetworkMessage>,
        peers: Vec<PeerId>,
        routes: Vec<ExactTargetRoute>,
    ) -> Result<Option<Self>, String> {
        let reply_routes = Self::synthesized_reply_routes(&routes);
        Self::classified_with_route_history(messages, peers, routes, reply_routes)
    }
    fn classified_with_reply_routes(
        messages: Vec<NetworkMessage>,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
    ) -> Result<Option<Self>, String> {
        if reply_routes.semantic_target() != &peer || reply_routes.is_empty() {
            return Err(
                "Sumeragi v2 exact-output reply history changed target geometry".to_owned(),
            );
        }
        let routes = reply_routes
            .iter()
            .cloned()
            .map(ExactTargetRoute::Reply)
            .collect::<Vec<_>>();
        let peers = vec![peer; routes.len()];
        Self::classified_with_route_history(messages, peers, routes, Some(reply_routes))
    }
    fn classified_with_route_history(
        messages: Vec<NetworkMessage>,
        peers: Vec<PeerId>,
        routes: Vec<ExactTargetRoute>,
        reply_routes: Option<NetworkReplyRoutes>,
    ) -> Result<Option<Self>, String> {
        if messages.is_empty() || peers.is_empty() {
            return Ok(None);
        }
        if routes.len() != peers.len() {
            return Err("Sumeragi v2 exact-output route count changed target geometry".to_owned());
        }
        let message_classes = messages
            .iter()
            .map(exact_output_class)
            .collect::<Result<Vec<_>, _>>()?;
        if message_classes.windows(2).any(|classes| {
            exact_output_class_priority(classes[0]) < exact_output_class_priority(classes[1])
        }) {
            return Err(
                "Sumeragi v2 exact-output fanout raises priority after an earlier message"
                    .to_owned(),
            );
        }
        let mut message_class_suffixes = vec![0; message_classes.len() + 1];
        for message_index in (0..message_classes.len()).rev() {
            message_class_suffixes[message_index] = message_class_suffixes[message_index + 1]
                | exact_output_class_bit(message_classes[message_index]);
        }
        let message_hashes = messages.iter().map(HashOf::new).collect();
        let targets = routes
            .into_iter()
            .map(|route| PendingExactTarget {
                route,
                ..PendingExactTarget::default()
            })
            .collect();
        let mut fanout = Self {
            messages,
            message_hashes,
            message_classes,
            message_class_suffixes,
            peers,
            targets,
            reply_routes,
            ingress_ownership: None,
            current_source_targets: BTreeMap::new(),
            next_target_index: 0,
            fifo_id: None,
            rollover_claim: ExactOutputRolloverClaim::Exact,
        };
        fanout.rebuild_current_source_targets()?;
        Ok(Some(fanout))
    }
    fn claimed(
        messages: Vec<NetworkMessage>,
        peers: Vec<PeerId>,
        rollover_claim: ExactOutputRolloverClaim,
    ) -> Result<Option<Self>, String> {
        let routes = vec![ExactTargetRoute::Topology; peers.len()];
        Self::claimed_with_routes(messages, peers, routes, rollover_claim)
    }
    fn claimed_with_routes(
        messages: Vec<NetworkMessage>,
        peers: Vec<PeerId>,
        routes: Vec<ExactTargetRoute>,
        rollover_claim: ExactOutputRolloverClaim,
    ) -> Result<Option<Self>, String> {
        let Some(mut fanout) = Self::classified_with_routes(messages, peers, routes)? else {
            return Ok(None);
        };
        rollover_claim.validate_fanout(&fanout.messages, &fanout.semantic_peers())?;
        fanout.rollover_claim = rollover_claim;
        Ok(Some(fanout))
    }
    fn claimed_with_reply_routes(
        messages: Vec<NetworkMessage>,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        rollover_claim: ExactOutputRolloverClaim,
    ) -> Result<Option<Self>, String> {
        let Some(mut fanout) = Self::classified_with_reply_routes(messages, peer, reply_routes)?
        else {
            return Ok(None);
        };
        rollover_claim.validate_fanout(&fanout.messages, &fanout.semantic_peers())?;
        fanout.rollover_claim = rollover_claim;
        Ok(Some(fanout))
    }
    fn claimed_with_reply_routes_and_ingress_ownership(
        messages: Vec<NetworkMessage>,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
        rollover_claim: ExactOutputRolloverClaim,
    ) -> Result<Option<Self>, String> {
        let Some(mut fanout) =
            Self::claimed_with_reply_routes(messages, peer, reply_routes, rollover_claim)?
        else {
            return Ok(None);
        };
        if let Some(ownership) = ingress_ownership {
            let routes = fanout.reply_routes.as_ref().ok_or_else(|| {
                "Sumeragi v2 ingress-owned reply lost its bounded route history".to_owned()
            })?;
            if !ownership.validate_exact() || !ownership.matches_reply_routes(Some(routes)) {
                return Err("Sumeragi v2 reply carried altered fair-ingress ownership".to_owned());
            }
            fanout.ingress_ownership = Some(ownership);
        }
        Ok(Some(fanout))
    }
    fn take_attempt(
        &mut self,
        target_index: usize,
    ) -> Option<(
        Post<NetworkMessage>,
        Option<NetworkActorAdmissionTicket>,
        ExactTargetRoute,
        u8,
    )> {
        let target = self.targets.get_mut(target_index)?;
        if target.parked || target.pending_flush.is_some() {
            return None;
        }
        if let Some(post) = target.current.take() {
            return Some((
                post,
                target.ticket.take(),
                target.route.clone(),
                target.reply_writer_timeout_attempt,
            ));
        }
        let data = self.messages.get(target.message_index)?.clone();
        let peer_id = self.peers.get(target_index)?.clone();
        Some((
            Post {
                data,
                peer_id,
                priority: Priority::High,
            },
            None,
            target.route.clone(),
            target.reply_writer_timeout_attempt,
        ))
    }
    fn expected_current_source_targets(
        &self,
    ) -> Result<BTreeMap<ExactTargetSource, BTreeSet<usize>>, String> {
        self.expected_current_source_targets_excluding(None)
    }
    fn expected_current_source_targets_excluding(
        &self,
        excluded_target: Option<usize>,
    ) -> Result<BTreeMap<ExactTargetSource, BTreeSet<usize>>, String> {
        let mut expected = BTreeMap::<ExactTargetSource, BTreeSet<usize>>::new();
        for target_index in 0..self.targets.len() {
            if excluded_target == Some(target_index) || self.target_is_complete(target_index) {
                continue;
            }
            expected
                .entry(self.current_target_source(target_index)?)
                .or_default()
                .insert(target_index);
        }
        Ok(expected)
    }
    fn rebuild_current_source_targets(&mut self) -> Result<(), String> {
        self.current_source_targets = self.expected_current_source_targets()?;
        Ok(())
    }
    /// Transfer owned lane work while pruning retired source occurrences and
    /// preserving live siblings; fresh inactive capabilities remain rejected.
    fn retain_active_unowned_reply_targets(&mut self) -> Result<usize, String> {
        if self.fifo_id.is_some()
            || self.targets.iter().any(|target| {
                target.current.is_some()
                    || target.ticket.is_some()
                    || target.pending_flush.is_some()
            })
        {
            return Err(
                "Sumeragi v2 cannot prune reply routes after exact-output ownership".to_owned(),
            );
        }
        if self.targets.len() != self.peers.len()
            || self
                .targets
                .iter()
                .any(|target| matches!(target.route, ExactTargetRoute::Topology))
        {
            return Err("Sumeragi v2 owned reply transfer has invalid target geometry".to_owned());
        }
        let reply_routes = self.reply_routes.as_mut().ok_or_else(|| {
            "Sumeragi v2 owned reply transfer lost its bounded route history".to_owned()
        })?;
        let routes_before = reply_routes.clone();
        let (_, receipt) = reply_routes.retain_active_with_receipt();
        let projected_routes = if let Some(ownership) = self.ingress_ownership.as_mut() {
            ownership.project_retained_reply_routes(receipt)
        } else {
            receipt.into_output(&routes_before)
        }
        .ok_or_else(|| "Sumeragi v2 owned reply pruning lost exact history".to_owned())?;
        *reply_routes = projected_routes;
        let mut retained_targets = Vec::with_capacity(self.targets.len());
        let mut retained_peers = Vec::with_capacity(self.peers.len());
        for (target, peer) in self.targets.drain(..).zip(self.peers.drain(..)) {
            if matches!(&target.route, ExactTargetRoute::Reply(route)
                if reply_routes
                    .iter()
                    .any(|retained| retained.same_delivery(route)))
            {
                retained_targets.push(target);
                retained_peers.push(peer);
            }
        }
        self.targets = retained_targets;
        self.peers = retained_peers;
        self.next_target_index = 0;
        // Close the monotonic race after filtering without independently
        // rereading any target's liveness. The second receipt is the sole
        // authority for both route history and target membership in this pass.
        let routes_before = reply_routes.clone();
        let (_, receipt) = reply_routes.retain_active_with_receipt();
        let projected_routes = if let Some(ownership) = self.ingress_ownership.as_mut() {
            ownership.project_retained_reply_routes(receipt)
        } else {
            receipt.into_output(&routes_before)
        }
        .ok_or_else(|| "Sumeragi v2 owned reply race pruning lost exact history".to_owned())?;
        *reply_routes = projected_routes;
        let mut retained_targets = Vec::with_capacity(self.targets.len());
        let mut retained_peers = Vec::with_capacity(self.peers.len());
        for (target, peer) in self.targets.drain(..).zip(self.peers.drain(..)) {
            if matches!(&target.route, ExactTargetRoute::Reply(route)
                if reply_routes
                    .iter()
                    .any(|retained| retained.same_delivery(route)))
            {
                retained_targets.push(target);
                retained_peers.push(peer);
            }
        }
        self.targets = retained_targets;
        self.peers = retained_peers;
        self.rebuild_current_source_targets()?;
        Ok(self.targets.len())
    }
    fn mark_admitted(&mut self, target_index: usize) -> Result<(), String> {
        if self
            .targets
            .get(target_index)
            .is_some_and(|target| target.parked)
        {
            return Err("Sumeragi v2 admitted a parked reply source".to_owned());
        }
        if self
            .targets
            .get(target_index)
            .is_some_and(|target| target.pending_flush.is_some())
        {
            return Err(
                "Sumeragi v2 advanced a reply cursor before consuming its flush witness".to_owned(),
            );
        }
        let prior_source = self.current_target_source(target_index)?;
        if self
            .current_source_targets
            .get(&prior_source)
            .is_none_or(|targets| !targets.contains(&target_index))
        {
            return Err("Sumeragi v2 local output FIFO lost its current target".to_owned());
        }
        let next_message_index = self
            .targets
            .get(target_index)
            .expect("selected exact-output target must remain present")
            .message_index
            .checked_add(1)
            .ok_or_else(|| "Sumeragi v2 exact-output message cursor overflowed".to_owned())?;
        let next_ingress_ownership = match &self.ingress_ownership {
            Some(ownership) => {
                let ExactTargetRoute::Reply(route) = &self
                    .targets
                    .get(target_index)
                    .expect("selected exact-output target must remain present")
                    .route
                else {
                    return Err(
                        "Sumeragi v2 ingress-owned output changed to a topology route".to_owned(),
                    );
                };
                let message_cursor = u64::try_from(next_message_index).map_err(|_| {
                    "Sumeragi v2 ingress-owned message cursor exceeded u64".to_owned()
                })?;
                let mut next = ownership.clone();
                if !next.advance_reply_cursors(route, message_cursor, 0) {
                    return Err(
                        "Sumeragi v2 exact-output admission regressed ingress ownership".to_owned(),
                    );
                }
                Some(next)
            }
            None => None,
        };
        let target = self
            .targets
            .get_mut(target_index)
            .expect("selected exact-output target must remain present");
        target.message_index = next_message_index;
        target.reply_writer_timeout_attempt = 0;
        self.ingress_ownership = next_ingress_ownership;
        let next_source = (!self.target_is_complete(target_index))
            .then(|| self.current_target_source(target_index))
            .transpose()?;
        if next_source.as_ref() == Some(&prior_source) {
            return Ok(());
        }
        let remove_prior_source = {
            let targets = self
                .current_source_targets
                .get_mut(&prior_source)
                .expect("preflighted local output source must remain present");
            let removed = targets.remove(&target_index);
            debug_assert!(removed);
            targets.is_empty()
        };
        if remove_prior_source {
            self.current_source_targets.remove(&prior_source);
        }
        if let Some(next_source) = next_source
            && !self
                .current_source_targets
                .entry(next_source)
                .or_default()
                .insert(target_index)
        {
            return Err("Sumeragi v2 local output FIFO registered one target twice".to_owned());
        }
        Ok(())
    }
    fn retain_returned(
        &mut self,
        target_index: usize,
        post: Post<NetworkMessage>,
        ticket: Option<NetworkActorAdmissionTicket>,
    ) -> Result<(), String> {
        let target = self
            .targets
            .get_mut(target_index)
            .expect("selected exact-output target must remain present");
        if target.parked {
            return Err("Sumeragi v2 returned output to a parked reply source".to_owned());
        }
        if target.pending_flush.is_some() {
            return Err("Sumeragi v2 returned output over a pending writer flush".to_owned());
        }
        let expected_hash = self
            .message_hashes
            .get(target.message_index)
            .ok_or_else(|| {
                "Sumeragi v2 exact-output target has no expected payload identity".to_owned()
            })?;
        if HashOf::new(&post.data) != *expected_hash {
            return Err("Sumeragi v2 network actor changed an exact output payload".to_owned());
        }
        debug_assert!(target.current.is_none());
        debug_assert!(target.ticket.is_none());
        target.current = Some(post);
        target.ticket = ticket;
        Ok(())
    }
    fn target_is_complete(&self, target_index: usize) -> bool {
        self.targets
            .get(target_index)
            .is_some_and(|target| target.message_index == self.messages.len())
    }
    fn target_source_at(
        &self,
        target_index: usize,
        message_index: usize,
    ) -> Result<ExactTargetSource, String> {
        let peer = self
            .peers
            .get(target_index)
            .ok_or_else(|| "Sumeragi v2 exact-output target lost its peer".to_owned())?;
        let target = self
            .targets
            .get(target_index)
            .ok_or_else(|| "Sumeragi v2 exact-output target disappeared".to_owned())?;
        let class = self
            .message_classes
            .get(message_index)
            .ok_or_else(|| "Sumeragi v2 exact-output target lost its current message".to_owned())?;
        Ok(target.route.source(peer, *class))
    }
    fn current_target_source(&self, target_index: usize) -> Result<ExactTargetSource, String> {
        let message_index = self
            .targets
            .get(target_index)
            .ok_or_else(|| "Sumeragi v2 exact-output target disappeared".to_owned())?
            .message_index;
        self.target_source_at(target_index, message_index)
    }
    fn outstanding_sources(&self) -> Result<BTreeSet<ExactTargetSource>, String> {
        self.outstanding_sources_excluding(None)
    }
    fn outstanding_sources_excluding(
        &self,
        excluded_target: Option<usize>,
    ) -> Result<BTreeSet<ExactTargetSource>, String> {
        let mut sources = BTreeSet::new();
        for (target_index, target) in self.targets.iter().enumerate() {
            if excluded_target == Some(target_index) {
                continue;
            }
            let peer = self
                .peers
                .get(target_index)
                .ok_or_else(|| "Sumeragi v2 exact-output target lost its peer".to_owned())?;
            let classes = self
                .message_class_suffixes
                .get(target.message_index)
                .ok_or_else(|| {
                    "Sumeragi v2 exact-output target advanced beyond its class suffix".to_owned()
                })?;
            for class in exact_output_classes(*classes) {
                sources.insert(target.route.source(peer, class));
            }
        }
        Ok(sources)
    }
    fn target_reservation(
        &self,
        semantic_target: &PeerId,
        class: ExactOutputClass,
    ) -> ExactTargetReservation {
        let kind = if class == ExactOutputClass::Safety && self.is_global_pacemaker_fanout() {
            ExactTargetReservationKind::Pacemaker
        } else if self.certified_sidecar_topology_progress_target() == Some(semantic_target) {
            ExactTargetReservationKind::SidecarTopologyProgress
        } else if self.retryable_certified_sidecar_responder_control_target()
            == Some(semantic_target)
        {
            ExactTargetReservationKind::SidecarReplyControl
        } else {
            ExactTargetReservationKind::Reliable
        };
        ExactTargetReservation {
            semantic_target: semantic_target.clone(),
            class,
            kind,
        }
    }
    fn outstanding_reservation_counts(
        &self,
    ) -> Result<BTreeMap<ExactTargetReservation, usize>, String> {
        let mut reservations = BTreeMap::<ExactTargetReservation, usize>::new();
        for (target_index, target) in self.targets.iter().enumerate() {
            let semantic_target = self
                .peers
                .get(target_index)
                .ok_or_else(|| "Sumeragi v2 exact-output target lost its peer".to_owned())?;
            let classes = self
                .message_class_suffixes
                .get(target.message_index)
                .ok_or_else(|| {
                    "Sumeragi v2 exact-output target advanced beyond its class suffix".to_owned()
                })?;
            for class in exact_output_classes(*classes) {
                let reservation = self.target_reservation(semantic_target, class);
                if reservation.kind == ExactTargetReservationKind::SidecarReplyControl {
                    // One bounded responder-control fanout may retain several
                    // exact authenticated return paths. Route/source bounds
                    // account for those paths; the dedicated progress credit
                    // must remain one unit for the semantic target.
                    reservations.entry(reservation).or_insert(1);
                    continue;
                }
                let count = reservations.entry(reservation).or_default();
                *count = count.checked_add(1).ok_or_else(|| {
                    "Sumeragi v2 outbound target/class ownership overflowed".to_owned()
                })?;
            }
        }
        Ok(reservations)
    }
    /// Reservation demand visible to read-only admission checks.
    fn admission_reservation_counts(
        &self,
    ) -> Result<BTreeMap<ExactTargetReservation, usize>, String> {
        let mut reservations = BTreeMap::<ExactTargetReservation, usize>::new();
        for (target_index, target) in self.targets.iter().enumerate() {
            let semantic_target = self
                .peers
                .get(target_index)
                .ok_or_else(|| "Sumeragi v2 exact-output target lost its peer".to_owned())?;
            let classes = self
                .message_class_suffixes
                .get(target.message_index)
                .ok_or_else(|| {
                    "Sumeragi v2 exact-output target advanced beyond its class suffix".to_owned()
                })?;
            for class in exact_output_classes(*classes) {
                let reservation = self.target_reservation(semantic_target, class);
                if reservation.kind == ExactTargetReservationKind::SidecarReplyControl {
                    reservations.entry(reservation).or_insert(1);
                    continue;
                }
                let count = reservations.entry(reservation).or_default();
                *count = count.checked_add(1).ok_or_else(|| {
                    "Sumeragi v2 outbound admission ownership overflowed".to_owned()
                })?;
            }
        }
        Ok(reservations)
    }
    fn reply_target_merge_plan(&self, candidate: &Self) -> Result<ReplyTargetMergePlan, String> {
        self.reply_target_merge_plan_with_hooks(candidate, |_| {}, || {})
    }
    #[cfg(test)]
    fn reply_target_merge_plan_after_candidate_prune<AfterCandidatePrune>(
        &self,
        candidate: &Self,
        after_candidate_prune: AfterCandidatePrune,
    ) -> Result<ReplyTargetMergePlan, String>
    where
        AfterCandidatePrune: FnMut(usize),
    {
        self.reply_target_merge_plan_with_hooks(candidate, after_candidate_prune, || {})
    }
    #[cfg(test)]
    fn reply_target_merge_plan_after_route_merge<AfterRouteMerge>(
        &self,
        candidate: &Self,
        after_route_merge: AfterRouteMerge,
    ) -> Result<ReplyTargetMergePlan, String>
    where
        AfterRouteMerge: FnOnce(),
    {
        self.reply_target_merge_plan_with_hooks(candidate, |_| {}, after_route_merge)
    }
    fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(
        &self,
        candidate: &Self,
        mut after_candidate_prune: AfterCandidatePrune,
        after_route_merge: AfterRouteMerge,
    ) -> Result<ReplyTargetMergePlan, String>
    where
        AfterCandidatePrune: FnMut(usize),
        AfterRouteMerge: FnOnce(),
    {
        if !self.can_coalesce_retry(candidate) {
            return Err("Sumeragi v2 exact-output request changed semantic identity".to_owned());
        }
        let Some(authority_route) = self.targets.iter().find_map(|target| match &target.route {
            ExactTargetRoute::Reply(route) => Some(route),
            ExactTargetRoute::Topology => None,
        }) else {
            return Err("Sumeragi v2 reply fanout lost its authenticated authority".to_owned());
        };
        // Preserve and consult the actor-owned bounded route history as one
        // atomic capability operation. Pruning records tombstones before the
        // candidate is merged, so a retired target cannot hide a forged
        // cross-source ordinal collision at this seam.
        let retained_routes = self.reply_routes.clone().ok_or_else(|| {
            "Sumeragi v2 retained reply fanout lost its bounded route history".to_owned()
        })?;
        let mut candidate_routes = candidate
            .reply_routes
            .clone()
            .ok_or_else(|| "Sumeragi v2 reply retry lost its bounded route history".to_owned())?;
        let mut candidate_ownership = candidate.ingress_ownership.clone();
        let mut merge_attempt = 0usize;
        let merge_receipt = loop {
            let (_, prune_receipt) = candidate_routes.retain_active_with_receipt();
            if let Some(ownership) = candidate_ownership.as_mut() {
                candidate_routes = ownership
                    .project_retained_reply_routes(prune_receipt)
                    .ok_or_else(|| {
                        "Sumeragi v2 candidate pruning lost fair-ingress ownership".to_owned()
                    })?;
            }
            let live_before_merge = candidate_routes.len();
            after_candidate_prune(merge_attempt);
            let mut merged_routes = retained_routes.clone();
            match merged_routes.merge_with_receipt(&candidate_routes) {
                Ok(receipt) => break ReplyRouteMergeReceipt::Strict(receipt),
                Err(NetworkReplyRouteError::Inactive) => {
                    // A candidate tenure may retire after the owned-transfer
                    // prune but before strict history merge reaches that member.
                    // Activity is monotonic, so the next prune must remove at
                    // least that raced occurrence; otherwise retrying could hide
                    // an invariant violation behind an unbounded loop.
                    let (_, prune_receipt) = candidate_routes.retain_active_with_receipt();
                    if let Some(ownership) = candidate_ownership.as_mut() {
                        candidate_routes = ownership
                            .project_retained_reply_routes(prune_receipt)
                            .ok_or_else(|| {
                            "Sumeragi v2 raced candidate pruning lost fair-ingress ownership"
                                .to_owned()
                        })?;
                    }
                    if candidate_routes.len() >= live_before_merge {
                        return Err(
                            "Sumeragi v2 inactive reply-history retry made no progress".to_owned()
                        );
                    }
                    merge_attempt = merge_attempt.checked_add(1).ok_or_else(|| {
                        "Sumeragi v2 reply-history retry count overflowed".to_owned()
                    })?;
                }
                Err(NetworkReplyRouteError::Stale) => {
                    if !self.rollover_claim.accepts_superseded_reply_delivery() {
                        return Err(
                            "Sumeragi v2 outbound reply fanout contains a stale capability"
                                .to_owned(),
                        );
                    }
                    // A delayed authenticated request may materialize the same
                    // immutable response after a newer delivery for its source
                    // already owns that output. The stale capability must not
                    // replace the retained writer, but supersession is not a
                    // consensus invariant failure. Reconcile only this
                    // classified case so fresh sibling routes and the bounded
                    // ingress history survive; every other capability failure
                    // remains fail-closed below.
                    let receipt = merged_routes
                        .merge_observed_with_receipt(&candidate_routes)
                        .map_err(|error| {
                            format!("invalid superseded Sumeragi v2 reply route history: {error}")
                        })?;
                    break ReplyRouteMergeReceipt::Superseded(receipt);
                }
                Err(error) => {
                    return Err(format!("invalid Sumeragi v2 reply route history: {error}"));
                }
            }
        };
        // Route history is the sole authoritative liveness snapshot for the
        // remainder of this plan. Ownership projects its semantic counts and
        // cursors onto that already-reconciled snapshot, and target membership
        // below never rereads liveness. A route retiring after this point is
        // removed with its target by the next bounded service pass.
        after_route_merge();
        let (merged_routes, ingress_ownership) =
            match (&self.ingress_ownership, candidate_ownership) {
                (Some(retained), Some(candidate)) => {
                    let mut retained = retained.clone();
                    let receipt_routes = match merge_receipt {
                        ReplyRouteMergeReceipt::Strict(receipt) => {
                            retained.merge_downstream_with_strict_receipt(candidate, receipt)
                        }
                        ReplyRouteMergeReceipt::Superseded(receipt) => {
                            retained.merge_downstream_with_observed_receipt(candidate, receipt)
                        }
                    };
                    let Some(receipt_routes) = receipt_routes else {
                        return Err(
                            "Sumeragi v2 exact-output coalescing lost fair-ingress ownership"
                                .to_owned(),
                        );
                    };
                    (receipt_routes, Some(retained))
                }
                (None, None) => {
                    let receipt_routes = match merge_receipt {
                        ReplyRouteMergeReceipt::Strict(receipt) => {
                            receipt.into_output(&retained_routes, &candidate_routes)
                        }
                        ReplyRouteMergeReceipt::Superseded(receipt) => {
                            receipt.into_output(&retained_routes, &candidate_routes)
                        }
                    }
                    .ok_or_else(|| {
                        "Sumeragi v2 exact-output route receipt changed its exact histories"
                            .to_owned()
                    })?;
                    (receipt_routes, None)
                }
                (Some(_), None) | (None, Some(_)) => {
                    return Err(
                        "Sumeragi v2 exact-output retry changed fair-ingress ownership shape"
                            .to_owned(),
                    );
                }
            };
        let mut retained_sources = BTreeSet::new();
        for target in &self.targets {
            let ExactTargetRoute::Reply(route) = &target.route else {
                return Err("Sumeragi v2 retained reply fanout changed route kind".to_owned());
            };
            if !route.same_request_authority(authority_route) {
                return Err("Sumeragi v2 reply capability changed actor or target".to_owned());
            }
            if !retained_sources.insert(route.source_key()) {
                return Err("Sumeragi v2 retained two attempts for one reply source".to_owned());
            }
        }
        let mut plan = Vec::with_capacity(
            self.targets
                .len()
                .checked_add(candidate.targets.len())
                .ok_or_else(|| "Sumeragi v2 reply merge-plan capacity overflowed".to_owned())?,
        );
        for (prior_index, prior_target) in self.targets.iter().enumerate() {
            let ExactTargetRoute::Reply(prior_route) = &prior_target.route else {
                unreachable!("retained reply fanout was validated above");
            };
            if !prior_target.parked
                && !self.target_is_complete(prior_index)
                && !merged_routes
                    .iter()
                    .any(|route| route.same_source(prior_route))
            {
                // The strict merge's authoritative snapshot removed this
                // retained source. Preserve its exact cursor, FIFO age, and
                // reservation while discarding only tenure-bound dispatch
                // state. A later authenticated reconnect updates this same
                // target instead of allocating another source owner.
                plan.push(ReplyTargetMerge::Park { prior_index });
            }
        }
        let mut used_prior = BTreeSet::new();
        let mut unmatched = Vec::new();
        let mut candidate_sources = BTreeSet::new();
        for (candidate_index, candidate_target) in candidate.targets.iter().enumerate() {
            let ExactTargetRoute::Reply(candidate_route) = &candidate_target.route else {
                return Err("Sumeragi v2 reply retry changed route kind".to_owned());
            };
            if !merged_routes
                .iter()
                .any(|route| route.same_delivery(candidate_route))
            {
                // The authoritative post-merge snapshot omitted this retired
                // or superseded occurrence. Do not take a second liveness read.
                continue;
            }
            if !candidate_route.same_request_authority(authority_route) {
                return Err("Sumeragi v2 reply capability changed actor or target".to_owned());
            }
            if !candidate_sources.insert(candidate_route.source_key()) {
                return Err("Sumeragi v2 retry carried one reply source twice".to_owned());
            }
            let prior_index = self.targets.iter().position(|prior| {
                matches!(
                    &prior.route,
                    ExactTargetRoute::Reply(prior_route)
                        if prior_route.same_source(candidate_route)
                )
            });
            if let Some(prior_index) = prior_index {
                if candidate.target_is_complete(candidate_index)
                    && !self.target_is_complete(prior_index)
                {
                    return Err(
                        "Sumeragi v2 retained sidecar flush conflicts with an incomplete source target"
                            .to_owned(),
                    );
                }
                let ExactTargetRoute::Reply(prior_route) = &self.targets[prior_index].route else {
                    unreachable!("located reply target must retain its route kind");
                };
                // The bounded route merge above already linearized liveness.
                // Reuse its immutable joint tenure/delivery monotonic
                // freshness classifier so a delayed delivery from a
                // superseded connection cannot be reclassified as a reconnect
                // solely because its actor-global delivery ordinal is larger.
                let update = candidate_route
                    .source_update_from_snapshot(prior_route)
                    .map_err(|error| {
                        format!(
                            "Sumeragi v2 post-merge reply route lost monotonic freshness: {error}"
                        )
                    })?;
                if !used_prior.insert(prior_index) {
                    return Err("Sumeragi v2 retry updated one reply attempt twice".to_owned());
                }
                // Cursor ownership belongs to the retained source attempt.
                // A reconnect may replace only its route capability; it cannot
                // reinterpret a successfully flushed terminal cursor as the
                // candidate's newly materialized cursor zero.
                plan.push(ReplyTargetMerge::Update {
                    prior_index,
                    candidate_index,
                    update,
                });
            } else {
                unmatched.push(candidate_index);
            }
        }
        for candidate_index in unmatched {
            // An inactive source still owns its non-regressing cursor. A newly
            // observed authenticated source must receive a distinct bounded
            // attempt and can never reuse or erase that parked source's slot.
            plan.push(ReplyTargetMerge::Append { candidate_index });
        }
        Ok(ReplyTargetMergePlan {
            targets: plan,
            reply_routes: merged_routes,
            ingress_ownership,
        })
    }
    fn coalesce_reservation_additions_for_plan(
        &self,
        candidate: &Self,
        plan: &[ReplyTargetMerge],
    ) -> Result<BTreeMap<ExactTargetReservation, usize>, String> {
        let semantic_target = candidate
            .semantic_peers()
            .into_iter()
            .next()
            .ok_or_else(|| "Sumeragi v2 reply fanout lost its semantic target".to_owned())?;
        let retained_reservations = self.outstanding_reservation_counts()?;
        let mut additions = BTreeMap::<ExactTargetReservation, usize>::new();
        for merge in plan {
            let added_mask = match *merge {
                ReplyTargetMerge::Park { .. } | ReplyTargetMerge::Update { .. } => 0,
                ReplyTargetMerge::Append { candidate_index } => {
                    let candidate_target =
                        candidate.targets.get(candidate_index).ok_or_else(|| {
                            "Sumeragi v2 retry candidate disappeared before reservation preflight"
                                .to_owned()
                        })?;
                    *candidate
                        .message_class_suffixes
                        .get(candidate_target.message_index)
                        .ok_or_else(|| {
                            "Sumeragi v2 retry cursor advanced beyond its reservation suffix"
                                .to_owned()
                        })?
                }
            };
            for class in exact_output_classes(added_mask) {
                let reservation = candidate.target_reservation(&semantic_target, class);
                if reservation.kind == ExactTargetReservationKind::SidecarReplyControl
                    && (retained_reservations.contains_key(&reservation)
                        || additions.contains_key(&reservation))
                {
                    continue;
                }
                let count = additions.entry(reservation).or_default();
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| "Sumeragi v2 alternate-route ownership overflowed".to_owned())?;
            }
        }
        Ok(additions)
    }
    fn preview_coalesce_plan(
        &self,
        candidate: &Self,
        plan: &ReplyTargetMergePlan,
    ) -> Result<ReplyTargetMergePreview, String> {
        if self.targets.len() != self.peers.len()
            || candidate.targets.len() != candidate.peers.len()
        {
            return Err("Sumeragi v2 reply fanout changed target geometry".to_owned());
        }
        let mut targets = self
            .targets
            .iter()
            .zip(&self.peers)
            .map(|(target, peer)| {
                (
                    target.route.clone(),
                    target.message_index,
                    target.parked,
                    peer.clone(),
                )
            })
            .collect::<Vec<_>>();
        for merge in &plan.targets {
            match *merge {
                ReplyTargetMerge::Park { prior_index } => {
                    let target = targets.get_mut(prior_index).ok_or_else(|| {
                        "Sumeragi v2 retired merge target disappeared before commit".to_owned()
                    })?;
                    if !matches!(target.0, ExactTargetRoute::Reply(_)) || target.2 {
                        return Err(
                            "Sumeragi v2 retired merge target changed before commit".to_owned()
                        );
                    }
                    target.2 = true;
                }
                ReplyTargetMerge::Update {
                    prior_index,
                    candidate_index,
                    update,
                } => {
                    let target = targets.get_mut(prior_index).ok_or_else(|| {
                        "Sumeragi v2 retry update target disappeared before commit".to_owned()
                    })?;
                    if !matches!(target.0, ExactTargetRoute::Reply(_)) {
                        return Err(
                            "Sumeragi v2 reply update targeted a topology attempt".to_owned()
                        );
                    }
                    let candidate_target =
                        candidate.targets.get(candidate_index).ok_or_else(|| {
                            "Sumeragi v2 retry candidate disappeared before commit".to_owned()
                        })?;
                    let ExactTargetRoute::Reply(candidate_route) = &candidate_target.route else {
                        return Err("Sumeragi v2 retry candidate changed route kind".to_owned());
                    };
                    match update {
                        NetworkReplyRouteSourceUpdate::Exact => {}
                        NetworkReplyRouteSourceUpdate::LaterDelivery => {
                            target.0 = ExactTargetRoute::Reply(candidate_route.clone());
                        }
                        NetworkReplyRouteSourceUpdate::Reconnected => {
                            target.0 = ExactTargetRoute::Reply(candidate_route.clone());
                            target.2 = false;
                        }
                    }
                }
                ReplyTargetMerge::Append { candidate_index } => {
                    let candidate_target =
                        candidate.targets.get(candidate_index).ok_or_else(|| {
                            "Sumeragi v2 retry candidate disappeared before commit".to_owned()
                        })?;
                    if !matches!(candidate_target.route, ExactTargetRoute::Reply(_)) {
                        return Err("Sumeragi v2 retry candidate changed route kind".to_owned());
                    }
                    let candidate_peer = candidate.peers.get(candidate_index).ok_or_else(|| {
                        "Sumeragi v2 retry candidate lost its peer before commit".to_owned()
                    })?;
                    targets.push((
                        candidate_target.route.clone(),
                        candidate_target.message_index,
                        candidate_target.parked,
                        candidate_peer.clone(),
                    ));
                }
            }
        }
        let mut current_source_targets = BTreeMap::<ExactTargetSource, BTreeSet<usize>>::new();
        let mut outstanding_sources = BTreeSet::new();
        for (target_index, (route, message_index, _parked, peer)) in targets.into_iter().enumerate()
        {
            let suffix = *self
                .message_class_suffixes
                .get(message_index)
                .ok_or_else(|| {
                    "Sumeragi v2 retry cursor advanced beyond its class suffix".to_owned()
                })?;
            for class in exact_output_classes(suffix) {
                outstanding_sources.insert(route.source(&peer, class));
            }
            if let Some(class) = self.message_classes.get(message_index) {
                current_source_targets
                    .entry(route.source(&peer, *class))
                    .or_default()
                    .insert(target_index);
            } else if message_index != self.messages.len() {
                return Err("Sumeragi v2 retry cursor advanced beyond its messages".to_owned());
            }
        }
        Ok(ReplyTargetMergePreview {
            current_source_targets,
            outstanding_sources,
        })
    }
    fn commit_coalesce_plan(
        &mut self,
        candidate: &Self,
        plan: &ReplyTargetMergePlan,
        current_source_targets: BTreeMap<ExactTargetSource, BTreeSet<usize>>,
    ) {
        for merge in &plan.targets {
            match *merge {
                ReplyTargetMerge::Park { prior_index } => {
                    let target = &mut self.targets[prior_index];
                    target.current = None;
                    target.ticket = None;
                    target.parked = true;
                }
                ReplyTargetMerge::Update {
                    prior_index,
                    candidate_index,
                    update,
                } => {
                    let ExactTargetRoute::Reply(candidate_route) =
                        &candidate.targets[candidate_index].route
                    else {
                        unreachable!("preflighted reply candidate must retain its route kind");
                    };
                    let target = &mut self.targets[prior_index];
                    target.apply_reply_route_update(candidate_route, update);
                }
                ReplyTargetMerge::Append { candidate_index } => {
                    let candidate_target = &candidate.targets[candidate_index];
                    self.targets.push(PendingExactTarget {
                        route: candidate_target.route.clone(),
                        message_index: candidate_target.message_index,
                        reply_writer_timeout_attempt: candidate_target.reply_writer_timeout_attempt,
                        current: None,
                        ticket: None,
                        pending_flush: None,
                        parked: candidate_target.parked,
                    });
                    self.peers.push(candidate.peers[candidate_index].clone());
                }
            }
        }
        self.reply_routes = Some(plan.reply_routes.clone());
        self.ingress_ownership = plan.ingress_ownership.clone();
        self.current_source_targets = current_source_targets;
    }
    #[cfg(test)]
    fn coalesce_retry(&mut self, candidate: &Self) -> Result<bool, String> {
        if !self.can_coalesce_retry(candidate) {
            return Ok(false);
        }
        let plan = self.reply_target_merge_plan(candidate)?;
        let preview = self.preview_coalesce_plan(candidate, &plan)?;
        self.commit_coalesce_plan(candidate, &plan, preview.current_source_targets);
        Ok(true)
    }
    fn can_coalesce_retry(&self, candidate: &Self) -> bool {
        self.message_hashes == candidate.message_hashes
            && self.semantic_peers() == candidate.semantic_peers()
            && self.rollover_claim == candidate.rollover_claim
            && self
                .targets
                .iter()
                .chain(&candidate.targets)
                .all(|target| matches!(&target.route, ExactTargetRoute::Reply(_)))
    }
    fn is_certified_sidecar_chunk_fanout(&self) -> bool {
        matches!(
            self.messages.as_slice(),
            [NetworkMessage::CertifiedMergeSidecar(message)]
                if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(_))
        ) && matches!(
            self.rollover_claim,
            ExactOutputRolloverClaim::CertifiedSidecarChunk { .. }
        )
    }
    /// Return the frozen-target reservation identity for topology-routed sidecar progress.
    ///
    /// Requester-owned Request/Close output needs one topology delivery
    /// opportunity independent of a parked reply source.
    fn certified_sidecar_topology_progress_target(&self) -> Option<&PeerId> {
        let target = match (self.messages.as_slice(), &self.rollover_claim) {
            (
                [NetworkMessage::CertifiedMergeSidecar(message)],
                ExactOutputRolloverClaim::CertifiedSidecarRequest { target, .. },
            ) if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Request(_))
                && matches!(
                    self.targets.as_slice(),
                    [route] if matches!(&route.route, ExactTargetRoute::Topology)
                ) =>
            {
                target
            }
            (
                [NetworkMessage::CertifiedMergeSidecar(message)],
                ExactOutputRolloverClaim::CertifiedSidecarControl { target, .. },
            ) if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Close(_))
                && matches!(
                    self.targets.as_slice(),
                    [route] if matches!(&route.route, ExactTargetRoute::Topology)
                ) =>
            {
                target
            }
            _ => return None,
        };
        self.peers
            .iter()
            .all(|peer| peer == target)
            .then_some(target)
    }
    /// Return a statelessly reproducible responder-control target. At most one
    /// is retained per target; requester output and responder chunks keep exact
    /// ownership, while controls for different targets stay independent.
    fn retryable_certified_sidecar_responder_control_target(&self) -> Option<&PeerId> {
        let route_shape_is_valid = match self.messages.as_slice() {
            [NetworkMessage::CertifiedMergeSidecar(message)] => match message.as_ref() {
                CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_) => self
                    .targets
                    .iter()
                    .all(|route| matches!(&route.route, ExactTargetRoute::Reply(_))),
                CertifiedMergeSidecarMessage::Request(_)
                | CertifiedMergeSidecarMessage::Close(_)
                | CertifiedMergeSidecarMessage::Chunk(_) => false,
            },
            _ => false,
        };
        let ExactOutputRolloverClaim::CertifiedSidecarControl { target, .. } = &self.rollover_claim
        else {
            return None;
        };
        (route_shape_is_valid
            && !self.targets.is_empty()
            && self.peers.iter().all(|peer| peer == target))
        .then_some(target)
    }
    /// Return whether one incomplete exact-reply target still has a writer.
    fn has_writable_reply_target(&self) -> bool {
        self.targets.iter().enumerate().any(|(index, target)| {
            !self.target_is_complete(index)
                && matches!(
                    &target.route,
                    ExactTargetRoute::Reply(route) if route.is_reply_writable()
                )
        })
    }
    /// Whether a responder control has no writer and no pending flush witness;
    /// only then may its actor-returned ticket cancel the reservation.
    fn is_stranded_retryable_certified_sidecar_responder_control(&self) -> bool {
        self.retryable_certified_sidecar_responder_control_target()
            .is_some()
            && !self.is_complete()
            && !self.has_writable_reply_target()
            && self
                .targets
                .iter()
                .all(|target| target.pending_flush.is_none())
    }
    #[cfg(test)]
    fn is_retryable_certified_sidecar_responder_control_fanout(&self) -> bool {
        self.retryable_certified_sidecar_responder_control_target()
            .is_some()
    }
    fn owns_source(&self, source: &ExactTargetSource) -> Result<bool, String> {
        for (target_index, target) in self.targets.iter().enumerate() {
            let peer = self
                .peers
                .get(target_index)
                .ok_or_else(|| "Sumeragi v2 exact-output target lost its peer".to_owned())?;
            let classes = self
                .message_class_suffixes
                .get(target.message_index)
                .ok_or_else(|| {
                    "Sumeragi v2 exact-output target advanced beyond its class suffix".to_owned()
                })?;
            if exact_output_classes(*classes)
                .any(|class| target.route.source(peer, class) == *source)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn target_is_local_head(&self, target_index: usize) -> Result<bool, String> {
        let source = self.current_target_source(target_index)?;
        let local_head = self
            .current_source_targets
            .get(&source)
            .and_then(BTreeSet::first)
            .ok_or_else(|| "Sumeragi v2 local output FIFO lost its current source".to_owned())?;
        Ok(*local_head == target_index)
    }
    fn advance_target_cursor(&mut self, target_index: usize) {
        self.next_target_index = (target_index + 1) % self.targets.len();
    }
    fn is_complete(&self) -> bool {
        self.targets
            .iter()
            .all(|target| target.message_index == self.messages.len())
    }
    fn has_dispatchable_target(&self) -> bool {
        self.targets.iter().enumerate().any(|(index, target)| {
            !target.parked && target.pending_flush.is_none() && !self.target_is_complete(index)
        })
    }
}
