//! Bounded off-actor serving of certified historical block bodies.
//!
//! The serialized consensus owner performs only request-identity validation and
//! finite admission. Kura reads, canonical body projection, RS16 encoding,
//! response signing, full-frame encoding, and full-frame hashing stay on this
//! dedicated worker. The completion carries a private source proof so exact
//! output and applied-height rollover can validate it without repeating that
//! work on the consensus actor.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroUsize,
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    time::{Duration, Instant},
};

use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{NetworkId, block::consensus_v2 as wire, peer::PeerId};
use iroha_p2p::network::NetworkReplyRoutes;

use super::{
    HistoricalBodyRequestIdentity, V2BlockSyncError, build_historical_body_response,
    rebind_cached_historical_body_response,
};
use crate::{
    NetworkMessage,
    kura::Kura,
    sumeragi::{
        FairV2IngressOwnershipEvidence,
        message::{BlockMessage, BlockMessageWire},
        v2_transport::authenticate_certified_body_request_identity,
    },
};

const FIRST_RELEASE_TASK_QUEUE_CAPACITY: usize = 2;
const FIRST_RELEASE_GLOBAL_BYTES_PER_SECOND: u64 = 64 * 1024 * 1024;
const FIRST_RELEASE_PRINCIPAL_BYTES_PER_SECOND: u64 = 32 * 1024 * 1024;
const FIRST_RELEASE_CACHE_RETAINED_HEAP_BYTES: u64 = 2 * wire::MAX_DA_ENCODED_PAYLOAD_BYTES;
// This covers the response/identity map nodes, FIFO slot, `Arc` allocations,
// fixed message/proof structs, and four maximum-size compact public-key
// buffers. Dynamic response vectors and the encoded frame are charged from
// their actual capacities separately.
const FIRST_RELEASE_CACHE_ENTRY_FIXED_HEAP_BYTES: usize =
    16 * 1024 + 4 * iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES;
// The root NetworkMessage decode policy uses this same cap specifically for
// CertifiedBodyResponse. It covers the canonical body, manifest hash sequence,
// signatures, every nested Norito envelope, and the authenticated P2P frame.
const FIRST_RELEASE_MAX_RESPONSE_FRAME_BYTES: usize =
    crate::MAX_SUMERAGI_V2_CERTIFIED_BODY_RESPONSE_NETWORK_FRAME_BYTES;

/// Local-only resource geometry for the historical-body worker.
///
/// This seam deliberately does not enter the shared Sumeragi configuration
/// fingerprint. A follow-up can populate the same fields from node-local
/// configuration without changing the worker or its admission invariant.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HistoricalBodyServeLimits {
    task_queue_capacity: NonZeroUsize,
    max_reply_route_fanout: NonZeroUsize,
    cache_entry_capacity: NonZeroUsize,
    cache_retained_heap_capacity: NonZeroUsize,
    principal_state_capacity: NonZeroUsize,
    global_bytes_per_second: u64,
    global_burst_bytes: u64,
    principal_bytes_per_second: u64,
    principal_burst_bytes: u64,
    admission_charge_bytes: u64,
}

impl HistoricalBodyServeLimits {
    /// Construct first-release local service bounds from the network actor's
    /// authenticated reply-source geometry.
    pub(crate) fn first_release(
        cache_entry_capacity: usize,
        reply_route_source_capacity: usize,
    ) -> Result<Self, V2BlockSyncError> {
        let cache_entry_capacity = NonZeroUsize::new(cache_entry_capacity)
            .ok_or_else(|| V2BlockSyncError::HistoricalBodyService("zero cache capacity".into()))?;
        let task_queue_capacity = NonZeroUsize::new(FIRST_RELEASE_TASK_QUEUE_CAPACITY)
            .expect("first-release historical-body queue capacity is non-zero");
        let max_reply_route_fanout =
            NonZeroUsize::new(reply_route_source_capacity).ok_or_else(|| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body reply-source capacity must be non-zero".into(),
                )
            })?;
        let cache_retained_heap_capacity = usize::try_from(FIRST_RELEASE_CACHE_RETAINED_HEAP_BYTES)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body cache retained-heap capacity is not representable".into(),
                )
            })?;
        // Every outstanding request can charge every authenticated route source
        // plus a distinct signed requester. Keep enough exact principal slots
        // for the complete bounded worker queue.
        let principal_state_capacity = max_reply_route_fanout
            .get()
            .checked_add(1)
            .and_then(|principals| principals.checked_mul(task_queue_capacity.get()))
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body principal capacity is not representable".into(),
                )
            })?;
        let admission_charge_bytes = u64::try_from(FIRST_RELEASE_MAX_RESPONSE_FRAME_BYTES)
            .map_err(|_| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body maximum response frame is not representable".into(),
                )
            })?;
        let maximum_fanout_charge = admission_charge_bytes
            .checked_mul(u64::try_from(max_reply_route_fanout.get()).map_err(|_| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body reply-source capacity is not representable".into(),
                )
            })?)
            .ok_or_else(|| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body maximum reply fanout charge overflowed".into(),
                )
            })?;
        let limits = Self {
            task_queue_capacity,
            max_reply_route_fanout,
            cache_entry_capacity,
            cache_retained_heap_capacity,
            principal_state_capacity,
            global_bytes_per_second: FIRST_RELEASE_GLOBAL_BYTES_PER_SECOND,
            global_burst_bytes: maximum_fanout_charge,
            principal_bytes_per_second: FIRST_RELEASE_PRINCIPAL_BYTES_PER_SECOND,
            principal_burst_bytes: maximum_fanout_charge,
            admission_charge_bytes,
        };
        limits.validate()?;
        Ok(limits)
    }

    fn validate(self) -> Result<(), V2BlockSyncError> {
        let maximum_fanout_charge = self
            .admission_charge_bytes
            .checked_mul(
                u64::try_from(self.max_reply_route_fanout.get()).map_err(|_| {
                    V2BlockSyncError::HistoricalBodyService(
                        "historical-body maximum reply fanout is not representable".into(),
                    )
                })?,
            )
            .ok_or_else(|| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body maximum reply fanout charge overflowed".into(),
                )
            })?;
        if self.global_bytes_per_second == 0
            || self.principal_bytes_per_second == 0
            || self.global_burst_bytes < maximum_fanout_charge
            || self.principal_burst_bytes < maximum_fanout_charge
            || self.global_burst_bytes < self.principal_burst_bytes
            || self.global_bytes_per_second < self.principal_bytes_per_second
        {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "historical-body limits cannot reserve one maximum supported reply fanout globally and per principal"
                    .into(),
            ));
        }
        Ok(())
    }
}

/// One exact dequeued request whose ingress and reply ownership stays attached
/// until either a prepared output is enqueued or the request is retired.
pub(crate) struct HistoricalBodyServeTask {
    request: wire::CertifiedBodyRequest,
    recipient: PeerId,
    authenticated_via: PeerId,
    reply_routes: NetworkReplyRoutes,
    admission_plan: HistoricalBodyAdmissionPlan,
    ingress_ownership: FairV2IngressOwnershipEvidence,
}

#[derive(Clone)]
struct HistoricalBodyAdmissionPlan {
    reply_route_source_capacity: usize,
    active_route_count: usize,
    active_route_sources: BTreeSet<PeerId>,
}

