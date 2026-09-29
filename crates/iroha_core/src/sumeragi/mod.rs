//! Translates to Emperor. Consensus-related logic of Iroha.
//!
//! `Consensus` trait is now implemented only by `Sumeragi` for now.
use crate::{
    merge_sidecar::{CertifiedMergeSidecarMessage, MAX_CERTIFIED_MERGE_CHUNK_BYTES},
    state::{State, StateView, WorldReadOnly},
};
use eyre::Result;
use iroha_config::parameters::{
    actual::{Common as CommonConfig, Sumeragi as SumeragiConfig},
    defaults::sumeragi::{
        BODY_ENVELOPE_HEADROOM_BYTES, CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
        TIMEOUT_VOTE_RESERVE_BYTES,
    },
};
use iroha_crypto::{Hash as CryptoHash, HashOf, PublicKey};
use iroha_data_model::{
    NetworkId,
    block::{
        consensus::Evidence,
        consensus_v2::{
            BlockSubject, ConsensusMessageV2, ConsensusMessageV2Payload, ConsensusMode,
            ConsensusRound,
        },
    },
    merge::{
        MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES, MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES,
        MergeCommitteeSignature,
    },
    nexus::LaneRelayEnvelope,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal, try_spawn_os_thread_as_future};
use iroha_genesis::GenesisBlock;
use iroha_model_base::peer::PeerId;
use iroha_p2p::network::{
    NetworkReplyRoute, NetworkReplyRouteError, NetworkReplyRouteSourceUpdate, NetworkReplyRoutes,
    NetworkReplyRoutesObservedMergeReceipt, NetworkReplyRoutesPruneReceipt,
    NetworkReplyRoutesStrictMergeReceipt,
};
use norito::codec::{Decode, Encode};
use parking_lot::Mutex;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const _: () = assert!(TIMEOUT_VOTE_RESERVE_BYTES >= MAX_VALID_TIMEOUT_VOTE_WIRE_BYTES);
// A maximal Kagemusha V1 CommitQC carries the ordinary BLS aggregate plus
// the exact 2f + 1 paired-Pasta seal bundle for the bounded 31-validator roster.
const _: () =
    assert!(iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES == 16 * 1024);