impl HistoricalBodyAdmissionPlan {
    fn from_reply_routes(reply_routes: &NetworkReplyRoutes) -> Result<Self, V2BlockSyncError> {
        let reply_route_source_capacity = reply_routes.source_capacity();
        let mut active_route_count = 0_usize;
        let mut active_route_sources = BTreeSet::new();
        for route in reply_routes.iter().filter(|route| route.is_active()) {
            active_route_count = active_route_count.checked_add(1).ok_or_else(|| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body active reply-route count overflowed".into(),
                )
            })?;
            if !active_route_sources.insert(route.authenticated_source_peer().clone()) {
                return Err(V2BlockSyncError::HistoricalBodyService(
                    "historical-body reply routes repeated an authenticated source".into(),
                ));
            }
        }
        Ok(Self {
            reply_route_source_capacity,
            active_route_count,
            active_route_sources,
        })
    }

    fn charges(
        &self,
        requester: &PeerId,
        charge_per_route: u64,
        max_reply_route_fanout: usize,
    ) -> Result<Option<HistoricalBodyAdmissionCharges>, V2BlockSyncError> {
        if self.active_route_sources.len() != self.active_route_count {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "historical-body active reply-route charge plan is inconsistent".into(),
            ));
        }
        if self.active_route_count == 0 {
            return Ok(None);
        }
        // The route set carries the network actor's configured authenticated-
        // source bound. A foreign/stale geometry or an over-bound remote set is
        // terminally rejected at admission without turning remote input into a
        // fail-stop local service error.
        if self.reply_route_source_capacity != max_reply_route_fanout
            || self.active_route_count > self.reply_route_source_capacity
        {
            return Ok(None);
        }
        let route_count = u64::try_from(self.active_route_count).map_err(|_| {
            V2BlockSyncError::HistoricalBodyService(
                "historical-body active reply-route count is not representable".into(),
            )
        })?;
        let fanout_charge = charge_per_route.checked_mul(route_count).ok_or_else(|| {
            V2BlockSyncError::HistoricalBodyService(
                "historical-body reply fanout byte charge overflowed".into(),
            )
        })?;
        let mut principals = BTreeMap::new();
        principals.insert(requester.clone(), fanout_charge);
        for source in &self.active_route_sources {
            if source != requester
                && principals
                    .insert(source.clone(), charge_per_route)
                    .is_some()
            {
                return Err(V2BlockSyncError::HistoricalBodyService(
                    "historical-body admission repeated a charged principal".into(),
                ));
            }
        }
        Ok(Some(HistoricalBodyAdmissionCharges {
            global: fanout_charge,
            principals,
        }))
    }
}

#[derive(Debug)]
struct HistoricalBodyAdmissionCharges {
    global: u64,
    principals: BTreeMap<PeerId, u64>,
}

impl HistoricalBodyServeTask {
    /// Bind a request to the exact authenticated transport occurrence.
    pub(crate) fn from_bound_ingress(
        request: wire::CertifiedBodyRequest,
        recipient: PeerId,
        authenticated_via: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> Result<Self, V2BlockSyncError> {
        let exact_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
        ));
        if request.requester != recipient
            || reply_routes.semantic_target() != &recipient
            || !reply_routes
                .iter()
                .any(|route| route.is_authenticated_via(&authenticated_via))
            || !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_message(&exact_message)
            || !ingress_ownership.matches_semantic_origin(&recipient)
            || !ingress_ownership.matches_reply_routes(Some(&reply_routes))
        {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "historical-body task changed authenticated ingress ownership".into(),
            ));
        }
        authenticate_certified_body_request_identity(&request, &recipient)?;
        let admission_plan = HistoricalBodyAdmissionPlan::from_reply_routes(&reply_routes)?;
        Ok(Self {
            request,
            recipient,
            authenticated_via,
            reply_routes,
            admission_plan,
            ingress_ownership,
        })
    }

    fn admission_charges(
        &self,
        charge_per_route: u64,
        max_reply_route_fanout: usize,
    ) -> Result<Option<HistoricalBodyAdmissionCharges>, V2BlockSyncError> {
        self.admission_plan.charges(
            &self.request.requester,
            charge_per_route,
            max_reply_route_fanout,
        )
    }

    /// Borrow the exact ingress owner for terminal retirement.
    pub(crate) fn ingress_ownership(&self) -> &FairV2IngressOwnershipEvidence {
        &self.ingress_ownership
    }

    fn clone_for_prepared_retry(&self) -> Self {
        Self {
            request: self.request.clone(),
            recipient: self.recipient.clone(),
            authenticated_via: self.authenticated_via.clone(),
            reply_routes: self.reply_routes.clone(),
            admission_plan: self.admission_plan.clone(),
            ingress_ownership: self.ingress_ownership.clone(),
        }
    }
}

/// Opaque proof that a dedicated worker matched one exact response to immutable
/// Kura history before preparing its canonical network frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HistoricalBodyDurableSourceProof {
    network_id: NetworkId,
    source_round: wire::ConsensusRound,
    source_subject: wire::BlockSubject,
    responder: PeerId,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    exact_output_hash: HashOf<NetworkMessage>,
}

impl HistoricalBodyDurableSourceProof {
    fn mint(
        network_id: NetworkId,
        request: &wire::CertifiedBodyRequest,
        network_message: &NetworkMessage,
    ) -> Result<Self, V2BlockSyncError> {
        let NetworkMessage::SumeragiBlock(envelope) = network_message else {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "prepared historical body is not Sumeragi block traffic".into(),
            ));
        };
        let BlockMessage::V2(message) = envelope.as_message() else {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "prepared historical body changed message family".into(),
            ));
        };
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload
        else {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "prepared historical body changed payload family".into(),
            ));
        };
        let exact_output_hash =
            validated_cached_exact_output_hash(response, request, network_message)?;
        Ok(Self {
            network_id,
            source_round: request.round,
            source_subject: request.subject,
            responder: response.responder.clone(),
            request_hash: HashOf::new(request),
            exact_output_hash,
        })
    }

    /// Match a prepared frame in one expected network using only scalar metadata
    /// and the worker-warmed hash.
    pub(crate) fn covers_message_in_network(
        &self,
        expected_network_id: &NetworkId,
        message: &NetworkMessage,
    ) -> bool {
        if &self.network_id != expected_network_id {
            return false;
        }
        let NetworkMessage::SumeragiBlock(envelope) = message else {
            return false;
        };
        let BlockMessage::V2(v2) = envelope.as_message() else {
            return false;
        };
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &v2.payload else {
            return false;
        };
        response.request_hash == self.request_hash
            && response.manifest.round == self.source_round
            && response.manifest.subject == self.source_subject
            && response.responder == self.responder
            && message.cached_exact_output_hash() == Some(self.exact_output_hash)
    }

    /// Return the network whose immutable Kura source minted this proof.
    pub(crate) const fn network_id(&self) -> NetworkId {
        self.network_id
    }

    /// Return the historical source round.
    pub(crate) const fn source_round(&self) -> wire::ConsensusRound {
        self.source_round
    }

    /// Borrow the worker's authenticated responder identity.
    pub(crate) fn responder(&self) -> &PeerId {
        &self.responder
    }
}

fn validated_cached_exact_output_hash(
    response: &wire::CertifiedBodyResponse,
    request: &wire::CertifiedBodyRequest,
    message: &NetworkMessage,
) -> Result<HashOf<NetworkMessage>, V2BlockSyncError> {
    if response.request_hash != HashOf::new(request)
        || response.manifest.round != request.round
        || response.manifest.subject != request.subject
    {
        return Err(V2BlockSyncError::HistoricalBodyService(
            "prepared historical body changed its exact request binding".into(),
        ));
    }
    message.cached_exact_output_hash().ok_or_else(|| {
        V2BlockSyncError::HistoricalBodyService(
            "historical-body worker did not precompute exact output identity".into(),
        )
    })
}

/// Prepared output whose fields are available only through a consuming handoff
/// into the exact-output service.
///
/// The post boundary may mint one private retry carrier before that handoff so
/// a full exact-output corridor cannot discard the worker-owned source.
pub(crate) struct PreparedHistoricalBodyOutput {
    task: HistoricalBodyServeTask,
    message: NetworkMessage,
    proof: HistoricalBodyDurableSourceProof,
}

impl PreparedHistoricalBodyOutput {
    /// Clone one bounded carrier for a possible exact-output retry.
    pub(crate) fn clone_for_exact_output_retry(&self) -> Self {
        Self {
            task: self.task.clone_for_prepared_retry(),
            message: self.message.clone(),
            proof: self.proof.clone(),
        }
    }

    /// Consume the sealed output into the exact-output post boundary.
    pub(crate) fn into_post_parts(
        self,
    ) -> (
        PeerId,
        NetworkReplyRoutes,
        FairV2IngressOwnershipEvidence,
        NetworkMessage,
        HistoricalBodyDurableSourceProof,
    ) {
        (
            self.task.recipient,
            self.task.reply_routes,
            self.task.ingress_ownership,
            self.message,
            self.proof,
        )
    }
}

/// Result of transferring a prepared body into the exact-output corridor.
#[allow(clippy::large_enum_variant, variant_size_differences)]
#[must_use = "a source-retained prepared body must be retried"]
pub(crate) enum PreparedHistoricalBodyPostOutcome {
    /// Exact output owns the response or completed every active reply route.
    Posted,
    /// The bounded corridor was full; the actor must retry this exact carrier.
    SourceRetained(PreparedHistoricalBodyOutput),
}

/// One terminal worker result. Failed requests retain their exact ingress owner
/// so the actor can distinguish remote rejection from a fail-stop local error.
pub(crate) enum HistoricalBodyServeCompletion {
    /// One fully prepared response ready for constant-time actor admission.
    Prepared(PreparedHistoricalBodyOutput),
    /// The requested immutable history is not present on this node.
    NoResponse(HistoricalBodyServeTask),
    /// Preparation rejected the request or found a fail-stop local error.
    Failed(HistoricalBodyServeTask, V2BlockSyncError),
}

impl HistoricalBodyServeCompletion {
    fn task(&self) -> &HistoricalBodyServeTask {
        match self {
            Self::Prepared(output) => &output.task,
            Self::NoResponse(task) | Self::Failed(task, _) => task,
        }
    }
}

/// Non-blocking local admission result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoricalBodyServeAdmission {
    /// Ownership moved into the bounded worker corridor.
    Queued,
    /// The global, transport-source, or requester time budget is exhausted.
    RateLimited,
    /// The fixed global outstanding-work bound is occupied.
    Busy,
}

/// Actor-side handle for the dedicated historical-body worker.
pub(crate) struct HistoricalBodyServeService {
    task_tx: SyncSender<HistoricalBodyServeTask>,
    completion_rx: Receiver<HistoricalBodyServeCompletion>,
    deferred_prepared: Option<PreparedHistoricalBodyOutput>,
    admission: HistoricalBodyAdmissionState,
}

impl HistoricalBodyServeService {
    /// Start one dedicated worker with fixed queue, rate, and cache bounds.
    pub(crate) fn spawn(
        network_id: NetworkId,
        kura: Arc<Kura>,
        responder_key: KeyPair,
        limits: HistoricalBodyServeLimits,
    ) -> Result<Self, V2BlockSyncError> {
        limits.validate()?;
        let queue_capacity = limits.task_queue_capacity.get();
        let (task_tx, task_rx) = mpsc::sync_channel(queue_capacity);
        let (completion_tx, completion_rx) = mpsc::sync_channel(queue_capacity);
        std::thread::Builder::new()
            .name("sumeragi-v2-historical-body".into())
            .spawn(move || {
                historical_body_worker(
                    network_id,
                    kura,
                    responder_key,
                    limits,
                    task_rx,
                    completion_tx,
                );
            })
            .map_err(|error| V2BlockSyncError::HistoricalBodyService(error.to_string()))?;
        Ok(Self {
            task_tx,
            completion_rx,
            deferred_prepared: None,
            admission: HistoricalBodyAdmissionState::new(limits),
        })
    }

    /// Reserve all applicable budgets and enqueue without waiting.
    pub(crate) fn try_enqueue(
        &mut self,
        task: HistoricalBodyServeTask,
    ) -> Result<HistoricalBodyServeAdmission, V2BlockSyncError> {
        if self.deferred_prepared.is_some() {
            return Ok(HistoricalBodyServeAdmission::Busy);
        }
        let now = Instant::now();
        if !self.admission.try_reserve(&task, now)? {
            return Ok(
                if self.admission.outstanding >= self.admission.outstanding_capacity {
                    HistoricalBodyServeAdmission::Busy
                } else {
                    HistoricalBodyServeAdmission::RateLimited
                },
            );
        }
        match self.task_tx.try_send(task) {
            Ok(()) => Ok(HistoricalBodyServeAdmission::Queued),
            Err(TrySendError::Full(task)) => {
                self.admission.release(&task)?;
                Ok(HistoricalBodyServeAdmission::Busy)
            }
            Err(TrySendError::Disconnected(task)) => {
                self.admission.release(&task)?;
                Err(V2BlockSyncError::HistoricalBodyWorkerDisconnected)
            }
        }
    }

    /// Take at most one completion and release its outstanding-work slot.
    pub(crate) fn try_recv(
        &mut self,
    ) -> Result<Option<HistoricalBodyServeCompletion>, V2BlockSyncError> {
        if let Some(prepared) = self.deferred_prepared.take() {
            return Ok(Some(HistoricalBodyServeCompletion::Prepared(prepared)));
        }
        match self.completion_rx.try_recv() {
            Ok(completion) => {
                self.admission.release(completion.task())?;
                Ok(Some(completion))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err(V2BlockSyncError::HistoricalBodyWorkerDisconnected)
            }
        }
    }

    /// Retain one exact-output-rejected completion for the next actor turn.
    pub(crate) fn defer_prepared(
        &mut self,
        prepared: PreparedHistoricalBodyOutput,
    ) -> Result<(), V2BlockSyncError> {
        if self.deferred_prepared.is_some() {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "historical-body retry slot already owns a prepared response".into(),
            ));
        }
        self.deferred_prepared = Some(prepared);
        Ok(())
    }

    /// Return whether a queued, running, or completed task still owns ingress.
    pub(crate) fn has_pending(&self) -> bool {
        self.deferred_prepared.is_some() || self.admission.outstanding != 0
    }
}

fn historical_body_worker(
    network_id: NetworkId,
    kura: Arc<Kura>,
    responder_key: KeyPair,
    limits: HistoricalBodyServeLimits,
    task_rx: Receiver<HistoricalBodyServeTask>,
    completion_tx: SyncSender<HistoricalBodyServeCompletion>,
) {
    let mut cache = HistoricalBodyResponseCache::new(network_id, limits);
    while let Ok(task) = task_rx.recv() {
        let result = cache.serve(
            kura.as_ref(),
            &task.request,
            &task.recipient,
            &responder_key,
        );
        let completion = match result {
            Ok(Some((message, proof))) => {
                HistoricalBodyServeCompletion::Prepared(PreparedHistoricalBodyOutput {
                    task,
                    message,
                    proof,
                })
            }
            Ok(None) => HistoricalBodyServeCompletion::NoResponse(task),
            Err(error) => HistoricalBodyServeCompletion::Failed(task, error),
        };
        if completion_tx.send(completion).is_err() {
            return;
        }
    }
}

#[derive(Clone)]
struct HistoricalBodyAdmissionState {
    global: RateBucket,
    principals: BTreeMap<PeerId, PrincipalState>,
    principal_capacity: usize,
    principal_rate: u64,
    principal_burst: u64,
    admission_charge: u64,
    max_reply_route_fanout: usize,
    outstanding: usize,
    outstanding_capacity: usize,
}

impl HistoricalBodyAdmissionState {
    fn new(limits: HistoricalBodyServeLimits) -> Self {
        let now = Instant::now();
        Self {
            global: RateBucket::new(
                limits.global_bytes_per_second,
                limits.global_burst_bytes,
                now,
            ),
            principals: BTreeMap::new(),
            principal_capacity: limits.principal_state_capacity.get(),
            principal_rate: limits.principal_bytes_per_second,
            principal_burst: limits.principal_burst_bytes,
            admission_charge: limits.admission_charge_bytes,
            max_reply_route_fanout: limits.max_reply_route_fanout.get(),
            outstanding: 0,
            outstanding_capacity: limits.task_queue_capacity.get(),
        }
    }