/// Native source-complete Pasta Commit attestations.
pub mod attestation;
/// The driver's block store over Kura: one certified `SignedBlockWire` frame per height.
pub mod block_store;
/// File-backed body store of the Sumeragi driver (bodies of accepted, unapplied blocks).
pub mod bodies;
/// The certified-chain reader: committed blocks as their Kura frames certify them.
pub mod certified_chain;
/// The execution result `R` of a block (`specs/sumeragi.md` §4.1).
pub mod commitment;
/// QC-based consensus message types and helpers (single-chain).
pub mod consensus;
/// Production cryptography of the Sumeragi driver: `H = iroha_crypto::Hash`, BLS-normal
/// signatures with admitted proofs of possession, and the node's signer.
pub mod crypto;
/// The node driver of the sans-IO Sumeragi core (`iroha_sumeragi`), not yet started by the
/// node (the v2 runtime still runs until the cutover).
pub mod driver;
/// The lag-2 height-configuration schedule and the genesis committee (`specs/sumeragi.md` §10).
pub(crate) mod epoch;
pub(crate) mod epoch_beacon;
pub(crate) mod epoch_election;
/// The node's executor: executes, applies and builds blocks on the committed State.
pub mod executor;
/// Portable proofs and challenge-bound current-node finality statements.
pub mod finality;
/// Lanes of the global chain: identity, pinned configuration, batches and admission.
pub mod lanes;
/// Bounded canonical native journals for offline operators and qualification.
pub mod native_journal;
/// The Sumeragi driver's P2P transport: the frame envelope, traffic classes, egress and
/// ingress.
pub mod net;
pub mod network_topology;
/// The node's Sumeragi instance: startup, production backends and the driver.
pub mod node;
pub(crate) mod output_guard;
/// Nonempty block payloads and the leader's proposal builder.
pub mod payload;
/// File-backed safety records, store id and installation log of the Sumeragi driver (§7.4).
pub mod records;
pub mod schedule;
/// Startup: genesis apply and replay, and the core's `Init`.
pub mod startup;
/// A certified test chain: real genesis, execution and BLS-certified Kura frames.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub mod test_chain;
pub use genesis_meta::{
    staged_genesis_execution_policy_hash, staged_genesis_nexus_amx_context_hash,
};
/// The initial validator roster: the authenticated subset of the configured trusted peers.
pub mod roster;
pub use roster::filter_validators_from_trusted;
/// Named Sumeragi threads with an explicit, configured stack-size budget.
pub(crate) mod threads;
pub use threads::set_sumeragi_stack_size_bytes;
// Certified-Serve durability belongs to the production lifecycle coordinator and ledger.
mod genesis_merge;
pub use genesis_merge::{
    GenesisMergeAuthority, GenesisMergeAuthorityError, freeze_genesis_merge_authority,
};
pub use v2_context::{
    GenesisV2Bootstrap, V2GenesisBootstrapError, freeze_staged_genesis_v2,
    validate_signed_genesis_v2_authority,
};
pub use v2_core::{
    CheckedProductionTransition, ProductionTwoStageRelayRetryTraceProjection,
    check_production_two_stage_relay_retry_transition,
    production_two_stage_relay_retry_trace_refines_source_fairness_kernel,
};
pub use v2_recovery::{
    AuthenticatedV2SnapshotStartup, V2SnapshotStartupPolicy, V2StartupReplayError,
    V2StartupReplayPlan, authenticate_v2_snapshot_replay_boundary,
    authenticate_v2_snapshot_startup, authenticated_v2_snapshot_startup_mode,
    plan_v2_startup_replay,
};
pub use v2_evidence::EvidenceValidationContext;
pub use v2_evidence::evidence_subject_height_view;
#[cfg(not(test))]
use self::output_guard::process_consensus_output_guard;
use self::{message::*, output_guard::ConsensusOutputGuard};
use crate::{EventsSender, IrohaNetwork, kura::Kura, queue::Queue};
impl InboundBlockMessage {
    /// Build one message delivered directly by an authenticated transport peer.
    pub(crate) fn from_authenticated_peer(message: BlockMessage, sender: PeerId) -> Self {
        Self {
            message,
            via: sender.clone(),
            sender,
            reply_routes: None,
            ingress_ownership: None,
        }
    }
    /// Build one transport message while preserving a relayed protocol origin.
    ///
    /// `sender` remains visible to consensus validation and response routing;
    /// `via` is the authenticated hop charged for every bounded ingress owner.
    #[cfg(test)]
    fn from_transport(message: BlockMessage, sender: PeerId, via: PeerId) -> Self {
        Self {
            message,
            sender,
            via,
            reply_routes: None,
            ingress_ownership: None,
        }
    }
    /// Normalize one transport message and retain its exact authenticated return route.
    ///
    /// # Errors
    ///
    /// Returns the precise route-capability error when the route is inactive,
    /// addresses another semantic sender, belongs to another authenticated
    /// delivery peer, or cannot form a bounded route set.
    pub fn try_from_transport_with_reply_route(
        message: BlockMessage,
        sender: PeerId,
        via: PeerId,
        reply_route: NetworkReplyRoute,
    ) -> Result<Self, NetworkReplyRouteError> {
        if reply_route.semantic_target() != &sender {
            return Err(NetworkReplyRouteError::Retargeted);
        }
        if !reply_route.is_authenticated_via(&via) {
            return Err(NetworkReplyRouteError::DifferentSource);
        }
        let reply_routes = NetworkReplyRoutes::try_from_route(reply_route)?;
        Ok(Self {
            message,
            sender,
            via,
            reply_routes: Some(reply_routes),
            ingress_ownership: None,
        })
    }
    /// Consume the envelope and return the normalized message and semantic origin.
    #[cfg(test)]
    pub(crate) fn into_message_and_sender(self) -> (BlockMessage, PeerId) {
        (self.message, self.sender)
    }
    /// Consume the envelope without losing its local-only authenticated reply authority.
    pub(crate) fn into_message_sender_and_reply_routes(
        self,
    ) -> (BlockMessage, PeerId, Option<NetworkReplyRoutes>) {
        (self.message, self.sender, self.reply_routes)
    }
    /// Borrow the bounded fair-ingress ownership carrier attached at
    /// `FairV2Ingress::try_push_at`.
    pub(crate) const fn ingress_ownership(&self) -> Option<&FairV2IngressOwnershipEvidence> {
        self.ingress_ownership.as_ref()
    }
    /// Move the exact fair-ingress ownership carrier into the downstream
    /// runtime-admission bridge without exposing or serializing capabilities.
    pub(crate) fn take_ingress_ownership(&mut self) -> Option<FairV2IngressOwnershipEvidence> {
        self.ingress_ownership.take()
    }
    /// Borrow the normalized message without removing it from its ingress lane.
    ///
    /// The serialized runner uses this view to make downstream admission and
    /// fair-ingress removal one atomic operation.
    pub(crate) fn message(&self) -> &BlockMessage {
        &self.message
    }
    /// Borrow the authenticated semantic protocol origin.
    pub(crate) const fn sender(&self) -> &PeerId {
        &self.sender
    }
    /// Borrow the exact authenticated return-route set before fair removal.
    ///
    /// The serialized v2 runner uses this only to validate and reserve an
    /// exact certified-body Serve lifecycle while this ingress owner is still
    /// locked in its source lane.
    pub(crate) fn reply_routes(&self) -> Option<&NetworkReplyRoutes> {
        self.reply_routes.as_ref()
    }
    /// Borrow the authenticated transport hop used for resource isolation.
    pub(crate) const fn via(&self) -> &PeerId {
        &self.via
    }
}
impl FairV2IngressSource {
    const fn is_native(&self) -> bool {
        matches!(self, Self::Native(_))
    }
    const fn uses_authenticated_capacity(&self) -> bool {
        matches!(self, Self::Authenticated(_) | Self::Native(_))
    }
    const fn class(&self) -> FairV2IngressSourceClass {
        match self {
            Self::Validator(_) => FairV2IngressSourceClass::Validator,
            Self::Authenticated(_) | Self::Native(_) => FairV2IngressSourceClass::Authenticated,
        }
    }
}