    fn try_reserve(
        &mut self,
        task: &HistoricalBodyServeTask,
        now: Instant,
    ) -> Result<bool, V2BlockSyncError> {
        let Some(charges) =
            task.admission_charges(self.admission_charge, self.max_reply_route_fanout)?
        else {
            return Ok(false);
        };
        self.try_reserve_charges(&charges, now)
    }

    /// Apply a complete fanout reservation only after the bounded shadow state
    /// passes every global, principal, capacity, and overflow check.
    fn try_reserve_charges(
        &mut self,
        charges: &HistoricalBodyAdmissionCharges,
        now: Instant,
    ) -> Result<bool, V2BlockSyncError> {
        let mut next = self.clone();
        if !next.try_reserve_charges_in_place(charges, now)? {
            return Ok(false);
        }
        *self = next;
        Ok(true)
    }

    fn try_reserve_charges_in_place(
        &mut self,
        charges: &HistoricalBodyAdmissionCharges,
        now: Instant,
    ) -> Result<bool, V2BlockSyncError> {
        if charges.global == 0 || charges.principals.is_empty() {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "historical-body admission produced an empty fanout charge".into(),
            ));
        }
        if charges.principals.values().any(|charge| *charge == 0) {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "historical-body admission produced a zero principal charge".into(),
            ));
        }
        if self.outstanding >= self.outstanding_capacity
            || !self.global.can_take(charges.global, now)
        {
            return Ok(false);
        }
        for (peer, charge) in &charges.principals {
            if let Some(state) = self.principals.get_mut(peer) {
                if !state.can_reserve(*charge, now) {
                    return Ok(false);
                }
            } else if *charge > self.principal_burst {
                return Ok(false);
            }
        }
        if !self.ensure_principals(charges.principals.keys().cloned(), now)? {
            return Ok(false);
        }
        if !self.global.take(charges.global, now) {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "historical-body global admission changed after preflight".into(),
            ));
        }
        for (peer, charge) in &charges.principals {
            self.principals
                .get_mut(peer)
                .ok_or_else(|| {
                    V2BlockSyncError::HistoricalBodyService(
                        "historical-body admission lost a charged principal".into(),
                    )
                })?
                .reserve(*charge, now)?;
        }
        self.outstanding = self.outstanding.checked_add(1).ok_or_else(|| {
            V2BlockSyncError::HistoricalBodyService(
                "historical-body outstanding admission overflow".into(),
            )
        })?;
        Ok(true)
    }

    #[cfg(test)]
    fn try_reserve_principals(
        &mut self,
        first: &PeerId,
        second: Option<&PeerId>,
        now: Instant,
    ) -> Result<bool, V2BlockSyncError> {
        let charges = self.single_route_test_charges(first, second);
        self.try_reserve_charges(&charges, now)
    }

    fn ensure_principals(
        &mut self,
        desired: impl IntoIterator<Item = PeerId>,
        now: Instant,
    ) -> Result<bool, V2BlockSyncError> {
        let desired = desired.into_iter().collect::<BTreeSet<_>>();
        if desired.len() > self.principal_capacity {
            return Ok(false);
        }
        let missing = desired
            .iter()
            .filter(|peer| !self.principals.contains_key(*peer))
            .cloned()
            .collect::<Vec<_>>();

        let retained = self
            .principals
            .len()
            .checked_add(missing.len())
            .ok_or_else(|| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body principal cardinality overflowed".into(),
                )
            })?;
        let evictions_needed = retained.saturating_sub(self.principal_capacity);
        let mut evictions = Vec::with_capacity(evictions_needed);
        if evictions_needed != 0 {
            for (candidate, state) in &mut self.principals {
                if desired.contains(candidate) {
                    continue;
                }
                if state.outstanding == 0 && state.bucket.is_full(now) {
                    evictions.push(candidate.clone());
                    if evictions.len() == evictions_needed {
                        break;
                    }
                }
            }
            if evictions.len() != evictions_needed {
                return Ok(false);
            }
        }
        for evict in evictions {
            self.principals.remove(&evict);
        }
        for peer in missing {
            if self
                .principals
                .insert(
                    peer,
                    PrincipalState::new(self.principal_rate, self.principal_burst, now),
                )
                .is_some()
            {
                return Err(V2BlockSyncError::HistoricalBodyService(
                    "historical-body principal insertion replaced live state".into(),
                ));
            }
        }
        Ok(true)
    }

    fn release(&mut self, task: &HistoricalBodyServeTask) -> Result<(), V2BlockSyncError> {
        let charges = task
            .admission_charges(self.admission_charge, self.max_reply_route_fanout)?
            .ok_or_else(|| {
                V2BlockSyncError::HistoricalBodyService(
                    "historical-body completion lost its active-route reservation".into(),
                )
            })?;
        self.release_charges(&charges)
    }

    /// Decrement all principal outstanding counters as one transaction. Token
    /// charges intentionally remain spent because the worker already owned the
    /// work.
    fn release_charges(
        &mut self,
        charges: &HistoricalBodyAdmissionCharges,
    ) -> Result<(), V2BlockSyncError> {
        let mut next = self.clone();
        next.release_charges_in_place(charges)?;
        *self = next;
        Ok(())
    }

    fn release_charges_in_place(
        &mut self,
        charges: &HistoricalBodyAdmissionCharges,
    ) -> Result<(), V2BlockSyncError> {
        if self.outstanding == 0 {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "historical-body completion underflowed outstanding work".into(),
            ));
        }
        for peer in charges.principals.keys() {
            if self
                .principals
                .get(peer)
                .is_none_or(|state| state.outstanding == 0)
            {
                return Err(V2BlockSyncError::HistoricalBodyService(
                    "historical-body completion lost a charged principal".into(),
                ));
            }
        }
        for peer in charges.principals.keys() {
            self.principals
                .get_mut(peer)
                .expect("preflighted historical-body principal remains present")
                .release()?;
        }
        self.outstanding = self.outstanding.checked_sub(1).ok_or_else(|| {
            V2BlockSyncError::HistoricalBodyService(
                "historical-body completion underflowed outstanding work".into(),
            )
        })?;
        Ok(())
    }

    #[cfg(test)]
    fn release_principals(
        &mut self,
        first: &PeerId,
        second: Option<&PeerId>,
    ) -> Result<(), V2BlockSyncError> {
        let charges = self.single_route_test_charges(first, second);
        self.release_charges(&charges)
    }

    #[cfg(test)]
    fn single_route_test_charges(
        &self,
        first: &PeerId,
        second: Option<&PeerId>,
    ) -> HistoricalBodyAdmissionCharges {
        let mut principals = BTreeMap::from([(first.clone(), self.admission_charge)]);
        if let Some(second) = second {
            principals.insert(second.clone(), self.admission_charge);
        }
        HistoricalBodyAdmissionCharges {
            global: self.admission_charge,
            principals,
        }
    }
}

#[derive(Clone)]
struct PrincipalState {
    bucket: RateBucket,
    outstanding: usize,
}

impl PrincipalState {
    fn new(rate: u64, burst: u64, now: Instant) -> Self {
        Self {
            bucket: RateBucket::new(rate, burst, now),
            outstanding: 0,
        }
    }

    fn can_reserve(&mut self, charge: u64, now: Instant) -> bool {
        self.outstanding.checked_add(1).is_some() && self.bucket.can_take(charge, now)
    }

    fn reserve(&mut self, charge: u64, now: Instant) -> Result<(), V2BlockSyncError> {
        if !self.bucket.take(charge, now) {
            return Err(V2BlockSyncError::HistoricalBodyService(
                "historical-body principal admission changed after preflight".into(),
            ));
        }
        self.outstanding = self.outstanding.checked_add(1).ok_or_else(|| {
            V2BlockSyncError::HistoricalBodyService(
                "historical-body principal admission changed after preflight".into(),
            )
        })?;
        Ok(())
    }

    fn release(&mut self) -> Result<(), V2BlockSyncError> {
        self.outstanding = self.outstanding.checked_sub(1).ok_or_else(|| {
            V2BlockSyncError::HistoricalBodyService(
                "historical-body principal outstanding state is inconsistent".into(),
            )
        })?;
        Ok(())
    }
}

#[derive(Clone)]
struct RateBucket {
    rate_per_second: u64,
    capacity: u64,
    tokens: u64,
    last_refill: Instant,
    refill_remainder: u128,
}

impl RateBucket {
    fn new(rate_per_second: u64, capacity: u64, now: Instant) -> Self {
        Self {
            rate_per_second,
            capacity,
            tokens: capacity,
            last_refill: now,
            refill_remainder: 0,
        }
    }

    fn refill(&mut self, now: Instant) {
        if self.tokens == self.capacity {
            self.last_refill = now;
            self.refill_remainder = 0;
            return;
        }
        let elapsed = now.saturating_duration_since(self.last_refill);
        let numerator = elapsed
            .as_nanos()
            .saturating_mul(u128::from(self.rate_per_second))
            .saturating_add(self.refill_remainder);
        let nanos_per_second = Duration::from_secs(1).as_nanos();
        let credit = numerator / nanos_per_second;
        let credit = u64::try_from(credit).unwrap_or(u64::MAX);
        self.tokens = self.capacity.min(self.tokens.saturating_add(credit));
        self.refill_remainder = if self.tokens == self.capacity {
            0
        } else {
            numerator % nanos_per_second
        };
        self.last_refill = now;
    }

    fn can_take(&mut self, amount: u64, now: Instant) -> bool {
        self.refill(now);
        self.tokens >= amount
    }

    fn take(&mut self, amount: u64, now: Instant) -> bool {
        if !self.can_take(amount, now) {
            return false;
        }
        self.tokens -= amount;
        true
    }

    fn is_full(&mut self, now: Instant) -> bool {
        self.refill(now);
        self.tokens == self.capacity
    }
}

#[derive(Clone)]
struct CachedHistoricalBodyResponse {
    responder: PeerId,
    message: NetworkMessage,
    proof: HistoricalBodyDurableSourceProof,
    retained_heap_bytes: usize,
}

#[derive(Clone, Copy)]
struct CachedHistoricalBodyRequestIdentity {
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    unsigned_request_hash: Hash,
}

struct HistoricalBodyResponseCache {
    network_id: NetworkId,
    capacity: usize,
    retained_heap_capacity: usize,
    responses: BTreeMap<HashOf<wire::CertifiedBodyRequest>, CachedHistoricalBodyResponse>,
    identities: BTreeMap<HistoricalBodyRequestIdentity, CachedHistoricalBodyRequestIdentity>,
    order: VecDeque<HashOf<wire::CertifiedBodyRequest>>,
    retained_heap_bytes: usize,
}

fn historical_body_response_retained_heap_charge(
    message: &NetworkMessage,
) -> Result<usize, V2BlockSyncError> {
    let NetworkMessage::SumeragiBlock(envelope) = message else {
        return Err(V2BlockSyncError::CorruptServerCache);
    };
    let encoded_frame_capacity = envelope.encoded_capacity().ok_or_else(|| {
        V2BlockSyncError::HistoricalBodyService(
            "historical-body worker lost preencoded response storage".into(),
        )
    })?;
    let BlockMessage::V2(message) = envelope.as_message() else {
        return Err(V2BlockSyncError::CorruptServerCache);
    };
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload else {
        return Err(V2BlockSyncError::CorruptServerCache);
    };
    let chunk_hash_bytes = response
        .manifest
        .chunk_hashes
        .capacity()
        .checked_mul(std::mem::size_of::<Hash>())
        .ok_or_else(|| {
            V2BlockSyncError::HistoricalBodyService(
                "historical-body decoded manifest heap charge overflowed".into(),
            )
        })?;
    encoded_frame_capacity
        .checked_add(response.body.capacity())
        .and_then(|bytes| bytes.checked_add(response.signature.capacity()))
        .and_then(|bytes| bytes.checked_add(chunk_hash_bytes))
        .and_then(|bytes| bytes.checked_add(FIRST_RELEASE_CACHE_ENTRY_FIXED_HEAP_BYTES))
        .ok_or_else(|| {
            V2BlockSyncError::HistoricalBodyService(
                "historical-body retained-heap charge overflowed".into(),
            )
        })
}

impl HistoricalBodyResponseCache {
    fn new(network_id: NetworkId, limits: HistoricalBodyServeLimits) -> Self {
        Self {
            network_id,
            capacity: limits.cache_entry_capacity.get(),
            retained_heap_capacity: limits.cache_retained_heap_capacity.get(),
            responses: BTreeMap::new(),
            identities: BTreeMap::new(),
            order: VecDeque::new(),
            retained_heap_bytes: 0,
        }
    }

    fn serve(
        &mut self,
        kura: &Kura,
        request: &wire::CertifiedBodyRequest,
        authenticated_requester: &PeerId,
        responder_key: &KeyPair,
    ) -> Result<Option<(NetworkMessage, HistoricalBodyDurableSourceProof)>, V2BlockSyncError> {
        authenticate_certified_body_request_identity(request, authenticated_requester)?;
        let request_hash = HashOf::new(request);
        let unsigned_request_hash = Hash::new(&request.signature_preimage());
        let responder = PeerId::new(responder_key.public_key().clone());
        let identity = HistoricalBodyRequestIdentity::from(request);
        if let Some(cached) = self.responses.get(&request_hash) {
            if cached.responder == responder {
                return Ok(Some((cached.message.clone(), cached.proof.clone())));
            }
        }
        if let Some(existing) = self.identities.get(&identity).copied() {
            if existing.unsigned_request_hash != unsigned_request_hash {
                return Err(V2BlockSyncError::ConflictingHistoricalBodyRequest {
                    existing: existing.request_hash,
                    incoming: request_hash,
                });
            }
            let cached = self
                .responses
                .get(&existing.request_hash)
                .ok_or(V2BlockSyncError::CorruptServerCache)?;
            let NetworkMessage::SumeragiBlock(envelope) = &cached.message else {
                return Err(V2BlockSyncError::CorruptServerCache);
            };
            let BlockMessage::V2(message) = envelope.as_message() else {
                return Err(V2BlockSyncError::CorruptServerCache);
            };
            let rebound = rebind_cached_historical_body_response(
                message,
                request_hash,
                responder.clone(),
                responder_key,
            )?;
            let (message, proof) = self.prepare(request, rebound)?;
            self.remove(existing.request_hash)?;
            return self.retain(
                request_hash,
                identity,
                unsigned_request_hash,
                responder,
                message,
                proof,
            );
        }
        if self.responses.contains_key(&request_hash) {
            return Err(V2BlockSyncError::CorruptServerCache);
        }
        let Some(response) = build_historical_body_response(
            kura,
            self.network_id,
            request.clone(),
            authenticated_requester,
            responder_key,
        )?
        else {
            return Ok(None);
        };
        let (message, proof) = self.prepare(request, response)?;
        self.retain(
            request_hash,
            identity,
            unsigned_request_hash,
            responder,
            message,
            proof,
        )
    }

    fn prepare(
        &self,
        request: &wire::CertifiedBodyRequest,
        response: wire::ConsensusMessageV2,
    ) -> Result<(NetworkMessage, HistoricalBodyDurableSourceProof), V2BlockSyncError> {
        let wire = BlockMessageWire::try_preencoded(Arc::new(BlockMessage::V2(response)))
            .map_err(|error| V2BlockSyncError::HistoricalBodyService(error.to_string()))?;
        let message = NetworkMessage::SumeragiBlock(Arc::new(wire));
        let _ = message.exact_output_hash();
        let proof = HistoricalBodyDurableSourceProof::mint(self.network_id, request, &message)?;
        Ok((message, proof))
    }