impl FairV2IngressHistoryServeRequest {
    const fn height(self) -> u64 {
        self.height
    }
    fn matches_configured_network(self, configured_network_id: Option<&NetworkId>) -> bool {
        self.required_network_id
            .is_none_or(|network_id| configured_network_id == Some(&network_id))
    }
}
impl FairV2IngressLeaderWireStatus {
    const fn blocks_replacement(self) -> bool {
        matches!(self, Self::Dormant | Self::Ingress | Self::Runtime)
    }
}
impl FairV2IngressOwnershipAction {
    const COUNT: usize = 5;
    const fn index(self) -> usize {
        match self {
            Self::New => 0,
            Self::ExactDuplicate => 1,
            Self::SameSourceLaterDelivery => 2,
            Self::Reconnect => 3,
            Self::NewAlternateSource => 4,
        }
    }
}
impl FairV2IngressCanonicalWire {
    fn decode(encoded_bytes: Arc<[u8]>, message_kind: FairV2IngressMessageKind) -> Option<Self> {
        let mut cursor = encoded_bytes.as_ref();
        let message = if message_kind.is_v2() {
            BlockMessage::V2(ConsensusMessageV2::decode(&mut cursor).ok()?)
        } else {
            BlockMessage::decode(&mut cursor).ok()?
        };
        if !cursor.is_empty() || FairV2IngressMessageKind::classify(&message) != Some(message_kind)
        {
            return None;
        }
        Some(Self {
            hash: CryptoHash::new(encoded_bytes.as_ref()),
            encoded_bytes,
            message_kind,
            class: FairV2IngressClass::classify_message(&message),
            is_timeout_vote: fair_v2_ingress_message_is_timeout_vote(&message),
            is_certified_fence_escape: fair_v2_ingress_message_is_certified_fence_escape(&message),
        })
    }
}
impl FairV2IngressPeerIdentityEncodings {
    fn append_identity(&mut self, projection: &mut Vec<u8>, peer: &PeerId) {
        if let Some(encoded) = self.encoded_peers.get(peer) {
            fair_v2_ingress_append_encoded_peer_identity(projection, encoded);
            return;
        }
        let encoded = peer.encode();
        fair_v2_ingress_append_encoded_peer_identity(projection, &encoded);
        self.encoded_peers.insert(peer.clone(), encoded);
    }
}


impl FairV2IngressPushError {
    fn rejected(inbound: InboundBlockMessage, reason: FairV2IngressRejectReason) -> Self {
        Self::Rejected(FairV2IngressRejection { inbound, reason })
    }
}


impl FairV2IngressCheckedSelectionScope {
    const fn is_lifecycle_lane_local(&self) -> bool {
        matches!(self, Self::LifecycleLaneLocal { .. })
    }
}