    #[allow(clippy::too_many_arguments)]
    fn retain(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        identity: HistoricalBodyRequestIdentity,
        unsigned_request_hash: Hash,
        responder: PeerId,
        message: NetworkMessage,
        proof: HistoricalBodyDurableSourceProof,
    ) -> Result<Option<(NetworkMessage, HistoricalBodyDurableSourceProof)>, V2BlockSyncError> {
        let retained_heap_bytes = historical_body_response_retained_heap_charge(&message)?;
        if retained_heap_bytes > self.retained_heap_capacity {
            return Ok(Some((message, proof)));
        }
        while self.responses.len() >= self.capacity
            || self
                .retained_heap_bytes
                .checked_add(retained_heap_bytes)
                .is_none_or(|retained| retained > self.retained_heap_capacity)
        {
            let oldest = self
                .order
                .pop_front()
                .ok_or(V2BlockSyncError::CorruptServerCache)?;
            self.remove(oldest)?;
        }
        self.retained_heap_bytes = self
            .retained_heap_bytes
            .checked_add(retained_heap_bytes)
            .ok_or(V2BlockSyncError::CorruptServerCache)?;
        self.responses.insert(
            request_hash,
            CachedHistoricalBodyResponse {
                responder,
                message: message.clone(),
                proof: proof.clone(),
                retained_heap_bytes,
            },
        );
        self.identities.insert(
            identity,
            CachedHistoricalBodyRequestIdentity {
                request_hash,
                unsigned_request_hash,
            },
        );
        self.order.push_back(request_hash);
        Ok(Some((message, proof)))
    }

    fn remove(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> Result<(), V2BlockSyncError> {
        let cached = self
            .responses
            .remove(&request_hash)
            .ok_or(V2BlockSyncError::CorruptServerCache)?;
        self.retained_heap_bytes = self
            .retained_heap_bytes
            .checked_sub(cached.retained_heap_bytes)
            .ok_or(V2BlockSyncError::CorruptServerCache)?;
        self.identities
            .retain(|_, cached| cached.request_hash != request_hash);
        self.order.retain(|cached| *cached != request_hash);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::block::{BlockHeader, consensus_v2::HeightContext};
    use iroha_p2p::network::NetworkReplyRouteTestFixture;

    fn typed_hash<T>(label: &[u8]) -> HashOf<T> {
        HashOf::from_untyped_unchecked(Hash::new(label))
    }

    fn peer() -> PeerId {
        PeerId::new(KeyPair::random().public_key().clone())
    }

    fn admission_test_limits() -> HistoricalBodyServeLimits {
        HistoricalBodyServeLimits {
            task_queue_capacity: NonZeroUsize::new(2).expect("non-zero test queue"),
            max_reply_route_fanout: NonZeroUsize::new(4).expect("non-zero reply fanout"),
            cache_entry_capacity: NonZeroUsize::new(1).expect("non-zero test cache"),
            cache_retained_heap_capacity: NonZeroUsize::new(1).expect("non-zero test cache heap"),
            principal_state_capacity: NonZeroUsize::new(10).expect("non-zero principal cap"),
            global_bytes_per_second: 20,
            global_burst_bytes: 20,
            principal_bytes_per_second: 10,
            principal_burst_bytes: 10,
            admission_charge_bytes: 10,
        }
    }

    #[test]
    fn rate_bucket_refills_but_never_exceeds_capacity() {
        let start = Instant::now();
        let mut bucket = RateBucket::new(10, 20, start);
        assert!(bucket.take(20, start));
        assert!(!bucket.take(1, start));
        assert!(bucket.take(10, start + Duration::from_secs(1)));
        assert!(!bucket.take(1, start + Duration::from_secs(1)));
        assert!(bucket.is_full(start + Duration::from_secs(4)));
    }

    #[test]
    fn rate_bucket_preserves_fractional_credit_across_frequent_checks() {
        let start = Instant::now();
        let mut bucket = RateBucket::new(10, 10, start);
        assert!(bucket.take(10, start));
        for tenth in 1..10 {
            assert!(!bucket.can_take(
                10,
                start + Duration::from_millis(u64::try_from(tenth * 100).expect("small fixture")),
            ));
        }
        assert!(bucket.take(10, start + Duration::from_secs(1)));
    }

    #[test]
    fn first_release_limits_reserve_maximum_supported_fanout() {
        let reply_route_source_capacity = 5;
        let limits = HistoricalBodyServeLimits::first_release(1, reply_route_source_capacity)
            .expect("valid configured limits");
        assert_eq!(
            limits.admission_charge_bytes,
            u64::try_from(FIRST_RELEASE_MAX_RESPONSE_FRAME_BYTES).expect("fixed cap fits u64")
        );
        let maximum_fanout_charge = limits
            .admission_charge_bytes
            .checked_mul(
                u64::try_from(limits.max_reply_route_fanout.get()).expect("fixed fanout fits u64"),
            )
            .expect("fixed fanout charge fits u64");
        assert_eq!(limits.global_burst_bytes, maximum_fanout_charge);
        assert_eq!(limits.principal_burst_bytes, maximum_fanout_charge);
        assert_eq!(limits.max_reply_route_fanout.get(), 5);
        assert_eq!(limits.task_queue_capacity.get(), 2);
        assert_eq!(limits.principal_state_capacity.get(), 12);

        let requester = peer();
        let mut route_fixture =
            NetworkReplyRouteTestFixture::with_source_capacity(peer(), reply_route_source_capacity);
        let first_route = route_fixture.mint_via(requester.clone(), peer());
        let mut reply_routes = NetworkReplyRoutes::try_from_route(first_route)
            .expect("start configured reply-route set");
        for _ in 1..reply_route_source_capacity {
            let candidate = NetworkReplyRoutes::try_from_route(
                route_fixture.mint_via(requester.clone(), peer()),
            )
            .expect("construct independent configured reply source");
            reply_routes
                .merge(&candidate)
                .expect("merge configured independent reply source");
        }
        let plan = HistoricalBodyAdmissionPlan::from_reply_routes(&reply_routes)
            .expect("derive admission from opaque configured routes");
        assert_eq!(plan.active_route_count, 5);
        let charges = plan
            .charges(
                &requester,
                limits.admission_charge_bytes,
                limits.max_reply_route_fanout.get(),
            )
            .expect("maximum supported fanout has a valid charge")
            .expect("maximum supported fanout is non-empty");
        assert!(
            HistoricalBodyAdmissionState::new(limits)
                .try_reserve_charges(&charges, Instant::now())
                .expect("production limits reserve maximum supported fanout")
        );

        let rejected = HistoricalBodyAdmissionPlan {
            active_route_count: limits.max_reply_route_fanout.get() + 1,
            active_route_sources: BTreeSet::from([
                requester,
                peer(),
                peer(),
                peer(),
                peer(),
                peer(),
            ]),
            reply_route_source_capacity,
        };
        assert!(
            rejected
                .charges(
                    rejected
                        .active_route_sources
                        .first()
                        .expect("non-empty oversized fanout"),
                    limits.admission_charge_bytes,
                    limits.max_reply_route_fanout.get(),
                )
                .expect("over-bound remote geometry is not a local service failure")
                .is_none(),
            "fanout above the configured source bound must be rejected nonfatally"
        );

        assert!(HistoricalBodyServeLimits::first_release(1, 0).is_err());
        assert!(HistoricalBodyServeLimits::first_release(1, usize::MAX).is_err());
    }

    #[test]
    fn mismatched_reply_source_geometry_is_rejected_nonfatally() {
        let requester = peer();
        let plan = HistoricalBodyAdmissionPlan {
            reply_route_source_capacity: 5,
            active_route_count: 1,
            active_route_sources: BTreeSet::from([requester.clone()]),
        };

        assert!(
            plan.charges(&requester, 10, 4)
                .expect("foreign route geometry is a remote admission rejection")
                .is_none()
        );
    }

    #[test]
    fn admission_bounds_global_and_per_principal_work_over_time() {
        let start = Instant::now();
        let first = peer();
        let second = peer();
        let third = peer();
        let mut admission = HistoricalBodyAdmissionState::new(admission_test_limits());

        assert!(
            admission
                .try_reserve_principals(&first, None, start)
                .expect("admit first principal")
        );
        admission
            .release_principals(&first, None)
            .expect("release first principal");
        assert!(
            !admission
                .try_reserve_principals(&first, None, start)
                .expect("rate-limit immediate repeat")
        );
        assert!(
            admission
                .try_reserve_principals(&second, None, start)
                .expect("admit independent principal")
        );
        admission
            .release_principals(&second, None)
            .expect("release second principal");
        assert!(
            !admission
                .try_reserve_principals(&third, None, start)
                .expect("global burst is exhausted")
        );

        let refilled = start + Duration::from_secs(1);
        assert!(
            admission
                .try_reserve_principals(&first, None, refilled)
                .expect("first principal refills")
        );
        assert!(
            admission
                .try_reserve_principals(&second, None, refilled)
                .expect("second principal refills")
        );
        assert!(
            !admission
                .try_reserve_principals(&third, None, refilled)
                .expect("outstanding work remains globally bounded")
        );
    }

    #[test]
    fn relayed_request_charges_transport_and_signed_requester() {
        let start = Instant::now();
        let via = peer();
        let requester = peer();
        let mut admission = HistoricalBodyAdmissionState::new(admission_test_limits());

        assert!(
            admission
                .try_reserve_principals(&via, Some(&requester), start)
                .expect("admit relayed request")
        );
        admission
            .release_principals(&via, Some(&requester))
            .expect("release relayed request");
        assert!(
            !admission
                .try_reserve_principals(&via, None, start)
                .expect("transport source remains charged")
        );
        assert!(
            !admission
                .try_reserve_principals(&requester, None, start)
                .expect("signed requester remains charged")
        );
    }

    #[test]
    fn fanout_reservation_charges_all_routes_and_releases_all_principals() {
        let start = Instant::now();
        let requester = peer();
        let first_source = peer();
        let second_source = peer();
        let mut limits = admission_test_limits();
        limits.global_bytes_per_second = 100;
        limits.global_burst_bytes = 100;
        limits.principal_bytes_per_second = 100;
        limits.principal_burst_bytes = 100;
        let mut admission = HistoricalBodyAdmissionState::new(limits);
        let plan = HistoricalBodyAdmissionPlan {
            reply_route_source_capacity: limits.max_reply_route_fanout.get(),
            active_route_count: 3,
            active_route_sources: BTreeSet::from([
                requester.clone(),
                first_source.clone(),
                second_source.clone(),
            ]),
        };
        let charges = plan
            .charges(&requester, 10, limits.max_reply_route_fanout.get())
            .expect("derive checked three-route charges")
            .expect("three routes produce one reservation");
        assert_eq!(charges.global, 30);
        assert_eq!(charges.principals[&requester], 30);
        assert_eq!(charges.principals[&first_source], 10);
        assert_eq!(charges.principals[&second_source], 10);

        assert!(
            admission
                .try_reserve_charges(&charges, start)
                .expect("reserve exact three-route fanout")
        );
        assert_eq!(admission.global.tokens, 70);
        assert_eq!(admission.outstanding, 1);
        assert_eq!(
            admission
                .principals
                .get(&requester)
                .expect("requester principal")
                .bucket
                .tokens,
            70
        );
        for source in [&first_source, &second_source] {
            let state = admission
                .principals
                .get(source)
                .expect("route-source principal");
            assert_eq!(state.bucket.tokens, 90);
            assert_eq!(state.outstanding, 1);
        }

        admission
            .release_charges(&charges)
            .expect("release every fanout principal atomically");
        assert_eq!(admission.outstanding, 0);
        assert!(
            charges
                .principals
                .keys()
                .all(|peer| admission.principals[peer].outstanding == 0)
        );
        assert_eq!(admission.global.tokens, 70, "completed work stays charged");
    }

    #[test]
    fn shared_fanout_principals_release_one_outstanding_request_at_a_time() {
        let start = Instant::now();
        let requester = peer();
        let source = peer();
        let mut limits = admission_test_limits();
        limits.global_bytes_per_second = 100;
        limits.global_burst_bytes = 100;
        limits.principal_bytes_per_second = 100;
        limits.principal_burst_bytes = 100;
        let mut admission = HistoricalBodyAdmissionState::new(limits);
        let charges = HistoricalBodyAdmissionCharges {
            global: 20,
            principals: BTreeMap::from([(requester.clone(), 20), (source.clone(), 10)]),
        };

        assert!(
            admission
                .try_reserve_charges(&charges, start)
                .expect("reserve first shared-principal request")
        );
        assert!(
            admission
                .try_reserve_charges(&charges, start)
                .expect("reserve second shared-principal request")
        );
        assert_eq!(admission.outstanding, 2);
        assert_eq!(admission.principals[&requester].outstanding, 2);
        assert_eq!(admission.principals[&source].outstanding, 2);

        admission
            .release_charges(&charges)
            .expect("release one shared-principal request");
        assert_eq!(admission.outstanding, 1);
        assert_eq!(admission.principals[&requester].outstanding, 1);
        assert_eq!(admission.principals[&source].outstanding, 1);
        admission
            .release_charges(&charges)
            .expect("release final shared-principal request");
        assert_eq!(admission.outstanding, 0);
        assert_eq!(admission.principals[&requester].outstanding, 0);
        assert_eq!(admission.principals[&source].outstanding, 0);
    }

    #[test]
    fn fanout_charge_overflow_fails_before_reservation() {
        let requester = peer();
        let relay = peer();
        let plan = HistoricalBodyAdmissionPlan {
            reply_route_source_capacity: 4,
            active_route_count: 2,
            active_route_sources: BTreeSet::from([requester.clone(), relay]),
        };

        assert!(plan.charges(&requester, u64::MAX, 4).is_err());
    }

    #[test]
    fn inactive_reply_routes_produce_no_worker_reservation() {
        let requester = peer();
        let plan = HistoricalBodyAdmissionPlan {
            reply_route_source_capacity: 4,
            active_route_count: 0,
            active_route_sources: BTreeSet::new(),
        };

        assert!(
            plan.charges(&requester, 10, 4)
                .expect("zero-route plan is structurally valid")
                .is_none()
        );
    }

    #[test]
    fn rejected_fanout_reservation_does_not_partially_charge_or_insert_principals() {
        let start = Instant::now();
        let requester = peer();
        let available_source = peer();
        let exhausted_source = peer();
        let mut limits = admission_test_limits();
        limits.global_bytes_per_second = 100;
        limits.global_burst_bytes = 100;
        limits.principal_bytes_per_second = 100;
        limits.principal_burst_bytes = 100;
        let mut admission = HistoricalBodyAdmissionState::new(limits);
        let mut exhausted = PrincipalState::new(100, 100, start);
        assert!(exhausted.bucket.take(100, start));
        admission
            .principals
            .insert(exhausted_source.clone(), exhausted);
        let charges = HistoricalBodyAdmissionCharges {
            global: 30,
            principals: BTreeMap::from([
                (requester.clone(), 30),
                (available_source.clone(), 10),
                (exhausted_source.clone(), 10),
            ]),
        };

        assert!(
            !admission
                .try_reserve_charges(&charges, start)
                .expect("reject fanout with one exhausted source")
        );
        assert_eq!(admission.global.tokens, 100);
        assert_eq!(admission.outstanding, 0);
        assert_eq!(admission.principals.len(), 1);
        assert!(!admission.principals.contains_key(&requester));
        assert!(!admission.principals.contains_key(&available_source));
        let exhausted = admission
            .principals
            .get(&exhausted_source)
            .expect("exhausted principal remains exact");
        assert_eq!(exhausted.bucket.tokens, 0);
        assert_eq!(exhausted.outstanding, 0);
    }

    #[test]
    fn failed_fanout_release_preserves_every_other_outstanding_owner() {
        let start = Instant::now();
        let requester = peer();
        let first_source = peer();
        let second_source = peer();
        let mut limits = admission_test_limits();
        limits.global_bytes_per_second = 100;
        limits.global_burst_bytes = 100;
        limits.principal_bytes_per_second = 100;
        limits.principal_burst_bytes = 100;
        let mut admission = HistoricalBodyAdmissionState::new(limits);
        let charges = HistoricalBodyAdmissionCharges {
            global: 30,
            principals: BTreeMap::from([
                (requester.clone(), 30),
                (first_source.clone(), 10),
                (second_source.clone(), 10),
            ]),
        };
        assert!(
            admission
                .try_reserve_charges(&charges, start)
                .expect("reserve exact three-route fanout")
        );
        admission
            .principals
            .get_mut(&second_source)
            .expect("second route source")
            .outstanding = 0;

        assert!(admission.release_charges(&charges).is_err());
        assert_eq!(admission.outstanding, 1);
        assert_eq!(admission.principals[&requester].outstanding, 1);
        assert_eq!(admission.principals[&first_source].outstanding, 1);
        assert_eq!(admission.principals[&second_source].outstanding, 0);
    }

    #[test]
    fn relayed_admission_replaces_idle_principals_atomically() {
        let start = Instant::now();
        let mut peers = (0..4).map(|_| peer()).collect::<Vec<_>>();
        peers.sort();
        let first = peers[0].clone();
        let second = peers[1].clone();
        let old_first = peers[2].clone();
        let old_second = peers[3].clone();
        let mut limits = admission_test_limits();
        limits.principal_state_capacity =
            NonZeroUsize::new(2).expect("non-zero principal capacity");
        let mut admission = HistoricalBodyAdmissionState::new(limits);
        let principal_rate = admission.principal_rate;
        let principal_burst = admission.principal_burst;
        admission.principals.insert(
            old_first.clone(),
            PrincipalState::new(principal_rate, principal_burst, start),
        );
        admission.principals.insert(
            old_second.clone(),
            PrincipalState::new(principal_rate, principal_burst, start),
        );

        assert!(
            admission
                .try_reserve_principals(&first, Some(&second), start)
                .expect("replace both idle principals and admit the relay pair")
        );
        assert_eq!(admission.principals.len(), 2);
        assert!(admission.principals.contains_key(&first));
        assert!(admission.principals.contains_key(&second));
        assert!(!admission.principals.contains_key(&old_first));
        assert!(!admission.principals.contains_key(&old_second));
        admission
            .release_principals(&first, Some(&second))
            .expect("release atomically admitted relay pair");
    }

    #[test]
    fn worker_cache_remove_keeps_fifo_indexes_exact() {
        let network_id = NetworkId::from_genesis_hash(typed_hash::<BlockHeader>(b"network"));
        let limits =
            HistoricalBodyServeLimits::first_release(2, 5).expect("valid configured limits");
        let mut cache = HistoricalBodyResponseCache::new(network_id, limits);
        let responder = PeerId::new(KeyPair::random().public_key().clone());
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(typed_hash::<HeightContext>(b"context")),
            height: 1,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: typed_hash::<BlockHeader>(b"block"),
            payload_hash: Hash::new(b"payload"),
        };
        let request_hash = typed_hash::<wire::CertifiedBodyRequest>(b"request");
        let message = NetworkMessage::Health;
        let proof = HistoricalBodyDurableSourceProof {
            network_id,
            source_round: round,
            source_subject: subject,
            responder: responder.clone(),
            request_hash,
            exact_output_hash: HashOf::new(&message),
        };
        let identity = HistoricalBodyRequestIdentity {
            round,
            subject,
            requester: responder.clone(),
        };
        cache.responses.insert(
            request_hash,
            CachedHistoricalBodyResponse {
                responder,
                message,
                proof,
                retained_heap_bytes: 7,
            },
        );
        cache.identities.insert(
            identity,
            CachedHistoricalBodyRequestIdentity {
                request_hash,
                unsigned_request_hash: Hash::new(b"unsigned request"),
            },
        );
        cache.order.push_back(request_hash);
        cache.retained_heap_bytes = 7;

        cache.remove(request_hash).expect("remove exact cache row");

        assert!(cache.responses.is_empty());
        assert!(cache.identities.is_empty());
        assert!(cache.order.is_empty());
        assert_eq!(cache.retained_heap_bytes, 0);
    }

    #[test]
    fn durable_proof_accepts_only_its_worker_warmed_exact_hash() {
        let network_id = NetworkId::from_genesis_hash(typed_hash::<BlockHeader>(b"network"));
        let responder = PeerId::new(KeyPair::random().public_key().clone());
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(typed_hash::<HeightContext>(b"context")),
            height: 1,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: typed_hash::<BlockHeader>(b"block"),
            payload_hash: Hash::new(b"payload"),
        };
        let request_hash = typed_hash::<wire::CertifiedBodyRequest>(b"request");
        let message = NetworkMessage::SumeragiBlock(Arc::new(
            BlockMessageWire::try_preencoded(Arc::new(BlockMessage::V2(
                wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CertifiedBodyResponse(
                        wire::CertifiedBodyResponse {
                            request_hash,
                            manifest: wire::PayloadManifest {
                                round,
                                subject,
                                payload_size_bytes: 1,
                                layout: wire::DataAvailabilityLayout {
                                    encoding: wire::PayloadEncoding::ReedSolomon16,
                                    chunk_size_bytes: 1,
                                    data_shards: 1,
                                    parity_shards: 1,
                                    max_payload_size_bytes: 1,
                                    max_chunk_count: 2,
                                },
                                chunk_hashes: vec![Hash::new(b"chunk")],
                                chunk_root: Hash::new(b"root"),
                            },
                            body: vec![0xA5],
                            responder: responder.clone(),
                            signature: vec![0x5A],
                        },
                    ),
                ),
            )))
            .expect("preencode body response"),
        ));
        let proof = HistoricalBodyDurableSourceProof {
            network_id,
            source_round: round,
            source_subject: subject,
            responder,
            request_hash,
            exact_output_hash: HashOf::new(&message),
        };