use crate::snapshot::{StartupRecovery, StartupRecoveryPublisher, startup_recovery_channel};
pub use admission_capacity::{
    AdmissionCapacityUnavailableV1, AuthenticatedAdmissionCapacityV1, Rs16PayloadGeometryV1,
};
pub use admission_input::QueuePlanInputCapacityErrorV1;

impl SumeragiHandle {
    fn new(
        block: Arc<FairV2Ingress>,
        lane_relay: mpsc::SyncSender<LaneRelayMessage>,
        wake: mpsc::SyncSender<()>,
        ingress_ready: Arc<AtomicBool>,
        pending_queue_plan_admission_dirty: Arc<AtomicBool>,
        output_guard: Arc<ConsensusOutputGuard>,
        startup_recovery: StartupRecovery,
    ) -> Self {
        Self {
            block,
            lane_relay,
            wake,
            ingress_ready,
            pending_queue_plan_admission_dirty,
            output_guard,
            emergency_fast_disabled: false,
            startup_recovery,
            beacon_readiness: Arc::default(),
            admission_capacity: Arc::new(std::sync::OnceLock::new()),
        }
    }
    /// Construct a permanently closed consensus ingress without launching an
    /// OS thread or allocating production queue geometry.
    ///
    /// Emergency Fast mode is read-only for its entire process lifetime. The
    /// disabled marker terminally classifies every owned ingress as
    /// [`SumeragiIngressDisposition::Obsolete`] and prevents queue-plan wake
    /// publication while preserving the ordinary handle type expected by P2P
    /// and Torii wiring.
    #[must_use]
    pub fn emergency_fast_disabled() -> Self {
        let block = Arc::new(
            FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, None,
            ),
        );
        let (lane_relay, lane_relay_rx) = mpsc::sync_channel(0);
        let (wake, wake_rx) = mpsc::sync_channel(0);
        drop(lane_relay_rx);
        drop(wake_rx);
        let mut handle = Self::new(
            block,
            lane_relay,
            wake,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            ConsensusOutputGuard::isolated(),
            StartupRecovery::unavailable(),
        );
        handle.emergency_fast_disabled = true;
        handle
    }
    fn wake(&self) {
        let _ = self.wake.try_send(());
    }
    /// Observe whether the current consensus owner accepts ordinary ingress.
    /// Startup replay, lifecycle activation and restart-required faults keep it closed.
    /// This readiness observation does not replace admission-time ownership checks.
    #[must_use]
    pub fn admission_ready(&self) -> bool {
        !self.emergency_fast_disabled
            && self.ingress_ready.load(Ordering::Acquire)
            && !self.restart_required()
    }
    /// Check production beacon readiness without closing bootstrap ingress.
    ///
    /// # Errors
    ///
    /// Returns a typed diagnostic while public key installation, exact provider
    /// custody, or a fresh lifecycle/state binding is unavailable.
    pub fn global_beacon_readiness(
        &self,
        state: &State,
    ) -> Result<(), crate::beacon::readiness::GlobalBeaconReadinessErrorV1> {
        self.beacon_readiness.check(state)
    }
    /// Observe the immutable signed RS16 layout after authenticated recovery.
    ///
    /// Pending recovery is an explicit retryable startup condition, never a
    /// default layout. This is only capacity evidence: callers must separately
    /// check live admission, State authority and the complete carrier envelope.
    pub fn authenticated_admission_capacity(
        &self,
    ) -> std::result::Result<AuthenticatedAdmissionCapacityV1, AdmissionCapacityUnavailableV1> {
        if self.emergency_fast_disabled {
            return Err(AdmissionCapacityUnavailableV1::Disabled);
        }
        if self.output_guard.restart_required() {
            return Err(AdmissionCapacityUnavailableV1::RestartRequired);
        }
        self.admission_capacity
            .get()
            .copied()
            .ok_or(AdmissionCapacityUnavailableV1::Pending)
    }
    /// Wake the serialized v2 owner after a QueuePlan admission certificate
    /// has been durably published in Kura.
    ///
    /// The certificate itself is never transferred through an in-memory
    /// channel: the runner re-reads bounded, hash-addressed Kura evidence when
    /// constructing the next canonical carrier. A saturated wake channel is
    /// already an equivalent outstanding notification.
    #[must_use]
    pub fn notify_pending_queue_plan_admission(&self) -> bool {
        let Some(_permit) = self.output_guard.acquire() else {
            return false;
        };
        self.pending_queue_plan_admission_dirty
            .store(true, Ordering::Release);
        if !self.ingress_ready.load(Ordering::Acquire) {
            return false;
        }
        self.wake();
        true
    }
    /// Try to transfer one exact normalized block envelope to the serialized owner.
    pub fn try_incoming_block_message_owned(
        &self,
        inbound: InboundBlockMessage,
    ) -> SumeragiIngressDisposition<InboundBlockMessage> {
        if self.emergency_fast_disabled {
            return SumeragiIngressDisposition::Obsolete;
        }
        let Some(permit) = self.output_guard.acquire() else {
            return SumeragiIngressDisposition::FailStop(inbound);
        };
        if !self.ingress_ready.load(Ordering::Acquire) {
            iroha_logger::debug!(
                "deferring Sumeragi ingress until context and safety WAL replay complete"
            );
            return SumeragiIngressDisposition::Retry(inbound);
        }
        let queue = crate::status::WorkerQueueKind::Blocks;
        match self.block.try_push(inbound) {
            Ok(FairV2IngressPushDisposition::Enqueued) => {
                crate::status::record_worker_queue_enqueue(queue);
                self.wake();
                SumeragiIngressDisposition::Accepted
            }
            Ok(FairV2IngressPushDisposition::Coalesced) => SumeragiIngressDisposition::Coalesced,
            Err(FairV2IngressPushError::Full(inbound)) => {
                iroha_logger::debug!(
                    ?queue,
                    "bounded per-source Sumeragi ingress queue is full; retaining caller ownership"
                );
                SumeragiIngressDisposition::Retry(inbound)
            }
            Err(FairV2IngressPushError::Closed(inbound)) => {
                iroha_logger::debug!(
                    ?queue,
                    "Sumeragi ingress queue closed during height rollover; retaining caller ownership"
                );
                SumeragiIngressDisposition::Retry(inbound)
            }
            Err(FairV2IngressPushError::FailStop(inbound)) => {
                iroha_logger::error!(
                    ?queue,
                    "durable Sumeragi ingress lifecycle failed; requiring process restart"
                );
                self.output_guard
                    .activate_restart_required_from_permit(permit);
                SumeragiIngressDisposition::FailStop(inbound)
            }
            Err(FairV2IngressPushError::Stale(inbound)) => {
                SumeragiIngressDisposition::Stale(inbound)
            }
            Err(FairV2IngressPushError::Rejected(rejection)) => {
                let message_kind = FairV2IngressMessageKind::classify(rejection.inbound.message());
                let round = fair_v2_ingress_consensus_round(rejection.inbound.message());
                iroha_logger::warn!(
                    ?queue,
                    reason = ?rejection.reason,
                    ?message_kind,
                    ?round,
                    semantic_origin = ?rejection.inbound.sender(),
                    authenticated_via = ?rejection.inbound.via(),
                    "permanently rejected Sumeragi ingress envelope"
                );
                SumeragiIngressDisposition::Rejected(rejection.inbound)
            }
        }
    }
    /// Try to enqueue a canonical message and preserve it on retryable pressure.
    pub fn try_incoming_block_message_from_owned(
        &self,
        sender: PeerId,
        message: BlockMessage,
    ) -> SumeragiIngressDisposition<InboundBlockMessage> {
        self.try_incoming_block_message_owned(InboundBlockMessage::from_authenticated_peer(
            message, sender,
        ))
    }
    /// Enqueue a canonical message from an authenticated transport peer.
    pub fn incoming_block_message_from(&self, sender: PeerId, message: BlockMessage) {
        let _ = self.try_incoming_block_message_from_owned(sender, message);
    }
    /// Try to enqueue a canonical message from an authenticated transport peer.
    pub fn try_incoming_block_message_from(&self, sender: PeerId, message: BlockMessage) -> bool {
        self.try_incoming_block_message_from_owned(sender, message)
            .accepted_or_coalesced()
    }
    /// Try to transfer one exact lane-relay item to its serialized owner.
    pub fn try_incoming_lane_relay_owned(
        &self,
        message: LaneRelayMessage,
    ) -> SumeragiIngressDisposition<LaneRelayMessage> {
        if self.emergency_fast_disabled {
            return SumeragiIngressDisposition::Obsolete;
        }
        let Some(permit) = self.output_guard.acquire() else {
            return SumeragiIngressDisposition::FailStop(message);
        };
        if !self.ingress_ready.load(Ordering::Acquire) {
            return SumeragiIngressDisposition::Retry(message);
        }
        // QueuePlan and Native drain have distinct process-lived owners for
        // this bounded channel. Returning every retired relay unchanged keeps
        // it from mistaking an enqueue for protocol admission.
        if !matches!(
            &message,
            LaneRelayMessage::QueuePlanAdmissionCertificate { .. }
                | LaneRelayMessage::DrainVote { .. }
        ) {
            return SumeragiIngressDisposition::Rejected(message);
        }
        if let LaneRelayMessage::QueuePlanAdmissionCertificate { certificate, .. } = &message
            && (certificate.is_empty()
                || certificate.len() > iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES)
        {
            iroha_logger::debug!(
                bytes = certificate.len(),
                "rejecting malformed QueuePlan admission certificate before lane ingress"
            );
            return SumeragiIngressDisposition::Rejected(message);
        }
        let send = match self
            .block
            .try_with_open_lane_relay_admission(message, |message| {
                self.lane_relay.try_send(message)
            }) {
            Ok(send) => send,
            Err(message) => return SumeragiIngressDisposition::Retry(message),
        };
        match send {
            Ok(()) => {
                crate::status::record_worker_queue_enqueue(
                    crate::status::WorkerQueueKind::LaneRelay,
                );
                self.wake();
                SumeragiIngressDisposition::Accepted
            }
            Err(mpsc::TrySendError::Full(message)) => {
                iroha_logger::debug!(
                    "bounded lane-local ingress queue is full; retaining caller ownership"
                );
                SumeragiIngressDisposition::Retry(message)
            }
            Err(mpsc::TrySendError::Disconnected(message)) => {
                crate::status::record_worker_queue_drop(crate::status::WorkerQueueKind::LaneRelay);
                iroha_logger::warn!("lane-local ingress queue is disconnected");
                self.output_guard
                    .activate_restart_required_from_permit(permit);
                SumeragiIngressDisposition::Closed(message)
            }
        }
    }
    /// Enqueue an inbound lane relay envelope.
    pub fn incoming_lane_relay(&self, envelope: LaneRelayEnvelope) {
        let _ = self.try_incoming_lane_relay(envelope);
    }
    /// Try to enqueue an inbound lane relay envelope.
    pub fn try_incoming_lane_relay(&self, envelope: LaneRelayEnvelope) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::Envelope(envelope))
            .accepted_or_coalesced()
    }
    /// Enqueue an inbound merge-committee signature.
    pub fn incoming_merge_signature(&self, signature: MergeCommitteeSignature) {
        let _ = self.try_incoming_merge_signature(signature);
    }
    /// Try to enqueue an inbound merge-committee signature.
    pub fn try_incoming_merge_signature(&self, signature: MergeCommitteeSignature) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::MergeSignature(signature))
            .accepted_or_coalesced()
    }
    /// Try to enqueue an authenticated lane-drain vote.
    pub fn try_incoming_lane_drain_vote(
        &self,
        sender: PeerId,
        vote: crate::lane_consensus::LaneDrainVoteV1,
    ) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::DrainVote { sender, vote })
            .accepted_or_coalesced()
    }
    /// Try to enqueue authenticated certified merge-sidecar traffic.
    pub fn try_incoming_certified_merge_sidecar(
        &self,
        sender: PeerId,
        message: CertifiedMergeSidecarMessage,
    ) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::CertifiedMergeSidecar {
            sender,
            reply_route: None,
            message,
        })
        .accepted_or_coalesced()
    }
    /// Enqueue an authenticated Native AMX control message.
    pub fn incoming_native_amx(
        &self,
        sender: PeerId,
        message: crate::native_amx::NativeAmxMessage,
    ) {
        let _ = self.try_incoming_native_amx(sender, message);
    }
    /// Try to enqueue an authenticated Native AMX control message.
    pub fn try_incoming_native_amx(
        &self,
        sender: PeerId,
        message: crate::native_amx::NativeAmxMessage,
    ) -> bool {
        self.try_incoming_lane_relay_owned(LaneRelayMessage::NativeAmx {
            sender,
            reply_route: None,
            message,
        })
        .accepted_or_coalesced()
    }
    /// Observe success-only startup recovery for supervised storage maintenance.
    #[must_use]
    pub fn startup_recovery(&self) -> StartupRecovery {
        self.startup_recovery.clone()
    }
    /// Return whether a fatal consensus failure requires process restart.
    #[must_use]
    pub fn restart_required(&self) -> bool {
        self.output_guard.restart_required()
    }
}
impl Drop for V2StartupReplayInventoryGuard {
    fn drop(&mut self) {
        self.finish();
    }
}