        assert_eq!(message.cached_exact_output_hash(), None);
        assert!(!proof.covers_message_in_network(&network_id, &message));
        assert_eq!(message.exact_output_hash(), proof.exact_output_hash);
        assert!(proof.covers_message_in_network(&network_id, &message));
        let NetworkMessage::SumeragiBlock(envelope) = &message else {
            panic!("fixture is block traffic")
        };
        let encoded_capacity = envelope
            .encoded_capacity()
            .expect("worker-warmed fixture retains its encoded frame");
        let BlockMessage::V2(v2) = envelope.as_message() else {
            panic!("fixture is v2 traffic")
        };
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &v2.payload else {
            panic!("fixture is a body response")
        };
        let decoded_dynamic_capacity = response
            .body
            .capacity()
            .checked_add(response.signature.capacity())
            .and_then(|bytes| {
                bytes.checked_add(
                    response
                        .manifest
                        .chunk_hashes
                        .capacity()
                        .checked_mul(std::mem::size_of::<Hash>())?,
                )
            })
            .expect("fixture retained-heap charge fits usize");
        assert!(
            historical_body_response_retained_heap_charge(&message)
                .expect("charge worker-warmed response")
                >= encoded_capacity
                    .checked_add(decoded_dynamic_capacity)
                    .and_then(|bytes| {
                        bytes.checked_add(FIRST_RELEASE_CACHE_ENTRY_FIXED_HEAP_BYTES)
                    })
                    .expect("fixture total retained-heap charge fits usize"),
            "cache accounting must charge both the encoded frame and decoded response"
        );

        let mut changed = message.clone();
        let NetworkMessage::SumeragiBlock(envelope) = &mut changed else {
            panic!("fixture is block traffic")
        };
        let BlockMessage::V2(v2) = Arc::make_mut(envelope).make_mut() else {
            panic!("fixture is v2 traffic")
        };
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &mut v2.payload
        else {
            panic!("fixture is a body response")
        };
        response.body.push(0xFF);
        assert_eq!(changed.cached_exact_output_hash(), None);
        assert!(!proof.covers_message_in_network(&network_id, &changed));
    }
}
